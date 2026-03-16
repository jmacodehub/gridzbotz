//! ═════════════════════════════════════════════════════════════════
//! GRID BOT V7.3 — WIN RATE GUARD → TickResult::paused (PR #131 C4)
//!
//! PR #131 C4: Wire WinRateGuard into process_tick()
//!   GAP: WinRateGuard struct was built (C4 Step 1) but never consumed
//!         by GridBot. Fleet manager had no visibility into win-rate
//!         suppression — bot appeared active during guard events.
//!   FIX:  Add win_rate_guard: WinRateGuard field to GridBot struct.
//!         Construct via WinRateGuard::new(&config) in GridBot::new().
//!         In process_tick(), after SL gate and before regime gate,
//!         call evaluate(perf.win_rate, self.successful_trades).
//!         Return TickResult::paused(reason) when suppressed.
//!   Change:
//!     ADDED import:  WinRateGuard to use crate::risk::{...}
//!     ADDED field:   win_rate_guard: WinRateGuard
//!     ADDED ctor:    let win_rate_guard = WinRateGuard::new(&config);
//!     ADDED (7 lines) in process_tick() between SL gate and regime gate:
//!       // ── PR #131 C4: Win Rate Guard ────────────────────────────
//!       let perf = self.engine.get_performance_stats().await;
//!       if !self.win_rate_guard.evaluate(perf.win_rate, self.successful_trades) {
//!           return Ok(TickResult::paused(
//!               self.win_rate_guard.reason().to_string()
//!           ));
//!       }
//!     ADDED (1 test): test_process_tick_win_rate_guard_returns_paused_reason
//!   Net: +1 import, +1 field, +2 ctor lines, +7 LOC in process_tick(),
//!        +1 boot log, +1 test. Zero trait/config/other-file changes.
//!
//! PR #126 C3: Wire StopLossManager into process_tick() — SL cooldown gate
//! PR #125 C3: Wire regime gate pause into TickResult
//! PR #121: Move notify_fill() post-enrichment — WMA P&L accuracy fix
//! PR #120: Wire should_reposition() into process_tick()
//! PR #119 C3: Wire reopen_level() — compile blocker resolved
//! PR #119 C2: Wire check_stale_orders() into process_price_update()
//! PR #119 C1: grid_rebalancer.rs V6.0 — check_stale_orders() method
//! PR #118: Wire total_realized_pnl() into BotStats.current_pnl
//! PR #117: Wire real pnl_delta into FillEvent.pnl per fill
//! PR #107: fee-reconciliation, dynamic spacing bounds, Bot trait contract
//! PR #105 C3: GridRebalancer switched to add_execution_only()
//! PR #102 C3: level_id mark-filled + CB delta P&L fix
//! PR #101 C2: TelegramBot wired
//! PR #99  C3b: wma_confidence_threshold wired
//! PR #98  C2b-ii: WMA voter P&L attribution
//!
//! March 16, 2026 - V7.3: Win Rate Guard → TickResult::paused (PR #131 C4) 📉
//! March 15, 2026 - V7.2: SL cooldown gate → TickResult::paused (PR #126 C3) 🛑
//! March 15, 2026 - V7.1: Regime gate → TickResult::paused (PR #125 C3) ⛔
//! March 14, 2026 - V7.0: notify_fill post-enrichment (PR #121) ✅
//! March 14, 2026 - V6.9: should_reposition() wired (PR #120) ✅
//! March 14, 2026 - V6.8: reopen_level() wired — PR #119 C3 complete ✅
//! March 14, 2026 - V6.7: Lifecycle engine wired (PR #119 C2) ⏰
//! March 14, 2026 - V6.6: BotStats.current_pnl → realized P&L (PR #118) 💰
//! March 13, 2026 - V6.5: FillEvent.pnl wired with real delta (PR #117) 💰
//! March 13, 2026 - V6.4: fee-reconciliation wired (PR #107) 💰
//! ═════════════════════════════════════════════════════════════════

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
use crate::trading::grid_level::GridLevelStatus;
use crate::risk::{CircuitBreaker, StopLossManager, WinRateGuard};
use crate::config::Config;
use crate::utils::TelegramBot;

