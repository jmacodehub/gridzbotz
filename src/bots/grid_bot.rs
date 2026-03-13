//! ═════════════════════════════════════════════════════════════════════════
//! GRID BOT V6.4 — FEE-RECONCILIATION WIRED (PR #107)
//!
//! PR #107 Commit 2: Replace hardcoded spacing bounds in GridRebalancerConfig
//!   Before: max_spacing: 0.0075, min_spacing: 0.001  (hardcoded)
//!   After:  max_spacing: config.trading.max_grid_spacing_pct
//!           min_spacing: config.trading.min_grid_spacing_pct
//!   Defaults are identical — zero behaviour change on all 46 TOMLs.
//!   Per-pair configs can now override via:
//!     [trading]
//!     max_grid_spacing_pct = 0.010   # 1.0%
//!     min_grid_spacing_pct = 0.002   # 0.2%
//!
//! PR #107 Commit 4: Thread fee_usdc into mark_buy/sell_filled
//!   Source: fees.taker_fee_bps from FeesConfig (single source of truth)
//!   Helpers: taker_fee_fraction() = bps/10_000 (for multiplication)
//!            taker_fee_percent()  = bps/100    (for display/logging)
//!   Formula: fee_usdc = fill_price * fill_size * taker_fee_fraction()
//!   mark_buy_filled(lid, fee_usdc)  — accumulates on GridLevel.fees_paid
//!   mark_sell_filled(lid, fee_usdc) — accumulates + computes NET P&L
//!   GridStateTracker.total_fees_paid() logged at stats heartbeat interval.
//!   Paper mode uses taker_fee_bps=0.0 by default — zero behaviour change.
//!
//! PR #107 Commit 5: Compile fixes
//!   - taker_fee_pct (phantom field) → taker_fee_fraction()/taker_fee_percent()
//!   - \u{0394} → \u{0394} (invalid Rust Unicode escapes)
//!   - max/min_grid_spacing_pct added to test_config() TradingConfig literals
//!
//! PR #107 Commit 6: Restore Bot trait contract
//!   - tick(price, ts) → process_tick(&mut self)  [trait requires no args]
//!   - TickResult::Continue → TickResult::active/paused/shutdown constructors
//!   - async fn stats() → fn stats() (sync — trait is sync)
//!   - BotStats fields restored: instance_id, bot_type, total_fills,
//!     total_orders, uptime_secs, is_paused, current_pnl, intent_conflicts
//!   - name() + instance_id() methods restored (were dropped in prior commit)
//!   - shutdown() restored with Telegram send_shutdown + display_strategy_performance
//!
//! PR #107 Commit 7: Wire real GridStateTracker V4.3 API
//!   - count()                  → level_counts(); used = .0 + .1
//!   - get_cancellable_levels() → get_all_levels() + filter Pending/BuyFilled
//!   - get_level(id)            → get_all_levels().find(|l| l.id == id)
//!   - mark_cancelled(id)       → cancel_level(id)
//!   - update_level(level)      → removed (tracker owns in-place mutation)
//!   - create_level() → u64 ID  → level.id / set_buy/sell_order removed
//!   - Type casts: profitable/unprofitable_trades as u64, adjustment_count as u32
//!
//! PR #105 Commit 3: GridRebalancer switched to .add_execution_only()
//!   (avoids WMA deadlock — see V6.3 header for full root-cause)
//! PR #102 Commit 3: level_id mark-filled + CB delta P&L fix.
//! PR #101 Commit 2: TelegramBot wired into GridBot.
//! PR #99  Commit 3b: wma_confidence_threshold fully wired end-to-end.
//! PR #98  Commit 2b-ii: WMA voter P&L attribution wired.
//! PR #94  (Commit 6): GridBotStats observability.
//! PR #93: CircuitBreaker wired.
//!
//! March 13, 2026 - V6.4: fee-reconciliation wired (PR #107 C2+C4+C5+C6+C7) 💰
//! March 12, 2026 - V6.3: GridRebalancer execution-only (PR #105 C3) 🔥
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
use crate::trading::grid_level::GridLevelStatus;
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
        info!("[BOT-V6.4] Initializing GridBot V6.4...");
        info!("[BOT-V6.4] WMAConfGate:      {:.2} (TOML-driven, PR #99)",
              config.strategies.wma_confidence_threshold);
        info!("[BOT-V6.4] OptimizerCadence: {} cycles",
              config.trading.optimizer_interval_cycles);
        info!("[BOT-V6.4] ConsensusSizing:  {} | multiplier={:.2}x",
              if config.trading.enable_smart_position_sizing { "ACTIVE" } else { "disabled" },
              config.trading.signal_size_multiplier);
        // PR #107 C2: log config-driven spacing bounds
        info!("[BOT-V6.4] DynSpacingBounds: {:.5}\u{2013}{:.5} (PR #107 C2)",
              config.trading.min_grid_spacing_pct,
              config.trading.max_grid_spacing_pct);
        // PR #107 C4+C5: taker_fee_percent() = bps/100 for display
        info!("[BOT-V6.4] TakerFee:         {:.4}% ({:.1} bps) (PR #107 C4)",
              config.fees.taker_fee_percent(),
              config.fees.taker_fee_bps);

        let telegram = TelegramBot::from_env();

        // PR #107 Commit 2: max_spacing / min_spacing now sourced from
        // TradingConfig instead of hardcoded literals.
        // Defaults (0.0075 / 0.001) are identical — no behaviour change.
        let grid_config = GridRebalancerConfig {
            grid_spacing:                   config.trading.grid_spacing_percent / 100.0,
            order_size:                     config.trading.min_order_size,
            min_usdc_balance:               config.trading.min_usdc_reserve,
            min_sol_balance:                config.trading.min_sol_reserve,
            enabled:                        config.strategies.grid.enabled,
            enable_dynamic_spacing:         config.trading.enable_dynamic_grid,
            enable_fee_filtering:           config.trading.enable_fee_optimization,
            volatility_window_seconds:      config.trading.volatility_window as u64,
            max_spacing:                    config.trading.max_grid_spacing_pct,  // was 0.0075
            min_spacing:                    config.trading.min_grid_spacing_pct,  // was 0.001
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

        // PR #105 Commit 3: GridRebalancer registered as execution-only.
        // It MUST NOT be a WMA voter — analyze() always returns Signal::Hold
        // (confidence=0.0), which permanently blocks the wma_conf_gate.
        // Use .add_execution_only() so it still receives on_fill() callbacks
        // but has no WMA performance slot.
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

        info!("[BOT-V6.4] {} strategies loaded ({} WMA voters, conf_gate={:.2})",
              manager.strategies.len(),
              manager.wma_engine.registered_count(),
              manager.wma_engine.min_confidence());

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);
        let circuit_breaker    = CircuitBreaker::new(&config);

        info!("[BOT-V6.4] GridBot V6.4 initialization complete");

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
        info!("[BOT] V6.4 GRID INIT - awaiting live price...");

        let initial_price = self.feed.latest_price().await;
        if initial_price <= 0.0 {
            bail!("Invalid initial price ${:.2} - cannot initialize grid", initial_price);
        }
        info!("[BOT] Live price received: ${:.4}", initial_price);

        self.place_grid_orders(initial_price).await
            .context("Initial grid placement failed")?;
        self.grid_initialized = true;
        self.last_price = Some(initial_price);

        // C7: level_counts() returns (pending, buy_filled, completed)
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

        // C7: get_all_levels() + filter Pending or BuyFilled — these are
        // the levels that still have open orders that can be cancelled.
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
            // Re-fetch the level snapshot to get order IDs
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
                // C7: mark_cancelled → cancel_level
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

            // C7: create_level() returns u64 ID — not a GridLevel struct.
            // The tracker owns the level; we work with the ID only.
            let level_id = self.grid_state.create_level(buy_price, sell_price, effective_size).await;

            if let Some(registry) = &self.intent_registry {
                let key = (pair.clone(), level_id);
                match registry.entry(key) {
                    dashmap::Entry::Occupied(e) => {
                        self.intent_conflicts += 1;
                        warn!("[INTENT] Level {} owned by '{}' — skipping", level_id, e.get());
                        // Cancel the just-created level so it doesn't linger
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

            // PR #102: Snapshot P&L *before* the fill loop so we can
            // compute per-fill deltas for CB.record_trade().
            let pnl_before = self.grid_state.total_realized_pnl().await;

            // PR #107 C4+C5: taker_fee_fraction() = bps/10_000 — correct unit
            // for direct multiplication. taker_fee_pct was a phantom field;
            // the canonical source is fees.taker_fee_bps via helpers.
            // Paper mode has taker_fee_bps=0.0 by default — zero behaviour change.
            let taker_fee_fraction = self.config.fees.taker_fee_fraction();

            for fill in &filled_orders {
                let is_buy    = fill.side == OrderSide::Buy;
                let fill_size = self.adaptive_optimizer.current_position_size;

                // fee = fill_price * fill_size * (taker_fee_bps / 10_000)
                let fee_usdc = price * fill_size * taker_fee_fraction;

                if let Some(lid) = fill.level_id {
                    if is_buy {
                        self.grid_state.mark_buy_filled(lid, fee_usdc).await;
                    } else {
                        self.grid_state.mark_sell_filled(lid, fee_usdc).await;
                    }
                }

                let pnl_after = self.grid_state.total_realized_pnl().await;
                let pnl_delta = pnl_after - pnl_before;

                self.total_fills_tracked += 1;

                info!("[FILL] #{}: {} {} @ ${:.4} | size:{:.4} | fee:${:.4} | \u{0394} P&L:${:.4} | ts:{}",
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
                        debug!("[WMA-ATTR] SELL attributed to {} voters | \u{0394} P&L:${:.4}",
                               voters.len(), pnl_delta);
                    }
                }
            }
        }

        self.last_known_pnl = wallet.pnl_usdc(price);
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
            // PR #107 C4: log total fees paid by GridStateTracker
            // C5: use taker_fee_percent() (bps/100) for display — not the
            //     now-renamed taker_fee_fraction local (which is bps/10_000).
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
            pnl_usdc:                wallet.pnl_usdc(current_price),
            roi_percent:             wallet.roi(current_price),
            win_rate:                perf_stats.win_rate,
            total_fees:              perf_stats.total_fees,
            trading_paused:          cb_status.is_tripped,
            // C7 type casts: EnhancedMetrics stores these as usize; GridBotStats expects u64
            profitable_trades:       self.enhanced_metrics.profitable_trades as u64,
            unprofitable_trades:     self.enhanced_metrics.unprofitable_trades as u64,
            max_drawdown:            self.enhanced_metrics.max_drawdown,
            signal_execution_ratio:  self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency:         self.enhanced_metrics.grid_efficiency,
            current_spacing_percent: self.adaptive_optimizer.current_spacing_percent,
            current_position_size:   self.adaptive_optimizer.current_position_size,
            // C7 type cast: AdaptiveOptimizer.adjustment_count is u64; GridBotStats expects u32
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

        println!("\n╔══════════════════════════════════════════╗");
        println!(  "║     GRID BOT V6.4 STATUS (PR #107)       ║");
        println!(  "╚══════════════════════════════════════════╝");
        println!("  Instance:          {}",   self.config.bot.instance_name());
        println!("  Uptime:            {}s",  uptime_secs);
        println!("  Cycles:            {}",   stats.total_cycles);
        println!("  Fills Tracked:     {}",   stats.total_fills_tracked);
        println!("  Successful Trades: {}",   stats.successful_trades);
        println!("  Grid Repositions:  {}",   stats.grid_repositions);
        println!("  Open Orders:       {}",   stats.open_orders);
        println!();
        println!("  Portfolio Value:   ${:.2}", stats.total_value_usdc);
        println!("  P&L:               ${:.4}", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);
        println!("  Win Rate:          {:.1}%", stats.win_rate * 100.0);
        // PR #107 C4+C5: taker_fee_percent() = bps/100 for display
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
        println!("══════════════════════════════════════════════");
    }

    pub async fn display_strategy_performance(&self) {
        self.manager.display_stats();
    }
}

// ═════════════════════════════════════════════════════════════════════════
// GridBotStats
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

// ═════════════════════════════════════════════════════════════════════════
// impl Bot for GridBot  — PR #107 C6: trait contract restored
// ═════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Bot for GridBot {
    // C6-fix-5: name() + instance_id() were dropped in prior commit — restored.
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

    // C6-fix-1+2: tick(price, ts) → process_tick(&mut self)
    //   Bot trait requires process_tick() with NO args — bot owns its feed.
    //   TickResult::Continue does not exist — use active/paused/shutdown constructors.
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

        info!("[BOT] Shutdown complete | cycles={} fills={} orders={} repos={} pnl=${:.2}",
              self.total_cycles, self.total_fills_tracked, self.total_orders_placed,
              self.grid_repositions, self.last_known_pnl);
        Ok(())
    }

    // C6-fix-3+4: async fn stats() → fn stats() (sync, no await).
    //   Bot trait declares: fn stats(&self) -> BotStats  (NOT async).
    //   BotStats fields restored: instance_id, bot_type, total_fills,
    //   total_orders, uptime_secs, is_paused, current_pnl, intent_conflicts.
    //   Removed phantom fields that don't exist on BotStats struct.
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
