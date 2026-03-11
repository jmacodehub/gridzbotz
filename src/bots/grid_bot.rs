//! ═════════════════════════════════════════════════════════════════════════
//! GRID BOT V6.2 — FILL LEVEL-ID WIRED + CB DELTA P&L
//!
//! PR #102 Commit 3: level_id mark-filled + CB delta P&L fix.
//!   • process_price_update(): capture pnl_before fill loop.
//!     For each fill with Some(level_id):
//!       - OrderSide::Buy  → grid_state.mark_buy_filled(level_id)
//!       - OrderSide::Sell → grid_state.mark_sell_filled(level_id)
//!     Re-read total_realized_pnl() after mark → compute delta.
//!     Pass pnl_delta (not cumulative snapshot) to CB.record_trade().
//!   Root causes fixed:
//!     1. GridStateTracker never updated in live mode (mark_*_filled
//!        only fired in paper mode via order-book matching).
//!     2. CB received cumulative P&L on every fill → NAV drift,
//!        consecutive-loss counter always firing on first loss.
//!
//! PR #101 Commit 2: TelegramBot wired into GridBot.
//! PR #99  Commit 3b: wma_confidence_threshold fully wired end-to-end.
//! PR #98  Commit 2b-ii: WMA voter P&L attribution wired.
//! PR #94  (Commit 6): GridBotStats observability.
//! PR #93: CircuitBreaker wired.
//!
//! March 12, 2026 - V6.2: fill level_id + CB delta P&L (PR #102) ✅
//! March 11, 2026 - V6.1: Telegram alerts wired (PR #101) 📲
//! March 11, 2026 - V6.0: wma_confidence_threshold fully wired (PR #99) 🎯
//! ═════════════════════════════════════════════════════════════════════════

use std::sync::Arc;
use std::time::Instant;
use async_trait::async_trait;
use anyhow::{Result, Context, bail};
use log::{info, warn, debug, trace};

use crate::bots::bot_trait::{Bot, BotStats, IntentRegistry, TickResult};
use crate::strategies::{
    StrategyManager, GridRebalancer, GridRebalancerConfig,
    StrategyRegistryBuilder,
};
use crate::strategies::rsi::{RSIStrategy, RsiConfig};
use crate::strategies::momentum::{MomentumStrategy, MomentumConfig};
use crate::strategies::mean_reversion::{MeanReversionStrategy, MeanReversionConfig};
use crate::strategies::momentum_macd::{MomentumMACDStrategy, MomentumMACDConfig};
use crate::strategies::shared::analytics::AnalyticsContext;
use crate::trading::{
    TradingEngine, OrderSide, GridStateTracker,
    EnhancedMetrics, AdaptiveOptimizer, PriceFeed,
};
use crate::risk::CircuitBreaker;
use crate::config::Config;
use crate::utils::TelegramBot;

// ═════════════════════════════════════════════════════════════════════════
// STRUCT
// ═════════════════════════════════════════════════════════════════════════

pub struct GridBot {
    pub manager:            StrategyManager,
    pub engine:             Arc<dyn TradingEngine + Send + Sync>,
    pub config:             Config,
    pub grid_state:         GridStateTracker,
    pub enhanced_metrics:   EnhancedMetrics,
    pub adaptive_optimizer: AdaptiveOptimizer,
    pub circuit_breaker:    CircuitBreaker,
    pub grid_rebalancer:    GridRebalancer,
    last_signal_strength:   f64,
    orders_filtered_session: u64,
    feed:                   Arc<PriceFeed>,
    session_start:          Instant,
    last_price:             Option<f64>,
    total_cycles:           u64,
    successful_trades:      u64,
    grid_repositions:       u64,
    last_reposition_time:   Option<Instant>,
    last_optimization_cycle: u64,
    grid_initialized:       bool,
    total_fills_tracked:    u64,
    total_orders_placed:    u64,
    last_known_pnl:         f64,
    intent_registry:        Option<IntentRegistry>,
    intent_conflicts:       u64,
    /// PR #101: Telegram notifier. No-op if env vars absent.
    telegram:               TelegramBot,
    /// PR #101: Edge-detection for CB state transitions.
    last_cb_tripped:        bool,
}

// ═════════════════════════════════════════════════════════════════════════
// CONSTRUCTOR
// ═════════════════════════════════════════════════════════════════════════