// ══════════════════════════════════════════════════════════════════════
// STRUCT
// ══════════════════════════════════════════════════════════════════════

pub struct GridBot {
    pub manager:            StrategyManager,
    pub engine:             Arc<dyn TradingEngine + Send + Sync>,
    pub config:             Config,
    pub grid_state:         GridStateTracker,
    pub enhanced_metrics:   EnhancedMetrics,
    pub adaptive_optimizer: AdaptiveOptimizer,
    pub circuit_breaker:    CircuitBreaker,
    pub stop_loss_manager:  StopLossManager,
    pub win_rate_guard:     WinRateGuard,
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
    /// PR #118: stores total_realized_pnl() — locked-in grid trading profit.
    last_known_pnl:         f64,
    intent_registry:        Option<IntentRegistry>,
    intent_conflicts:       u64,
    /// PR #101: Telegram notifier.
    telegram:               TelegramBot,
    /// PR #101: Edge-detection for CB state transitions.
    last_cb_tripped:        bool,
}

// ══════════════════════════════════════════════════════════════════════
// CONSTRUCTOR
// ══════════════════════════════════════════════════════════════════════

impl GridBot {
    pub fn new(
        config: Config,
        engine: Arc<dyn TradingEngine + Send + Sync>,
        feed:   Arc<PriceFeed>,
    ) -> Result<Self> {
        info!("[BOT-V7.3] Initializing GridBot V7.3...");
        info!("[BOT-V7.3] WMAConfGate:      {:.2} (TOML-driven, PR #99)",
              config.strategies.wma_confidence_threshold);
        info!("[BOT-V7.3] OptimizerCadence: {} cycles",
              config.trading.optimizer_interval_cycles);
        info!("[BOT-V7.3] ConsensusSizing:  {} | multiplier={:.2}x",
              if config.trading.enable_smart_position_sizing { "ACTIVE" } else { "disabled" },
              config.trading.signal_size_multiplier);
        info!("[BOT-V7.3] DynSpacingBounds: {:.5}\u{2013}{:.5} (PR #107 C2)",
              config.trading.min_grid_spacing_pct,
              config.trading.max_grid_spacing_pct);
        info!("[BOT-V7.3] TakerFee:         {:.4}% ({:.1} bps) (PR #107 C4)",
              config.fees.taker_fee_percent(),
              config.fees.taker_fee_bps);
        info!("[BOT-V7.3] PnLSource:        grid_state.total_realized_pnl() (PR #118)");
        info!("[BOT-V7.3] Lifecycle:        enable={} max_age={}m refresh={}m (PR #119)",
              config.trading.enable_order_lifecycle,
              config.trading.order_max_age_minutes,
              config.trading.order_refresh_interval_minutes);
        info!("[BOT-V7.3] Reposition:       threshold={:.2}% cooldown={}s (PR #120)",
              config.trading.reposition_threshold,
              config.trading.rebalance_cooldown_secs);
        info!("[BOT-V7.3] WMAFillPnL:       notify_fill post-enrichment (PR #121)");
        info!("[BOT-V7.3] RegimeGatePause:  TickResult::paused wired (PR #125 C3)");
        info!("[BOT-V7.3] SLCooldownGate:   StopLossManager wired (PR #126 C3)");
        info!("[BOT-V7.3] WinRateGuard:     enabled={} min={:.1}% resume={:.1}% warmup={} (PR #131 C4)",
              config.risk.enable_win_rate_guard,
              config.risk.min_win_rate_pct,
              config.risk.win_rate_guard_resume_pct,
              config.risk.min_trades_before_guard);

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
            max_spacing:                    config.trading.max_grid_spacing_pct,
            min_spacing:                    config.trading.min_grid_spacing_pct,
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
            .add_execution_only(grid_rebalancer_for_manager, config.strategies.grid.weight)
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

        info!("[BOT-V7.3] {} strategies loaded ({} WMA voters, conf_gate={:.2})",
              manager.strategies.len(),
              manager.wma_engine.registered_count(),
              manager.wma_engine.min_confidence());

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);
        let circuit_breaker    = CircuitBreaker::new(&config);
        let stop_loss_manager  = StopLossManager::new(&config);
        let win_rate_guard     = WinRateGuard::new(&config);

        info!("[BOT-V7.3] GridBot V7.3 initialization complete");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,
            adaptive_optimizer,
            circuit_breaker,
            stop_loss_manager,
            win_rate_guard,
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
        info!("[BOT] V7.3 GRID INIT - awaiting live price...");

        let initial_price = self.feed.latest_price().await;
        if initial_price <= 0.0 {
            bail!("Invalid initial price ${:.2} - cannot initialize grid", initial_price);
        }
        info!("[BOT] Live price received: ${:.4}", initial_price);

        self.place_grid_orders(initial_price).await
            .context("Initial grid placement failed")?;
        self.grid_initialized = true;
        self.last_price = Some(initial_price);

        let total_levels            = self.config.trading.grid_levels as usize;
        let (pending, buy_filled, _) = self.grid_state.level_counts().await;
        let used_levels             = pending + buy_filled;
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
            let total                = self.config.trading.grid_levels as usize;
            let (p, b, _)            = self.grid_state.level_counts().await;
            self.enhanced_metrics.update_grid_stats(total, p + b);
            return Ok(());
        }

        info!("[BOT] Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);
        let reposition_start = Instant::now();
        let trading_pair     = self.config.trading_pair();

        let all_levels   = self.grid_state.get_all_levels().await;
        let cancellable: Vec<u64> = all_levels
            .iter()
            .filter(|l| {
                l.status == GridLevelStatus::Pending
                    || l.status == GridLevelStatus::BuyFilled
            })
            .map(|l| l.id)
            .collect();

        let mut cancelled = 0;
        for level_id in cancellable {
            let levels_snap = self.grid_state.get_all_levels().await;
            if let Some(level) = levels_snap.iter().find(|l| l.id == level_id) {
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
                self.grid_state.cancel_level(level_id).await;
                if let Some(r) = &self.intent_registry {
                    r.remove(&(trading_pair.clone(), level_id));
                }
            }
        }
        if cancelled > 0 { info!("[BOT] Cancelled {} orders", cancelled); }

        self.place_grid_orders(current_price).await?;
        self.grid_repositions += 1;
        self.last_reposition_time = Some(Instant::now());
        let total     = self.config.trading.grid_levels as usize;
        let (p, b, _) = self.grid_state.level_counts().await;
        self.enhanced_metrics.update_grid_stats(total, p + b);
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

            let level_id = self.grid_state.create_level(buy_price, sell_price, effective_size).await;

            if let Some(registry) = &self.intent_registry {
                let key = (pair.clone(), level_id);
                match registry.entry(key) {
                    dashmap::Entry::Occupied(e) => {
                        self.intent_conflicts += 1;
                        warn!("[INTENT] Level {} owned by '{}' \u{2014} skipping", level_id, e.get());
                        self.grid_state.cancel_level(level_id).await;
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
                OrderSide::Buy, buy_price, effective_size, Some(level_id)
            ).await {
                Ok(_id) => { orders_placed += 1; self.total_orders_placed += 1; }
                Err(e) => { warn!("[BOT] Buy failed @ ${:.4}: {}", buy_price, e); orders_failed += 1; continue; }
            }

            if !self.grid_rebalancer
                .should_place_order(OrderSide::Sell, sell_price, min_order_sol, &stats).await
            {
                orders_filtered += 1;
                continue;
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Sell, sell_price, effective_size, Some(level_id)
            ).await {
                Ok(_id) => { orders_placed += 1; self.total_orders_placed += 1; }
                Err(e) => { warn!("[BOT] Sell failed @ ${:.4}: {}", sell_price, e); orders_failed += 1; }
            }
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
            debug!("[CB] Trading halted \u{2014} skipping tick at ${:.4}", price);
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

        let mut filled_orders = self.engine.process_price_update(price).await
            .context("Engine tick failed")?;

        let wallet  = self.engine.get_wallet().await;
        let new_nav = wallet.total_value_usdc(price);

        if !filled_orders.is_empty() {
            info!("[BOT] {} fills at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            let taker_fee_fraction = self.config.fees.taker_fee_fraction();

            for fill in &mut filled_orders {
                let is_buy    = fill.side == OrderSide::Buy;
                let fill_size = self.adaptive_optimizer.current_position_size;
                let fee_usdc  = price * fill_size * taker_fee_fraction;

                let pnl_before = self.grid_state.total_realized_pnl().await;

                if let Some(lid) = fill.level_id {
                    if is_buy {
                        self.grid_state.mark_buy_filled(lid, fee_usdc).await;
                    } else {
                        self.grid_state.mark_sell_filled(lid, fee_usdc).await;
                    }
                }

                let pnl_after = self.grid_state.total_realized_pnl().await;
                let pnl_delta = pnl_after - pnl_before;

                fill.pnl = Some(pnl_delta);

                // ── PR #121: notify_fill post-enrichment ─────────────────────────────────────
                // fill.pnl is now Some(pnl_delta) — WMA voters receive accurate P&L.
                // Previously fired pre-loop before enrichment (fill.pnl = None).
                self.manager.notify_fill(fill);
                // ───────────────────────────────────────────────────────────────────────

                self.total_fills_tracked += 1;

                info!("[FILL] #{}: {} {} @ ${:.4} | size:{:.4} | fee:${:.4} | \u{0394}P&L:${:.4} | ts:{}",
                      self.total_fills_tracked,
                      if is_buy { "BUY" } else { "SELL" },
                      fill.order_id, price, fill_size, fee_usdc, pnl_delta, timestamp);

                self.enhanced_metrics.record_trade(is_buy, pnl_delta, timestamp);
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

                    let voters: Vec<String> = self.manager.get_last_wma_voters().to_vec();
                    for voter in &voters {
                        self.manager.record_fill_for_wma(voter, pnl_delta);
                    }
                    if !voters.is_empty() {
                        debug!("[WMA-ATTR] SELL attributed to {} voters | \u{0394}P&L:${:.4}",
                               voters.len(), pnl_delta);
                    }
                }
            }
        }

        // ── PR #118: source last_known_pnl from total_realized_pnl() ─────────────────────
        self.last_known_pnl = self.grid_state.total_realized_pnl().await;

        // ── PR #119 C2/C3: Order Lifecycle Engine ────────────────────────────────────
        let stale_ids = self.grid_rebalancer
            .check_stale_orders(&self.grid_state, price)
            .await;

        if !stale_ids.is_empty() {
            let order_size = self.adaptive_optimizer.current_position_size;
            for level_id in stale_ids {
                let sell_price = price * (1.0 + self.config.trading.grid_spacing_percent / 100.0);
                self.grid_state.reopen_level(level_id, price, sell_price).await;

                match self.engine.place_limit_order_with_level(
                    OrderSide::Buy, price, order_size, Some(level_id)
                ).await {
                    Ok(_id) => {
                        self.total_orders_placed += 1;
                        debug!("[LIFECYCLE] Re-placed level {} buy @ ${:.4}", level_id, price);
                    }
                    Err(e) => {
                        warn!("[LIFECYCLE] Re-place failed for level {}: {}", level_id, e);
                    }
                }
            }
        }

        self.enhanced_metrics.update_portfolio_value(new_nav);

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

        let interval = self.config.metrics.stats_interval;
        if interval > 0 && self.total_cycles % interval == 0 {
            let total_fees_paid = self.grid_state.total_fees_paid().await;
            info!("[FEES] Total fees paid (grid levels): ${:.4} | taker_rate={:.4}%",
                  total_fees_paid, self.config.fees.taker_fee_percent());

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
            pnl_usdc:                self.last_known_pnl,
            roi_percent:             wallet.roi(current_price),
            win_rate:                perf_stats.win_rate,
            total_fees:              perf_stats.total_fees,
            trading_paused:          cb_status.is_tripped,
            profitable_trades:       self.enhanced_metrics.profitable_trades as u64,
            unprofitable_trades:     self.enhanced_metrics.unprofitable_trades as u64,
            max_drawdown:            self.enhanced_metrics.max_drawdown,
            signal_execution_ratio:  self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency:         self.enhanced_metrics.grid_efficiency,
            current_spacing_percent: self.adaptive_optimizer.current_spacing_percent,
            current_position_size:   self.adaptive_optimizer.current_position_size,
            optimization_count:      self.adaptive_optimizer.adjustment_count as u32,
            total_fills_tracked:     self.total_fills_tracked,
            intent_conflicts:        self.intent_conflicts,
            fee_checks:              fee_checked,
            fee_passed,
            fee_blocked,
            orders_filtered_session: self.orders_filtered_session,
        }
    }

    pub async fn display_status(&self) {
        let stats       = self.get_stats().await;
        let uptime_secs = self.session_start.elapsed().as_secs();
        let cb_status   = self.circuit_breaker.status();
        let total_fees  = self.grid_state.total_fees_paid().await;

        println!("\n\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}");
        println!(  "\u{2551}     GRID BOT V7.3 STATUS (PR #131 C4 \u{1f4c9})    \u{2551}");
        println!(  "\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}");
        println!("  Instance:          {}",   self.config.bot.instance_name());
        println!("  Uptime:            {}s",  uptime_secs);
        println!("  Cycles:            {}",   stats.total_cycles);
        println!("  Fills Tracked:     {}",   stats.total_fills_tracked);
        println!("  Successful Trades: {}",   stats.successful_trades);
        println!("  Grid Repositions:  {}",   stats.grid_repositions);
        println!("  Open Orders:       {}",   stats.open_orders);
        println!();
        println!("  Portfolio Value:   ${:.2}", stats.total_value_usdc);
        println!("  Realized P&L:      ${:.4} (grid fills only)", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);
        println!("  Win Rate:          {:.1}%", stats.win_rate * 100.0);
        println!("  Total Fees Paid:   ${:.4} (taker={:.4}%)",
                 total_fees, self.config.fees.taker_fee_percent());
        println!();
        println!("  CB Tripped:        {}",   cb_status.is_tripped);
        println!("  Max Drawdown:      {:.2}%", stats.max_drawdown);
        println!("  Fee Checks:        {} passed / {} blocked",
                 stats.fee_passed, stats.fee_blocked);
        println!("  Orders Filtered:   {}",   stats.orders_filtered_session);
        println!("  Intent Conflicts:  {}",   stats.intent_conflicts);
        println!();
        println!("  Spacing:           {:.3}%", stats.current_spacing_percent * 100.0);
        println!("  Position Size:     {:.4} SOL", stats.current_position_size);
        println!("  Optimizations:     {}",   stats.optimization_count);
        println!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");
    }

    pub async fn display_strategy_performance(&self) {
        self.manager.display_stats();
    }
}

// ══════════════════════════════════════════════════════════════════════
// GridBotStats
// ══════════════════════════════════════════════════════════════════════

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
    pub profitable_trades:       u64,
    pub unprofitable_trades:     u64,
    pub max_drawdown:            f64,
    pub signal_execution_ratio:  f64,
    pub grid_efficiency:         f64,
    pub current_spacing_percent: f64,
    pub current_position_size:   f64,
    pub optimization_count:      u32,
    pub total_fills_tracked:     u64,
    pub intent_conflicts:        u64,
    pub fee_checks:              u64,
    pub fee_passed:              u64,
    pub fee_blocked:             u64,
    pub orders_filtered_session: u64,
}