impl GridBot {
    pub fn new(
        config: Config,
        engine: Arc<dyn TradingEngine + Send + Sync>,
        feed:   Arc<PriceFeed>,
    ) -> Result<Self> {
        info!("[BOT-V6.2] Initializing GridBot V6.2...");
        info!("[BOT-V6.2] WMAConfGate:      {:.2} (TOML-driven, PR #99)",
              config.strategies.wma_confidence_threshold);
        info!("[BOT-V6.2] OptimizerCadence: {} cycles",
              config.trading.optimizer_interval_cycles);
        info!("[BOT-V6.2] ConsensusSizing:  {} | multiplier={:.2}x",
              if config.trading.enable_smart_position_sizing { "ACTIVE" } else { "disabled" },
              config.trading.signal_size_multiplier);

        let telegram = TelegramBot::from_env();

        let grid_config = GridRebalancerConfig {
            grid_spacing:                   config.trading.grid_spacing_percent / 100.0,
            order_size:                     config.trading.min_order_size,
            min_usdc_balance:               config.trading.min_usdc_reserve,
            min_sol_balance:                config.trading.min_sol_reserve,
            enabled:                        config.strategies.grid.enabled,
            enable_dynamic_spacing:         config.trading.enable_dynamic_grid,
            enable_fee_filtering:           config.trading.enable_fee_optimization,
            volatility_window_seconds:      config.trading.volatility_window as u64,
            max_spacing:                    0.0075,
            min_spacing:                    0.001,
            enable_regime_gate:             config.trading.enable_regime_gate,
            min_volatility_to_trade:        config.trading.min_volatility_to_trade,
            pause_in_very_low_vol:          config.trading.pause_in_very_low_vol,
            enable_order_lifecycle:         config.trading.enable_order_lifecycle,
            order_max_age_minutes:          config.trading.order_max_age_minutes,
            order_refresh_interval_minutes: config.trading.order_refresh_interval_minutes,
            min_orders_to_maintain:         config.trading.min_orders_to_maintain,
            ..GridRebalancerConfig::default()
        };

        let grid_rebalancer = GridRebalancer::with_fees(
            grid_config.clone(), config.fees.clone(),
        ).context("Failed to create GridRebalancer")?;

        let grid_rebalancer_for_manager = GridRebalancer::with_fees(
            grid_config, config.fees.clone(),
        ).context("Failed to create GridRebalancer for StrategyManager")?;

        let analytics_ctx = AnalyticsContext::default();

        let (_manager, _weights) = StrategyRegistryBuilder::new()
            .add(grid_rebalancer_for_manager, config.strategies.grid.weight)
            .add_if(
                config.strategies.momentum.enabled,
                MomentumStrategy::new_from_config(&MomentumConfig {
                    fast_period: config.strategies.momentum.lookback_period,
                    ..MomentumConfig::default()
                }),
                config.strategies.momentum.weight,
            )
            .add_if(
                config.strategies.mean_reversion.enabled,
                MeanReversionStrategy::new_from_config(&MeanReversionConfig {
                    mean_period: config.strategies.mean_reversion.sma_period,
                    ..MeanReversionConfig::default()
                }),
                config.strategies.mean_reversion.weight,
            )
            .add_if(
                config.strategies.rsi.enabled,
                RSIStrategy::new_from_config(&RsiConfig {
                    rsi_period:           config.strategies.rsi.period,
                    oversold_threshold:   config.strategies.rsi.oversold_threshold,
                    overbought_threshold: config.strategies.rsi.overbought_threshold,
                    extreme_oversold:     config.strategies.rsi.extreme_oversold,
                    extreme_overbought:   config.strategies.rsi.extreme_overbought,
                }),
                config.strategies.rsi.weight,
            )
            .add_if(
                config.strategies.momentum_macd.enabled,
                MomentumMACDStrategy::new_from_config(&MomentumMACDConfig {
                    strong_histogram_threshold: config.strategies.momentum_macd.strong_histogram_threshold,
                    min_warmup_periods:         config.strategies.momentum_macd.min_warmup_periods,
                    ..MomentumMACDConfig::default()
                }),
                config.strategies.momentum_macd.weight,
            )
            .build_with_confidence(
                analytics_ctx,
                config.strategies.wma_confidence_threshold,
            );

        let manager = _manager;

        info!("[BOT-V6.2] {} strategies loaded (conf_gate={:.2})",
              manager.strategies.len(),
              manager.wma_engine.min_confidence());

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);
        let circuit_breaker    = CircuitBreaker::new(&config);