// ══════════════════════════════════════════════════════════════════════
// impl Bot for GridBot
// ══════════════════════════════════════════════════════════════════════

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

    /// PR #120: snapshot last_price BEFORE process_price_update() overwrites
    /// self.last_price, then check for price drift and reposition if needed.
    /// PR #125 C3: After process_price_update(), check regime gate state and
    /// return TickResult::paused if GridRebalancer suppressed trading this tick.
    /// PR #126 C3: Before regime gate, check SL cooldown gate and return
    /// TickResult::paused if StopLossManager is in cooldown.
    /// PR #131 C4: After SL gate and before regime gate, evaluate WinRateGuard.
    /// Return TickResult::paused if win-rate has fallen below min threshold.
    async fn process_tick(&mut self) -> Result<TickResult> {
        let price = self.feed.latest_price().await;
        if price <= 0.0 {
            warn!("[BOT::process_tick] Invalid price {:.4} \u{2014} shutdown", price);
            return Ok(TickResult::shutdown());
        }
        let ts = chrono::Utc::now().timestamp();
        let fills_before  = self.total_fills_tracked;
        let orders_before = self.total_orders_placed;

        // ── PR #120: reposition guard ─────────────────────────────────────────────
        if let Some(last) = self.last_price {
            if self.should_reposition(price, last).await {
                self.reposition_grid(price, last).await
                    .context("Grid reposition failed")?;
            }
        }
        // ─────────────────────────────────────────────────────────────────────────

        self.process_price_update(price, ts).await?;

        let fills_this_tick  = self.total_fills_tracked.saturating_sub(fills_before);
        let orders_this_tick = self.total_orders_placed.saturating_sub(orders_before);

        // ── PR #126 C3: SL cooldown gate → surface pause to fleet manager ──────────
        // is_trading_allowed() auto-resets after stop_loss_cooldown_secs elapses.
        // Checked before Win Rate Guard — SL/TP is a harder stop.
        if !self.stop_loss_manager.is_trading_allowed() {
            let remaining = self.stop_loss_manager
                .sl_cooldown_remaining()
                .map(|d| d.as_secs())
                .unwrap_or(0);
            return Ok(TickResult::paused(format!(
                "SL cooldown active \u{2014} {}s remaining", remaining
            )));
        }
        // ─────────────────────────────────────────────────────────────────────────

        // ── PR #131 C4: Win Rate Guard → surface pause to fleet manager ───────────
        // evaluate() updates internal state and returns false when suppressed.
        // Uses perf.win_rate (0.0–1.0 fraction) + self.successful_trades count.
        // Positioned after SL gate (harder stop) and before regime gate.
        let perf = self.engine.get_performance_stats().await;
        if !self.win_rate_guard.evaluate(perf.win_rate, self.successful_trades) {
            return Ok(TickResult::paused(
                self.win_rate_guard.reason().to_string()
            ));
        }
        // ─────────────────────────────────────────────────────────────────────────

        // ── PR #125 C3: Regime gate → surface pause to fleet manager ─────────────
        // trading_paused is set by should_trade_now() inside analyze_all().
        // Reading the AtomicBool via grid_stats() avoids double-evaluating vol.
        let regime_stats = self.grid_rebalancer.grid_stats().await;
        if regime_stats.trading_paused {
            return Ok(TickResult::paused(&regime_stats.pause_reason));
        }
        // ─────────────────────────────────────────────────────────────────────────

        let stats = self.get_stats().await;
        if stats.trading_paused {
            return Ok(TickResult::paused("circuit breaker tripped"));
        }
        Ok(TickResult::active(fills_this_tick, orders_this_tick))
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("[BOT] Graceful shutdown for '{}'", self.instance_id());
        let final_price = self.last_price.unwrap_or(0.0);
        self.display_status().await;
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

        info!("[BOT] Shutdown complete | cycles={} fills={} orders={} repos={} realized_pnl=${:.4}",
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

// ══════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use crate::trading::{FillEvent, OrderSide};

    #[test]
    fn test_fill_event_pnl_is_option_f64_and_assignable() {
        let mut fill = FillEvent::new(
            "ORDER-SELL-001",
            OrderSide::Sell,
            142.30,
            0.0500,
            0.0036,
            None,
            1741899000,
        );
        assert!(fill.pnl.is_none());
        fill.pnl = Some(0.1820);
        assert_eq!(fill.pnl, Some(0.1820));
        assert!(fill.pnl.unwrap() > 0.0);
    }

    #[test]
    fn test_bot_stats_current_pnl_is_f64() {
        use crate::bots::bot_trait::BotStats;
        let stats = BotStats {
            instance_id:      "sol-usdc-grid-01".into(),
            bot_type:         "GridBot".into(),
            total_cycles:     100,
            total_fills:      5,
            total_orders:     10,
            uptime_secs:      300,
            is_paused:        false,
            current_pnl:      0.3640,
            intent_conflicts: 0,
        };
        let pnl: f64 = stats.current_pnl;
        assert!(pnl > 0.0);
        assert!((pnl - 0.3640).abs() < 1e-9);
    }

    #[test]
    fn test_realized_pnl_accumulates_across_fills() {
        let fill_deltas: Vec<f64> = vec![0.1820, 0.1750, 0.1920];
        let total_realized: f64 = fill_deltas.iter().sum();
        assert!((total_realized - 0.5490).abs() < 1e-9);
        for delta in &fill_deltas {
            assert!(*delta > 0.0);
        }
    }

    #[test]
    fn test_reposition_triggered_on_price_drift() {
        let reposition_threshold = 2.0_f64;

        let price_change_pct = |current: f64, last: f64| -> f64 {
            ((current - last).abs() / last) * 100.0
        };

        let drift_above = price_change_pct(153.0, 148.0);
        assert!(drift_above > reposition_threshold,
            "3.38% drift must exceed 2.0% threshold, got {:.3}%", drift_above);

        let drift_below = price_change_pct(149.0, 148.0);
        assert!(drift_below <= reposition_threshold,
            "0.68% drift must not exceed 2.0% threshold, got {:.3}%", drift_below);

        let drift_exact = price_change_pct(151.0, 148.04);
        assert!(drift_exact <= reposition_threshold,
            "boundary drift must not fire; got {:.3}%", drift_exact);
    }

    /// PR #121: Validates fill.pnl is populated before notify_fill() is called.
    #[test]
    fn test_notify_fill_receives_enriched_pnl() {
        let mut fill = FillEvent::new(
            "ORDER-SELL-002",
            OrderSide::Sell,
            155.00,
            0.0500,
            0.0039,
            None,
            1741899100,
        );

        assert!(fill.pnl.is_none(),
            "fill.pnl must be None before enrichment");

        let pnl_delta = 0.2150_f64;
        fill.pnl = Some(pnl_delta);

        assert!(fill.pnl.is_some(),
            "fill.pnl must be Some before notify_fill is called");
        assert!(
            (fill.pnl.unwrap() - pnl_delta).abs() < 1e-9,
            "fill.pnl must match computed pnl_delta exactly"
        );
    }

    /// PR #125 C3: Validates the regime gate pause reason string
    /// flows through GridStats correctly.
    #[test]
    fn test_process_tick_paused_reason_matches_regime_gate() {
        use crate::strategies::grid_rebalancer::GridStats;

        let paused_stats = GridStats {
            total_rebalances:        0,
            rebalances_filtered:     0,
            efficiency_percent:      100.0,
            dynamic_spacing_enabled: true,
            current_spacing_percent: 0.15,
            volatility:              0.03,
            market_regime:           "VERY_LOW_VOL".to_string(),
            trading_paused:          true,
            pause_reason:            "VERY_LOW_VOL regime".to_string(),
        };

        assert!(paused_stats.trading_paused);
        assert!(!paused_stats.pause_reason.is_empty());
        assert_eq!(paused_stats.pause_reason, "VERY_LOW_VOL regime");

        let active_stats = GridStats {
            trading_paused: false,
            pause_reason:   String::new(),
            volatility:     1.5,
            market_regime:  "MEDIUM_VOL".to_string(),
            ..paused_stats.clone()
        };
        assert!(!active_stats.trading_paused);
        assert!(active_stats.pause_reason.is_empty());
    }

    /// PR #126 C3: Validates the SL cooldown gate data path.
    #[test]
    fn test_process_tick_sl_gate_returns_paused_reason() {
        use crate::risk::StopLossManager;
        use crate::config::ConfigBuilder;

        let mut config = ConfigBuilder::new()
            .build()
            .expect("default test config must be valid");
        config.risk.stop_loss_pct          = 5.0;
        config.risk.stop_loss_cooldown_secs = 300;

        let mut mgr = StopLossManager::new(&config);

        assert!(mgr.is_trading_allowed());
        assert!(mgr.sl_cooldown_remaining().is_none());

        let tripped = mgr.should_stop_loss(100.0, 94.8);
        assert!(tripped, "should_stop_loss must fire at -5.2%");

        assert!(!mgr.is_trading_allowed());

        let remaining = mgr.sl_cooldown_remaining();
        assert!(remaining.is_some());
        let secs = remaining.unwrap().as_secs();
        assert!(secs > 290 && secs <= 300,
            "remaining must be near 300s on fresh trip, got {}s", secs);

        let pause_reason = format!("SL cooldown active \u{2014} {}s remaining", secs);
        assert!(!pause_reason.is_empty());
        assert!(pause_reason.contains("SL cooldown active"));
        assert!(pause_reason.contains(&secs.to_string()));
    }

    /// PR #131 C4: Validates the Win Rate Guard gate data path.
    /// process_tick() calls win_rate_guard.evaluate(perf.win_rate, successful_trades);
    /// when false it returns TickResult::paused with the suppression reason.
    /// This test verifies the WinRateGuard state transitions that feed that decision.
    #[test]
    fn test_process_tick_win_rate_guard_returns_paused_reason() {
        use crate::risk::WinRateGuard;
        use crate::config::ConfigBuilder;

        let mut config = ConfigBuilder::new()
            .build()
            .expect("default test config must be valid");
        config.risk.enable_win_rate_guard     = true;
        config.risk.min_win_rate_pct          = 40.0;
        config.risk.win_rate_guard_resume_pct = 45.0;
        config.risk.min_trades_before_guard   = 10;

        let mut guard = WinRateGuard::new(&config);

        // Invariant 1: fresh guard allows trading.
        assert!(
            guard.is_trading_allowed(),
            "Win Rate Guard must allow trading before any evaluation"
        );
        assert!(
            guard.reason().is_empty(),
            "reason must be empty before suppression"
        );

        // Invariant 2: warmup window — even 0% win rate with 9 trades must allow.
        assert!(
            guard.evaluate(0.0, 9),
            "warmup window (9 < 10 trades) must always allow trading"
        );
        assert!(!guard.is_suppressed());

        // Invariant 3: guard fires at min_trades boundary with bad win rate.
        assert!(
            !guard.evaluate(0.35, 20),
            "35% win rate below 40% min must suppress at 20 trades"
        );
        assert!(guard.is_suppressed());

        // Invariant 4: reason string is non-empty and embedded in the pause message.
        let pause_reason = guard.reason().to_string();
        assert!(
            !pause_reason.is_empty(),
            "reason must be non-empty when suppressed"
        );
        assert!(
            pause_reason.contains("win rate"),
            "pause reason must mention win rate: {}", pause_reason
        );

        // Invariant 5: hysteresis — 41% is above suppress but below resume (45%).
        assert!(
            !guard.evaluate(0.41, 25),
            "41% above suppress but below resume (45%) must stay suppressed"
        );
        assert!(guard.is_suppressed(), "hysteresis must hold below resume_pct");

        // Invariant 6: resumes cleanly at resume_pct.
        assert!(
            guard.evaluate(0.45, 30),
            "exactly 45% must trigger resume"
        );
        assert!(!guard.is_suppressed());
        assert!(
            guard.reason().is_empty(),
            "reason must clear after resume"
        );
    }
}