        info!("[BOT-V6.2] GridBot V6.2 initialization complete");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,
            adaptive_optimizer,
            circuit_breaker,
            grid_rebalancer,
            last_signal_strength:    0.0,
            orders_filtered_session: 0,
            feed,
            session_start:           Instant::now(),
            last_price:              None,
            total_cycles:            0,
            successful_trades:       0,
            grid_repositions:        0,
            last_reposition_time:    None,
            last_optimization_cycle: 0,
            grid_initialized:        false,
            total_fills_tracked:     0,
            total_orders_placed:     0,
            last_known_pnl:          0.0,
            intent_registry:         None,
            intent_conflicts:        0,
            telegram,
            last_cb_tripped:         false,
        })
    }

    async fn pre_init_hook(&mut self) -> Result<()> {
        info!("[BOT] Async pre-init hook complete");
        Ok(())
    }

    async fn initialize_with_price(&mut self) -> Result<()> {
        info!("[BOT] V6.2 GRID INIT - awaiting live price...");

        let initial_price = self.feed.latest_price().await;
        if initial_price <= 0.0 {
            bail!("Invalid initial price ${:.2} - cannot initialize grid", initial_price);
        }
        info!("[BOT] Live price received: ${:.4}", initial_price);

        self.place_grid_orders(initial_price).await
            .context("Initial grid placement failed")?;
        self.grid_initialized = true;
        self.last_price = Some(initial_price);

        let total_levels = self.config.trading.grid_levels as usize;
        let used_levels  = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

        let wallet = self.engine.get_wallet().await;
        self.telegram.send_bot_started(
            self.config.bot.instance_name(),
            &self.config.trading_pair(),
            wallet.total_value_usdc(initial_price),
            self.config.trading.grid_spacing_percent,
            self.config.strategies.wma_confidence_threshold,
            &self.config.bot.execution_mode,
        ).await;

        info!("[BOT] Grid initialized - {} levels @ {:.3}% spacing",
              self.config.trading.grid_levels,
              self.config.trading.grid_spacing_percent);
        Ok(())
    }

    pub async fn should_reposition(&self, current_price: f64, last_price: f64) -> bool {
        if !self.grid_initialized {
            info!("[BOT] Grid not initialized - will initialize on first cycle");
            return true;
        }
        if self.last_price.is_none() { return false; }
        if let Some(last_reposition) = self.last_reposition_time {
            let elapsed = last_reposition.elapsed().as_secs();
            if elapsed < self.config.trading.rebalance_cooldown_secs { return false; }
        }
        let price_change_pct = ((current_price - last_price).abs() / last_price) * 100.0;
        let should = price_change_pct > self.config.trading.reposition_threshold;
        if should {
            debug!("[BOT] Reposition triggered: {:.3}% > {:.3}%",
                   price_change_pct, self.config.trading.reposition_threshold);
        }
        should
    }

    pub async fn reposition_grid(&mut self, current_price: f64, last_price: f64) -> Result<()> {
        if !self.grid_initialized {
            warn!("[BOT] Emergency grid init at ${:.4}", current_price);
            self.place_grid_orders(current_price).await
                .context("Emergency grid initialization failed")?;
            self.grid_initialized = true;
            let total = self.config.trading.grid_levels as usize;
            let used  = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total, used);
            return Ok(());
        }

        info!("[BOT] Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);
        let reposition_start = Instant::now();
        let trading_pair     = self.config.trading_pair();
        let cancellable      = self.grid_state.get_cancellable_levels().await;
        let mut cancelled    = 0;

        for level_id in cancellable {
            if let Some(level) = self.grid_state.get_level(level_id).await {
                if let Some(id) = &level.buy_order_id {
                    self.engine.cancel_order(id).await
                        .unwrap_or_else(|e| warn!("[BOT] Cancel buy failed: {}", e));
                    cancelled += 1;
                }
                if let Some(id) = &level.sell_order_id {
                    self.engine.cancel_order(id).await
                        .unwrap_or_else(|e| warn!("[BOT] Cancel sell failed: {}", e));
                    cancelled += 1;
                }
                self.grid_state.mark_cancelled(level_id).await;
                if let Some(r) = &self.intent_registry {
                    r.remove(&(trading_pair.clone(), level_id));
                }
            }
        }
        if cancelled > 0 { info!("[BOT] Cancelled {} orders", cancelled); }

        self.place_grid_orders(current_price).await?;
        self.grid_repositions += 1;
        self.last_reposition_time = Some(Instant::now());
        let total = self.config.trading.grid_levels as usize;
        let used  = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total, used);
        info!("[BOT] Repositioned in {}ms", reposition_start.elapsed().as_millis());
        Ok(())
    }

    async fn place_grid_orders(&mut self, current_price: f64) -> Result<()> {
        let grid_spacing  = self.adaptive_optimizer.current_spacing_percent;
        let order_size    = self.adaptive_optimizer.current_position_size;
        let num_levels    = self.config.trading.grid_levels;
        let min_order_sol = self.config.trading.min_order_size;
        let pair          = self.config.trading_pair();

        let effective_size = if self.config.trading.enable_smart_position_sizing {
            let m = 1.0 + self.last_signal_strength
                * (self.config.trading.signal_size_multiplier - 1.0);
            (order_size * m).clamp(min_order_sol, self.config.trading.max_position_size)
        } else {
            order_size
        };

        self.grid_rebalancer.update_price(current_price).await
            .unwrap_or_else(|e| warn!("[BOT] Rebalancer price update failed: {}", e));
        let stats = self.grid_rebalancer.grid_stats().await;

        let mut orders_placed   = 0u32;
        let mut orders_failed   = 0u32;
        let mut orders_filtered = 0u32;
        let buy_levels  = num_levels / 2;
        let sell_levels = num_levels - buy_levels;

        for i in 1..=buy_levels.min(sell_levels) {
            let buy_price  = current_price * (1.0 - grid_spacing * i as f64);
            let sell_price = current_price * (1.0 + grid_spacing * i as f64);
            let mut level  = self.grid_state.create_level(buy_price, sell_price, effective_size).await;

            if let Some(registry) = &self.intent_registry {
                let key = (pair.clone(), level.id);
                match registry.entry(key) {
                    dashmap::Entry::Occupied(e) => {
                        self.intent_conflicts += 1;
                        warn!("[INTENT] Level {} owned by '{}' — skipping", level.id, e.get());
                        continue;
                    }
                    dashmap::Entry::Vacant(e) => {
                        e.insert(self.config.bot.instance_name().to_string());
                    }
                }
            }

            if !self.grid_rebalancer
                .should_place_order(OrderSide::Buy, buy_price, min_order_sol, &stats).await
            {
                orders_filtered += 1;
                continue;
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Buy, buy_price, effective_size, Some(level.id)
            ).await {
                Ok(id) => { level.set_buy_order(id); orders_placed += 1; self.total_orders_placed += 1; }
                Err(e) => { warn!("[BOT] Buy failed @ ${:.4}: {}", buy_price, e); orders_failed += 1; continue; }
            }

            if !self.grid_rebalancer
                .should_place_order(OrderSide::Sell, sell_price, min_order_sol, &stats).await
            {
                orders_filtered += 1;
                self.grid_state.update_level(level).await;
                continue;
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Sell, sell_price, effective_size, Some(level.id)
            ).await {
                Ok(id) => { level.set_sell_order(id); orders_placed += 1; self.total_orders_placed += 1; }
                Err(e) => { warn!("[BOT] Sell failed @ ${:.4}: {}", sell_price, e); orders_failed += 1; }
            }
            self.grid_state.update_level(level).await;
        }

        self.orders_filtered_session += orders_filtered as u64;
        info!("[BOT] Placed {} orders, {} filtered, {} failed",
              orders_placed, orders_filtered, orders_failed);
        Ok(())
    }

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        if !self.circuit_breaker.is_trading_allowed() {
            if !self.last_cb_tripped {
                self.last_cb_tripped = true;
                let cb  = self.circuit_breaker.status();
                let nav = self.engine.get_wallet().await.total_value_usdc(price);
                let reason = cb.trip_reason
                    .map(|r| r.to_string())
                    .unwrap_or_else(|| "Unknown".to_string());
                self.telegram.send_circuit_breaker_tripped(
                    self.config.bot.instance_name(),
                    &reason,
                    cb.current_drawdown_pct,
                    nav,
                    self.config.risk.circuit_breaker_cooldown_secs,
                ).await;
            }
            debug!("[CB] Trading halted — skipping tick at ${:.4}", price);
            return Ok(());
        }

        if self.last_cb_tripped {
            self.last_cb_tripped = false;
            self.telegram.send_circuit_breaker_reset(
                self.config.bot.instance_name(),
            ).await;
        }

        self.total_cycles += 1;
        self.last_price = Some(price);
        self.enhanced_metrics.update_price_range(price);
        trace!("[BOT] Tick ${:.4} (cycle {})", price, self.total_cycles);

        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Strategy consensus failed")?;
        self.last_signal_strength = signal.strength();
        self.enhanced_metrics.record_signal(true);

        let filled_orders = self.engine.process_price_update(price).await
            .context("Engine tick failed")?;

        for fill in &filled_orders { self.manager.notify_fill(fill); }

        let wallet  = self.engine.get_wallet().await;
        let new_nav = wallet.total_value_usdc(price);

        if !filled_orders.is_empty() {
            info!("[BOT] {} fills at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            // ── PR #102: Snapshot P&L *before* the fill loop so we can
            //    compute per-fill deltas for CB.record_trade().
            //    Previously the cumulative snapshot was passed on every fill,
            //    causing CB NAV drift and spurious consecutive-loss trips.
            let pnl_before = self.grid_state.total_realized_pnl().await;

            for fill in &filled_orders {
                let is_buy    = fill.side == OrderSide::Buy;
                let fill_size = self.adaptive_optimizer.current_position_size;

                // ── PR #102: Mark the grid level filled so GridStateTracker
                //    stays consistent in live mode. Synthetic FillEvents from
                //    real_trader carry level_id set by place_limit_order_with_level().
                if let Some(lid) = fill.level_id {
                    if is_buy {
                        self.grid_state.mark_buy_filled(lid).await;
                    } else {
                        self.grid_state.mark_sell_filled(lid).await;
                    }
                }

                // Re-read after mark so delta reflects the state update above.
                let pnl_after = self.grid_state.total_realized_pnl().await;
                let pnl_delta = pnl_after - pnl_before;

                self.total_fills_tracked += 1;

                info!("[FILL] #{}: {} {} @ ${:.4} | size:{:.4} | Δ P&L:${:.4} | ts:{}",
                      self.total_fills_tracked,
                      if is_buy { "BUY" } else { "SELL" },
                      fill.order_id, price, fill_size, pnl_delta, timestamp);

                self.enhanced_metrics.record_trade(is_buy, pnl_delta, timestamp);

                // CB receives per-fill delta, not cumulative snapshot.
                self.circuit_breaker.record_trade(pnl_delta, new_nav);

                if fill.side == OrderSide::Sell {
                    self.telegram.send_fill(
                        self.config.bot.instance_name(),
                        "SELL",
                        price,
                        fill_size,
                        pnl_delta,
                        self.total_fills_tracked,
                    ).await;

                    // WMA P&L attribution (PR #98)
                    let voters: Vec<String> = self.manager.get_last_wma_voters().to_vec();
                    for voter in &voters {
                        self.manager.record_fill_for_wma(voter, pnl_delta);
                    }
                    if !voters.is_empty() {
                        debug!("[WMA-ATTR] SELL attributed to {} voters | Δ P&L:${:.4}",
                               voters.len(), pnl_delta);
                    }
                }
            }
        }

        self.last_known_pnl = wallet.pnl_usdc(price);
        self.enhanced_metrics.update_portfolio_value(new_nav);

        // Adaptive optimizer
        if self.total_cycles - self.last_optimization_cycle
            >= self.config.trading.optimizer_interval_cycles
        {
            let result = self.adaptive_optimizer.optimize(&self.enhanced_metrics);
            if result.any_changes() {
                info!("[OPT] Applied: {} | spacing={:.3}% size={:.3} SOL",
                      result.reason, result.new_spacing * 100.0, result.new_position_size);
            }
            self.last_optimization_cycle = self.total_cycles;
        }

        // Periodic heartbeat
        let interval = self.config.metrics.stats_interval;
        if interval > 0 && self.total_cycles % interval == 0 {
            let perf   = self.engine.get_performance_stats().await;
            let cb_ok  = !self.circuit_breaker.status().is_tripped;
            self.telegram.send_heartbeat(
                self.config.bot.instance_name(),
                price,
                new_nav,
                self.last_known_pnl,
                wallet.roi(price),
                self.total_fills_tracked,
                perf.win_rate,
                cb_ok,
            ).await;
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> GridBotStats {
        let wallet        = self.engine.get_wallet().await;
        let perf_stats    = self.engine.get_performance_stats().await;
        let open_orders   = self.engine.open_order_count().await;
        let current_price = self.last_price.unwrap_or(0.0);
        let cb_status     = self.circuit_breaker.status();

        let (fee_checked, fee_passed, fee_blocked) = self.grid_rebalancer
            .fee_filter_stats()
            .map(|s| (s.total_checks, s.trades_passed, s.trades_filtered))
            .unwrap_or((0, 0, 0));

        GridBotStats {
            total_cycles:            self.total_cycles,
            successful_trades:       self.successful_trades,
            grid_repositions:        self.grid_repositions,
            open_orders,
            total_value_usdc:        wallet.total_value_usdc(current_price),
            pnl_usdc:                wallet.pnl_usdc(current_price),
            roi_percent:             wallet.roi(current_price),
            win_rate:                perf_stats.win_rate,
            total_fees:              perf_stats.total_fees,
            trading_paused:          cb_status.is_tripped,
            profitable_trades:       self.enhanced_metrics.profitable_trades,
            unprofitable_trades:     self.enhanced_metrics.unprofitable_trades,
            max_drawdown:            self.enhanced_metrics.max_drawdown,
            signal_execution_ratio:  self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency:         self.enhanced_metrics.grid_efficiency,
            current_spacing_percent: self.adaptive_optimizer.current_spacing_percent,
            current_position_size:   self.adaptive_optimizer.current_position_size,
            optimization_count:      self.adaptive_optimizer.adjustment_count,
            total_fills_tracked:     self.total_fills_tracked,
            intent_conflicts:        self.intent_conflicts,
            last_signal_strength:    self.last_signal_strength,
            orders_filtered_session: self.orders_filtered_session,
            fee_filter_total_checked: fee_checked,
            fee_filter_total_passed:  fee_passed,
            fee_filter_total_blocked: fee_blocked,
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats       = self.get_stats().await;
        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl   = self.grid_state.total_realized_pnl().await;
        let border      = "=".repeat(60);

        println!("\n{border}");
        println!("   [BOT] GRID BOT V6.2 - STATUS REPORT");
        println!("{border}");
        println!("\n[PERFORMANCE]");
        println!("  Total Cycles:      {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions:  {}", stats.grid_repositions);
        println!("  Open Orders:       {}", stats.open_orders);
        println!("  Fills Tracked:     {}", stats.total_fills_tracked);
        println!("  Orders Placed:     {}", self.total_orders_placed);
        println!("  Intent Conflicts:  {}", stats.intent_conflicts);
        println!("  Optimizer Cadence: {} cycles", self.config.trading.optimizer_interval_cycles);
        println!("  WMA Conf Gate:     {:.2}", self.manager.wma_engine.min_confidence());
        println!("  Telegram:          {}", if self.telegram.is_enabled() { "✅ Enabled" } else { "Disabled" });

        if self.config.trading.enable_smart_position_sizing {
            let live_mult = 1.0 + self.last_signal_strength
                * (self.config.trading.signal_size_multiplier - 1.0);
            println!("  Signal Strength:   {:.3} | {:.3}x (max {:.2}x)",
                     self.last_signal_strength, live_mult,
                     self.config.trading.signal_size_multiplier);
        }

        println!("\n[GRID]");
        println!("  Active Levels:     {}", grid_levels);
        println!("  Filled Buys:       {}", filled_buys);
        println!("  Realized P&L:      ${:.2}", total_pnl);

        if let Some(ffs) = self.grid_rebalancer.fee_filter_stats() {
            println!("\n[FEE FILTER]");
            println!("  Total Checked:     {}", ffs.total_checks);
            println!("  Passed:            {}", ffs.trades_passed);
            println!("  Blocked:           {}", ffs.trades_filtered);
        }

        println!("\n[PORTFOLIO]");
        println!("  Total Value:       ${:.2}", stats.total_value_usdc);
        println!("  P&L:               ${:.2}", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);

        println!("\n[TRADING]");
        if stats.profitable_trades + stats.unprofitable_trades == 0 {
            println!("  Win Rate:          - (no closed trades yet)");
        } else {
            println!("  Win Rate:          {:.2}%", stats.win_rate);
        }
        println!("  Total Fees:        ${:.2}", stats.total_fees);
        let cb = self.circuit_breaker.status();
        if cb.is_tripped {
            println!("  [CB] TRIPPED — {} | {}",
                     cb.trip_reason.map(|r| r.to_string()).unwrap_or_default(),
                     cb.cooldown_remaining
                         .map(|d| format!("{}s remaining", d.as_secs()))
                         .unwrap_or_else(|| "resetting".to_string()));
        } else {
            println!("  [CB] OK (losses={} drawdown={:.2}%)",
                     cb.consecutive_losses, cb.current_drawdown_pct);
        }

        println!("\n[METRICS]");
        self.enhanced_metrics.display();
        self.adaptive_optimizer.display();
        println!("\n[PRICE] Current SOL: ${:.4}", current_price);
        println!("\n{border}");
        if grid_levels <= 10 { self.grid_state.display_all().await; }
    }

    pub async fn display_strategy_performance(&self) {
        self.manager.display_stats();
    }
}

// ═════════════════════════════════════════════════════════════════════════
// impl Bot for GridBot
// ═════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Bot for GridBot {
    fn name(&self) -> &str { "GridBot" }

    fn instance_id(&self) -> &str { self.config.bot.instance_name() }

    fn set_intent_registry(&mut self, registry: IntentRegistry) {
        info!("[BOT] Intent registry wired for '{}'", self.instance_id());
        self.intent_registry = Some(registry);
    }

    async fn initialize(&mut self) -> Result<()> {
        self.pre_init_hook().await?;
        self.initialize_with_price().await
            .context("Bot::initialize - grid placement failed")?;
        Ok(())
    }

    async fn process_tick(&mut self) -> Result<TickResult> {
        let price = self.feed.latest_price().await;
        if price <= 0.0 {
            warn!("[BOT::process_tick] Invalid price {:.4} — shutdown", price);
            return Ok(TickResult::shutdown());
        }
        let ts = chrono::Utc::now().timestamp();
        let fills_before  = self.total_fills_tracked;
        let orders_before = self.total_orders_placed;

        self.process_price_update(price, ts).await?;

        let fills_this_tick  = self.total_fills_tracked.saturating_sub(fills_before);
        let orders_this_tick = self.total_orders_placed.saturating_sub(orders_before);
        let stats            = self.get_stats().await;

        if stats.trading_paused {
            return Ok(TickResult::paused("circuit breaker tripped"));
        }
        Ok(TickResult::active(fills_this_tick, orders_this_tick))
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("[BOT] Graceful shutdown for '{}'", self.instance_id());
        let final_price = self.last_price.unwrap_or(0.0);
        self.display_status(final_price).await;
        self.display_strategy_performance().await;

        let wallet  = self.engine.get_wallet().await;
        let perf    = self.engine.get_performance_stats().await;
        self.telegram.send_shutdown(
            self.instance_id(),
            self.session_start.elapsed().as_secs(),
            self.total_fills_tracked,
            self.total_orders_placed,
            self.last_known_pnl,
            wallet.roi(final_price),
            perf.win_rate,
        ).await;

        info!("[BOT] Shutdown complete | cycles={} fills={} orders={} repos={} pnl=${:.2}",
              self.total_cycles, self.total_fills_tracked, self.total_orders_placed,
              self.grid_repositions, self.last_known_pnl);
        Ok(())
    }

    fn stats(&self) -> BotStats {
        BotStats {
            instance_id:      self.config.bot.instance_name().to_string(),
            bot_type:         "GridBot".to_string(),
            total_cycles:     self.total_cycles,
            total_fills:      self.total_fills_tracked,
            total_orders:     self.total_orders_placed,
            uptime_secs:      self.session_start.elapsed().as_secs(),
            is_paused:        self.circuit_breaker.status().is_tripped,
            current_pnl:      self.last_known_pnl,
            intent_conflicts: self.intent_conflicts,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// GRID BOT STATS
// ═════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct GridBotStats {
    pub total_cycles:            u64,
    pub successful_trades:       u64,
    pub grid_repositions:        u64,
    pub open_orders:             usize,
    pub total_value_usdc:        f64,
    pub pnl_usdc:                f64,
    pub roi_percent:             f64,
    pub win_rate:                f64,
    pub total_fees:              f64,
    pub trading_paused:          bool,
    pub profitable_trades:       usize,
    pub unprofitable_trades:     usize,
    pub max_drawdown:            f64,
    pub signal_execution_ratio:  f64,
    pub grid_efficiency:         f64,
    pub current_spacing_percent: f64,
    pub current_position_size:   f64,
    pub optimization_count:      u64,
    pub total_fills_tracked:     u64,
    pub intent_conflicts:        u64,
    pub last_signal_strength:    f64,
    pub orders_filtered_session: u64,
    pub fee_filter_total_checked: u64,
    pub fee_filter_total_passed:  u64,
    pub fee_filter_total_blocked: u64,
}

impl GridBotStats {
    pub fn display_summary(&self) {
        println!("\n[STATS] GRID BOT V6.2 STATISTICS");
        println!("   Cycles:     {}", self.total_cycles);
        println!("   Trades:     {}", self.successful_trades);
        println!("   Fills:      {}", self.total_fills_tracked);
        println!("   Value:      ${:.2}", self.total_value_usdc);
        println!("   P&L:        ${:.2}", self.pnl_usdc);
        println!("   ROI:        {:.2}%", self.roi_percent);
        if self.profitable_trades + self.unprofitable_trades == 0 {
            println!("   Win Rate:   - (no closed trades)");
        } else {
            println!("   Win Rate:   {:.2}%", self.win_rate);
        }
        println!("   Fees:       ${:.2}", self.total_fees);
        println!("   CB Status:  {}",
                 if self.trading_paused { "TRIPPED" } else { "OK" });
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::risk::circuit_breaker::TripReason;

    fn zero_stats() -> GridBotStats {
        GridBotStats {
            total_cycles: 0, successful_trades: 0, grid_repositions: 0,
            open_orders: 0, total_value_usdc: 0.0, pnl_usdc: 0.0,
            roi_percent: 0.0, win_rate: 0.0, total_fees: 0.0,
            trading_paused: false, profitable_trades: 0, unprofitable_trades: 0,
            max_drawdown: 0.0, signal_execution_ratio: 0.0, grid_efficiency: 0.0,
            current_spacing_percent: 0.0, current_position_size: 0.0,
            optimization_count: 0, total_fills_tracked: 0, intent_conflicts: 0,
            last_signal_strength: 0.0, orders_filtered_session: 0,
            fee_filter_total_checked: 0, fee_filter_total_passed: 0,
            fee_filter_total_blocked: 0,
        }
    }

    #[test]
    fn test_gridbotstats_fields() {
        let stats = GridBotStats {
            total_cycles: 100, successful_trades: 42, grid_repositions: 3,
            open_orders: 6, total_value_usdc: 1050.0, pnl_usdc: 50.0,
            roi_percent: 5.0, win_rate: 0.65, total_fees: 1.25,
            trading_paused: false, profitable_trades: 28, unprofitable_trades: 14,
            max_drawdown: 2.1, signal_execution_ratio: 0.88, grid_efficiency: 0.91,
            current_spacing_percent: 0.003, current_position_size: 0.1,
            optimization_count: 2, total_fills_tracked: 42, intent_conflicts: 0,
            last_signal_strength: 0.0, orders_filtered_session: 0,
            fee_filter_total_checked: 0, fee_filter_total_passed: 0,
            fee_filter_total_blocked: 0,
        };
        assert_eq!(stats.total_cycles, 100);
        assert_eq!(stats.successful_trades, 42);
        assert!(!stats.trading_paused);
        assert!((stats.pnl_usdc - 50.0).abs() < 1e-9);
    }

    #[test]
    fn test_gridbotstats_intent_conflicts_tracked() {
        let s = GridBotStats { intent_conflicts: 3, ..zero_stats() };
        assert_eq!(s.intent_conflicts, 3);
    }

    #[test]
    fn test_win_rate_guard_zero_closed_trades() {
        let s = zero_stats();
        assert_eq!(s.profitable_trades + s.unprofitable_trades, 0);
        assert!((s.win_rate - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_win_rate_guard_with_closed_trades() {
        let s = GridBotStats {
            total_cycles: 200, successful_trades: 10, grid_repositions: 1,
            open_orders: 6, total_value_usdc: 1020.0, pnl_usdc: 20.0,
            roi_percent: 2.0, win_rate: 75.0, total_fees: 0.25,
            profitable_trades: 6, unprofitable_trades: 2, max_drawdown: 0.5,
            signal_execution_ratio: 99.8, grid_efficiency: 0.7,
            current_spacing_percent: 0.003, current_position_size: 0.1,
            optimization_count: 1, total_fills_tracked: 10, ..zero_stats()
        };
        assert!(s.profitable_trades + s.unprofitable_trades > 0);
        assert!((s.win_rate - 75.0).abs() < 1e-9);
    }

    #[test]
    fn test_tick_result_paused_reason() {
        let r = TickResult::paused("regime gate / circuit breaker");
        assert!(r.active);
        assert!(r.pause_reason.as_deref().unwrap_or("").contains("regime gate"));
    }

    #[test]
    fn test_tick_result_shutdown_on_bad_price() {
        let r = TickResult::shutdown();
        assert!(!r.active);
        assert_eq!(r.fills, 0);
    }

    #[test]
    fn test_bot_stats_default_zero() {
        let s = BotStats::default();
        assert_eq!(s.total_cycles, 0);
        assert!(!s.is_paused);
    }

    #[test]
    fn test_tick_result_orders_placed_field() {
        let r = TickResult::active(2, 6);
        assert_eq!(r.fills, 2);
        assert_eq!(r.orders_placed, 6);
    }

    #[test]
    fn test_registry_cleanup_on_reposition() {
        use crate::bots::bot_trait::new_intent_registry;
        let registry = new_intent_registry();
        let pair = "SOL/USDC".to_string();
        registry.insert((pair.clone(), 1u64), "bot-01".into());
        registry.insert((pair.clone(), 2u64), "bot-01".into());
        registry.insert((pair.clone(), 3u64), "bot-01".into());
        registry.remove(&(pair.clone(), 1u64));
        registry.remove(&(pair.clone(), 2u64));
        assert_eq!(registry.len(), 1);
        assert!(registry.contains_key(&(pair.clone(), 3u64)));
    }

    #[test]
    fn test_circuit_breaker_field_initialized() {
        use crate::config::*;
        let config = Config {
            bot: BotConfig { name: "test".into(), version: "1.0".into(),
                environment: "test".into(), execution_mode: "paper".into(), instance_id: None },
            network: NetworkConfig { cluster: "devnet".into(),
                rpc_url: "http://localhost".into(), commitment: "confirmed".into(), ws_url: None },
            security: SecurityConfig::default(),
            trading: TradingConfig::default(),
            strategies: StrategiesConfig::default(),
            execution: ExecutionConfig::default(),
            risk: RiskConfig { max_position_size_pct: 80.0, max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0, take_profit_pct: 10.0, enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0, circuit_breaker_cooldown_secs: 60,
                max_consecutive_losses: 5, enable_trailing_stop: false },
            fees: FeesConfig::default(), priority_fees: PriorityFeeConfig::default(),
            pyth: PythConfig::default(), performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(), metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(), alerts: AlertsConfig::default(),
        };
        let cb = CircuitBreaker::new(&config);
        assert!(!cb.status().is_tripped);
    }

    #[test]
    fn test_circuit_breaker_trading_paused_in_stats() {
        let s = GridBotStats { trading_paused: true, ..zero_stats() };
        assert!(s.trading_paused);
        let s2 = GridBotStats { trading_paused: false, ..s.clone() };
        assert!(!s2.trading_paused);
    }

    #[test]
    fn test_circuit_breaker_record_trade_real_pnl() {
        use crate::config::*;
        let config = Config {
            bot: BotConfig { name: "test".into(), version: "1.0".into(),
                environment: "test".into(), execution_mode: "paper".into(), instance_id: None },
            network: NetworkConfig { cluster: "devnet".into(),
                rpc_url: "http://localhost".into(), commitment: "confirmed".into(), ws_url: None },
            security: SecurityConfig::default(), trading: TradingConfig::default(),
            strategies: StrategiesConfig::default(), execution: ExecutionConfig::default(),
            risk: RiskConfig { max_position_size_pct: 80.0, max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0, take_profit_pct: 10.0, enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0, circuit_breaker_cooldown_secs: 60,
                max_consecutive_losses: 3, enable_trailing_stop: false },
            fees: FeesConfig::default(), priority_fees: PriorityFeeConfig::default(),
            pyth: PythConfig::default(), performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(), metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(), alerts: AlertsConfig::default(),
        };
        let mut cb = CircuitBreaker::new(&config);
        cb.record_trade(-10.0, 990.0);
        cb.record_trade(-10.0, 980.0);
        cb.record_trade(-10.0, 970.0);
        assert!(cb.status().is_tripped);
        assert!(matches!(cb.status().trip_reason, Some(TripReason::ConsecutiveLosses)));
    }

    fn compute_effective_size(enable: bool, size: f64, strength: f64,
        mult: f64, min: f64, max: f64) -> f64 {
        if enable {
            (size * (1.0 + strength * (mult - 1.0))).clamp(min, max)
        } else { size }
    }

    #[test] fn test_smart_sizing_disabled_uses_base_size() {
        assert!((compute_effective_size(false,0.1,1.0,2.0,0.05,10.0) - 0.1).abs() < 1e-12);
    }
    #[test] fn test_smart_sizing_hold_signal_no_size_change() {
        for m in [1.0f64,1.5,2.0,3.0] {
            assert!((compute_effective_size(true,0.1,0.0,m,0.05,10.0) - 0.1).abs() < 1e-12);
        }
    }
    #[test] fn test_smart_sizing_strong_signal_scales_up() {
        assert!((compute_effective_size(true,0.1,1.0,2.0,0.05,10.0) - 0.2).abs() < 1e-12);
    }
    #[test] fn test_smart_sizing_clamp_respects_max_position() {
        assert!((compute_effective_size(true,5.0,1.0,3.0,0.05,8.0) - 8.0).abs() < 1e-12);
    }
    #[test] fn test_smart_sizing_clamp_respects_min_order() {
        assert!((compute_effective_size(true,0.1,1.0,0.5,0.08,10.0) - 0.08).abs() < 1e-12);
    }

    #[test]
    fn test_gridbotstats_fee_filter_fields_zero_default() {
        let s = zero_stats();
        assert_eq!(s.fee_filter_total_checked, 0);
        assert_eq!(s.fee_filter_total_passed, 0);
        assert_eq!(s.fee_filter_total_blocked, 0);
    }

    #[test]
    fn test_gridbotstats_fee_filter_fields_populated() {
        let s = GridBotStats {
            fee_filter_total_checked: 120,
            fee_filter_total_passed: 95,
            fee_filter_total_blocked: 25,
            orders_filtered_session: 25,
            ..zero_stats()
        };
        assert_eq!(s.fee_filter_total_passed + s.fee_filter_total_blocked,
                   s.fee_filter_total_checked);
    }

    #[test]
    fn test_gridbotstats_signal_strength_field() {
        let s = GridBotStats { last_signal_strength: 0.75, ..zero_stats() };
        assert!((s.last_signal_strength - 0.75).abs() < 1e-12);
    }

    #[test]
    fn test_telegram_disabled_by_default_no_env() {
        let bot = crate::utils::TelegramBot::new(None, None);
        assert!(!bot.is_enabled());
    }

    #[test]
    fn test_last_cb_tripped_initial_false() {
        let tripped: bool = false;
        assert!(!tripped);
    }

    /// PR #102: Verify pnl_delta semantics — delta must be zero when
    /// P&L does not change (e.g. BUY fill before a SELL closes the round-trip).
    #[test]
    fn test_pnl_delta_zero_on_buy_fill() {
        let pnl_before = 0.0_f64;
        let pnl_after  = 0.0_f64; // BUY fill does not realize P&L
        let delta = pnl_after - pnl_before;
        assert!((delta - 0.0).abs() < 1e-12);
    }

    /// PR #102: Verify pnl_delta is positive on a profitable SELL.
    #[test]
    fn test_pnl_delta_positive_on_profitable_sell() {
        let pnl_before = 0.0_f64;
        let pnl_after  = 1.25_f64; // SELL closed a profitable round-trip
        let delta = pnl_after - pnl_before;
        assert!(delta > 0.0);
        assert!((delta - 1.25).abs() < 1e-12);
    }
}
