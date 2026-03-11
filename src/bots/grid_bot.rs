//! ═════════════════════════════════════════════════════════════════════════
//! GRID BOT V6.0 - ELITE AUTONOMOUS TRADING ORCHESTRATOR
//!
//! PR #98 Commit 2b-ii: WMA voter P&L attribution wired.
//!    process_price_update(): snapshot total_realized_pnl once before
//!    the fill loop to avoid repeated async calls.
//!    SELL fills only: get_last_wma_voters() → record_fill_for_wma()
//!    for each voter that cleared the confidence gate this tick.
//!    BUY fills skipped — no realized P&L until round-trip completes.
//!    TODO(tech-debt): replace session-cumulative pnl snapshot with
//!    per-fill delta once level-P&L API is available (follow-up PR).
//!
//! PR #94 (Commit 6): GridBotStats observability — Items 1 + 2 telemetry.
//!    GridBot struct: `orders_filtered_session: u64` added.
//!    place_grid_orders(): accumulates into self.orders_filtered_session.
//!    GridBotStats: 5 new fields:
//!      last_signal_strength       — consensus strength at last tick
//!      orders_filtered_session    — orders skipped by SmartFeeFilter (session)
//!      fee_filter_total_checked   — total evaluated by fee filter
//!      fee_filter_total_passed    — total that passed the fee filter
//!      fee_filter_total_blocked   — total blocked by fee filter
//!    get_stats(): populates all 5 from self + grid_rebalancer.fee_filter_stats()
//!    display_summary(): adds [SIGNAL SIZING] + [FEE FILTER] sections
//!    Zero behaviour change — trading logic untouched.
//!
//! PR #94 (Commit 5b): Consensus-Signal-Driven Position Sizing WIRED.
//!    `last_signal_strength: f64` field added to GridBot.
//!    process_price_update() caches signal.strength() after analyze_all().
//!    place_grid_orders() computes effective_size:
//!      multiplier   = 1.0 + last_signal_strength * (signal_size_multiplier - 1.0)
//!      effective_sz = (order_size * multiplier).clamp(min_order_sol, max_position_size)
//!    Flag=false OR signal_size_multiplier=1.0 (default) → effective_size == order_size.
//!    Zero behaviour change for all 46 existing TOMLs.
//!
//! PR #94 (Commit 5a): signal_size_multiplier added to TradingConfig.
//!    Default 1.0, validation [0.5, 3.0] when enable_smart_position_sizing=true.
//!
//! PR #94 (Commit 4): SmartFeeFilter call site wired.
//!    place_grid_orders() now gates each level through
//!    rebalancer.should_place_order(side, price, min_order_size, &stats)
//!    so the full P&L simulation (fees + slippage + market impact) fires
//!    on every candidate order. Falls through to legacy spread gate when
//!    enable_fee_filtering = false (zero breaking change).
//!
//! PR #94 (Commit 2): OPT-1 - optimizer_interval_cycles wired from config
//!    Was: `const OPTIMIZATION_INTERVAL_CYCLES: u64 = 50` (hardcoded)
//!    Now: `self.config.trading.optimizer_interval_cycles` (TOML-driven)
//!    Zero behaviour change — default in config/mod.rs is 50.
//!
//! PR #93 FIXES:
//! [fix] P0 #1 (SAFETY): CircuitBreaker wired into GridBot with real NAV P&L.
//!
//! PR #92 FIXES:
//! [fix] P0 #2 (CORRECTNESS): process_price_update() fill-side detection fixed.
//! [fix] P0 #3 (UX): Win Rate display guard added.
//!
//! V5.8 CHANGES (PR #86 - Multi-Bot Orchestrator / GAP-3):
//! [ok] intent_registry: Option<IntentRegistry> field - injected by Orchestrator
//! [ok] set_intent_registry() impl - wires the shared DashMap conflict guard
//! [ok] place_grid_orders(): DashMap::entry() atomic check before each level
//! [ok] intent_conflicts: u64 counter - surfaced in BotStats via stats()
//! [ok] Solo path: intent_registry = None - zero behavior change, zero cost
//!
//! March 11, 2026 - V6.0: WMA voter P&L attribution wired (PR #98 Commit 2b-ii) 🤝
//! March 11, 2026 - V6.0: fix FeeFilterStats field names (PR #94 hotfix) 🔧
//! March 11, 2026 - V6.0: GridBotStats observability (PR #94 Commit 6) 📊
//! March 11, 2026 - V6.0: Consensus sizing wired (PR #94 Commit 5b) 📊
//! March 11, 2026 - V5.9: signal_size_multiplier config (PR #94 Commit 5a) 📐
//! March 10, 2026 - V5.9: CircuitBreaker wired (PR #93) 🛑
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
    TradingEngine,
    OrderSide,
    GridStateTracker,
    EnhancedMetrics,
    AdaptiveOptimizer,
    PriceFeed,
};
use crate::risk::CircuitBreaker;
use crate::config::Config;

// OPTIMIZATION_INTERVAL_CYCLES removed (PR #94 OPT-1).
// Use self.config.trading.optimizer_interval_cycles (TOML-driven, default 50).

// ═════════════════════════════════════════════════════════════════════════
// GRID BOT STRUCT
// ═════════════════════════════════════════════════════════════════════════

pub struct GridBot {
    pub manager:            StrategyManager,
    pub engine:             Arc<dyn TradingEngine + Send + Sync>,
    pub config:             Config,
    pub grid_state:         GridStateTracker,
    pub enhanced_metrics:   EnhancedMetrics,
    pub adaptive_optimizer: AdaptiveOptimizer,
    /// PR #93: CircuitBreaker wired at bot layer — guards both paper and real
    /// execution. Initialized from config; peak_balance self-anchors on first
    /// record_trade() call so no price is needed at construction time.
    pub circuit_breaker:    CircuitBreaker,
    /// PR #94 Commit 4: GridRebalancer ref for should_place_order() filter.
    /// Kept separately so place_grid_orders() can call the fee filter gate
    /// without routing through StrategyManager fan-out.
    pub grid_rebalancer:    GridRebalancer,
    /// PR #94 Commit 5b: Cached signal strength from last consensus tick.
    /// Updated in process_price_update() after every analyze_all() call.
    /// Used by place_grid_orders() to scale order size when
    /// enable_smart_position_sizing = true.
    /// Initialized to 0.0 (Hold equivalent — flat sizing until first analysis).
    last_signal_strength:   f64,
    /// PR #94 Commit 6: Session-lifetime count of orders skipped by
    /// SmartFeeFilter across all place_grid_orders() calls.
    /// Accumulated from the local `orders_filtered` counter per placement.
    /// Surfaced in GridBotStats so orchestrator / Telegram / Supabase can
    /// consume it without reaching into GridBot internals.
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
    /// Shared intent registry for multi-bot conflict detection (PR #86).
    intent_registry:        Option<IntentRegistry>,
    /// Real conflict events detected this session.
    intent_conflicts:       u64,
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
        info!("[BOT-V6.0] Initializing GridBot V6.0...");
        info!("[BOT-V6.0] Engine:          Injected by main.rs (Paper or Real)");
        info!("[BOT-V6.0] PriceFeed:       Owned via Arc - process_tick() autonomous");
        info!("[BOT-V6.0] Bot Trait:       IMPLEMENTED + DISPATCHED (PR #84+#85+#86)");
        info!("[BOT-V6.0] CircuitBreaker:  WIRED (PR #93) - bot-layer protection active");
        info!("[BOT-V6.0] SmartFeeFilter:  WIRED (PR #94 Commit 4) - per-order P&L gate active");
        info!("[BOT-V6.0] ConsensusSizing: {} (PR #94 Commit 5b) | multiplier={:.2}x",
              if config.trading.enable_smart_position_sizing { "ACTIVE" } else { "disabled" },
              config.trading.signal_size_multiplier);
        info!("[BOT-V6.0] FeeFilterStats:  WIRED into GridBotStats (PR #94 Commit 6)");
        info!("[BOT-V6.0] WMAAttribution:  WIRED (PR #98 Commit 2b-ii) - SELL fills → WMA P&L");
        info!("[BOT-V6.0] OptimizerCadence: {} cycles (TOML-driven, PR #94 OPT-1)",
              config.trading.optimizer_interval_cycles);

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
            grid_config.clone(),
            config.fees.clone(),
        ).context("Failed to create GridRebalancer")?;

        let grid_rebalancer_for_manager = GridRebalancer::with_fees(
            grid_config,
            config.fees.clone(),
        ).context("Failed to create GridRebalancer for StrategyManager")?;

        let analytics_ctx = AnalyticsContext::default();
        let (_manager, _weights) = StrategyRegistryBuilder::new()
            .add(
                grid_rebalancer_for_manager,
                config.strategies.grid.weight,
            )
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
            .build(analytics_ctx);

        let manager = _manager;

        info!("[BOT-V6.0] {} strategies loaded via StrategyRegistryBuilder",
              manager.strategies.len());

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);
        let circuit_breaker    = CircuitBreaker::new(&config);

        info!("[BOT-V6.0] GridBot V6.0 initialization complete");

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
        })
    }

    async fn pre_init_hook(&mut self) -> Result<()> {
        info!("[BOT] Async pre-init hook complete");
        Ok(())
    }

    async fn initialize_with_price(&mut self) -> Result<()> {
        info!("[BOT] V6.0 GRID INIT - awaiting live price...");

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
        if self.last_price.is_none() {
            trace!("No last price - skipping reposition check");
            return false;
        }
        if let Some(last_reposition) = self.last_reposition_time {
            let cooldown_secs = self.config.trading.rebalance_cooldown_secs;
            let elapsed = last_reposition.elapsed().as_secs();
            if elapsed < cooldown_secs {
                trace!("Reposition cooldown: {}s elapsed, {}s required", elapsed, cooldown_secs);
                return false;
            }
        }
        let price_change_pct = ((current_price - last_price).abs() / last_price) * 100.0;
        let threshold = self.config.trading.reposition_threshold;
        let should = price_change_pct > threshold;
        if should {
            debug!("[BOT] Reposition triggered: {:.3}% change > {:.3}% threshold",
                   price_change_pct, threshold);
        }
        should
    }

    pub async fn reposition_grid(&mut self, current_price: f64, last_price: f64) -> Result<()> {
        if !self.grid_initialized {
            warn!("[BOT] Grid not initialized - emergency init at ${:.4}", current_price);
            self.place_grid_orders(current_price).await
                .context("Emergency grid initialization failed")?;
            self.grid_initialized = true;
            let total_levels = self.config.trading.grid_levels as usize;
            let used_levels  = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);
            info!("[BOT] Emergency grid init complete");
            return Ok(());
        }

        info!("[BOT] Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);
        let reposition_start = Instant::now();

        let filled_buys = self.grid_state.get_levels_with_filled_buys().await;
        if !filled_buys.is_empty() {
            warn!("[BOT] {} levels have filled buys - preserving sell orders!", filled_buys.len());
        }

        let trading_pair = self.config.trading_pair();
        let cancellable  = self.grid_state.get_cancellable_levels().await;
        let mut cancelled_count = 0;

        for level_id in cancellable {
            if let Some(level) = self.grid_state.get_level(level_id).await {
                if let Some(buy_id) = &level.buy_order_id {
                    match self.engine.cancel_order(buy_id).await {
                        Ok(_)  => { cancelled_count += 1; }
                        Err(e) => { warn!("[BOT] Failed to cancel buy {}: {}", buy_id, e); }
                    }
                }
                if let Some(sell_id) = &level.sell_order_id {
                    match self.engine.cancel_order(sell_id).await {
                        Ok(_)  => { cancelled_count += 1; }
                        Err(e) => { warn!("[BOT] Failed to cancel sell {}: {}", sell_id, e); }
                    }
                }
                self.grid_state.mark_cancelled(level_id).await;
                if let Some(registry) = &self.intent_registry {
                    registry.remove(&(trading_pair.clone(), level_id));
                }
            }
        }
        if cancelled_count > 0 {
            info!("[BOT] Cancelled {} orders", cancelled_count);
        }

        self.place_grid_orders(current_price).await?;
        self.grid_repositions += 1;
        self.last_reposition_time = Some(Instant::now());
        let total_levels = self.config.trading.grid_levels as usize;
        let used_levels  = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);
        info!("[BOT] Grid repositioned in {}ms", reposition_start.elapsed().as_millis());
        Ok(())
    }

    async fn place_grid_orders(&mut self, current_price: f64) -> Result<()> {
        let grid_spacing  = self.adaptive_optimizer.current_spacing_percent;
        let order_size    = self.adaptive_optimizer.current_position_size;
        let num_levels    = self.config.trading.grid_levels;
        let min_order_sol = self.config.trading.min_order_size;
        let pair = self.config.trading_pair();

        // PR #94 Commit 5b: consensus-signal-driven position sizing.
        let effective_size = if self.config.trading.enable_smart_position_sizing {
            let multiplier = 1.0
                + self.last_signal_strength
                * (self.config.trading.signal_size_multiplier - 1.0);
            (order_size * multiplier)
                .clamp(min_order_sol, self.config.trading.max_position_size)
        } else {
            order_size
        };

        self.grid_rebalancer.update_price(current_price).await
            .unwrap_or_else(|e| warn!("[BOT] Rebalancer price update failed: {}", e));
        let stats = self.grid_rebalancer.grid_stats().await;

        if self.config.trading.enable_smart_position_sizing {
            debug!(
                "[BOT] Grid params: {} levels @ {:.3}% spacing | \
                 base={:.3} SOL effective={:.3} SOL (strength={:.3} mult={:.3}x)",
                num_levels, grid_spacing * 100.0, order_size, effective_size,
                self.last_signal_strength,
                1.0 + self.last_signal_strength
                    * (self.config.trading.signal_size_multiplier - 1.0),
            );
        } else {
            debug!("[BOT] Grid params: {} levels @ {:.3}% spacing, {:.3} SOL/order",
                   num_levels, grid_spacing * 100.0, order_size);
        }

        let mut orders_placed   = 0;
        let mut orders_failed   = 0;
        let mut orders_filtered = 0;
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
                        warn!(
                            "[INTENT] Level {} at ${:.4} owned by '{}' - skipping (conflicts: {})",
                            level.id, buy_price, e.get(), self.intent_conflicts
                        );
                        continue;
                    }
                    dashmap::Entry::Vacant(e) => {
                        e.insert(self.config.bot.instance_name().to_string());
                    }
                }
            }

            if !self.grid_rebalancer
                .should_place_order(OrderSide::Buy, buy_price, min_order_sol, &stats)
                .await
            {
                orders_filtered += 1;
                trace!("[FEE-FILTER] Buy skipped @ ${:.4} (level {})", buy_price, level.id);
                continue;
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Buy, buy_price, effective_size, Some(level.id)
            ).await {
                Ok(id) => {
                    level.set_buy_order(id);
                    orders_placed += 1;
                    self.total_orders_placed += 1;
                }
                Err(e) => {
                    warn!("[BOT] Failed buy @ ${:.4}: {}", buy_price, e);
                    orders_failed += 1;
                    continue;
                }
            }

            if !self.grid_rebalancer
                .should_place_order(OrderSide::Sell, sell_price, min_order_sol, &stats)
                .await
            {
                orders_filtered += 1;
                trace!("[FEE-FILTER] Sell skipped @ ${:.4} (level {})", sell_price, level.id);
                self.grid_state.update_level(level).await;
                continue;
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Sell, sell_price, effective_size, Some(level.id)
            ).await {
                Ok(id) => {
                    level.set_sell_order(id);
                    orders_placed += 1;
                    self.total_orders_placed += 1;
                }
                Err(e) => {
                    warn!("[BOT] Failed sell @ ${:.4}: {}", sell_price, e);
                    orders_failed += 1;
                }
            }
            self.grid_state.update_level(level).await;
        }

        self.orders_filtered_session += orders_filtered as u64;

        info!("[BOT] Placed {} orders ({} pairs), {} filtered, {} failed",
              orders_placed, buy_levels.min(sell_levels), orders_filtered, orders_failed);
        Ok(())
    }

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        if !self.circuit_breaker.is_trading_allowed() {
            debug!("[CB] Trading halted - skipping tick at ${:.4}", price);
            return Ok(());
        }

        self.total_cycles += 1;
        self.last_price = Some(price);
        self.enhanced_metrics.update_price_range(price);
        trace!("[BOT] Processing price ${:.4} (cycle {})", price, self.total_cycles);

        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Strategy consensus failed")?;

        self.last_signal_strength = signal.strength();

        self.enhanced_metrics.record_signal(true);
        trace!("[BOT] Signal: {} (strength={:.3})", signal.display(), self.last_signal_strength);

        let filled_orders = self.engine.process_price_update(price).await
            .context("Engine tick failed")?;

        for fill in &filled_orders {
            self.manager.notify_fill(fill);
        }

        let wallet  = self.engine.get_wallet().await;
        let new_nav = wallet.total_value_usdc(price);

        if !filled_orders.is_empty() {
            info!("[BOT] {} orders filled at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            // PR #98 Commit 2b-ii: snapshot realized P&L once for the whole
            // fill batch — avoids N async calls inside the loop.
            //
            // Attribution logic:
            //   SELL fills only — a SELL completes the buy→sell round-trip,
            //   which is when grid P&L is realized. BUY fills have no realized
            //   P&L yet; attributing them would corrupt WMA win-rate tracking.
            //
            // P&L source: grid_state.total_realized_pnl() is the session
            // cumulative. Positive session = WMA win; negative = loss.
            // TODO(tech-debt): replace with per-fill delta once level-P&L
            // API is available (follow-up PR after #98).
            let realized_pnl_snapshot = self.grid_state.total_realized_pnl().await;

            for fill in &filled_orders {
                let is_buy    = fill.side == OrderSide::Buy;
                let pnl       = realized_pnl_snapshot;
                let fill_size = self.adaptive_optimizer.current_position_size;
                self.total_fills_tracked += 1;
                info!("[FILL] #{}: {} {} @ ${:.4} | size: {:.4} | P&L: ${:.2} | ts: {}",
                      self.total_fills_tracked,
                      if is_buy { "BUY" } else { "SELL" },
                      fill.order_id, price, fill_size, pnl, timestamp);
                self.enhanced_metrics.record_trade(is_buy, pnl, timestamp);
                self.circuit_breaker.record_trade(pnl, new_nav);

                // PR #98 Commit 2b-ii: attribute P&L to WMA voters on SELL fills.
                // SELL = grid round-trip complete — real P&L event.
                // BUY fills skipped: open position, no realized P&L yet.
                if fill.side == OrderSide::Sell {
                    let voters: Vec<String> = self.manager
                        .get_last_wma_voters()
                        .to_vec();
                    for voter in &voters {
                        self.manager.record_fill_for_wma(voter, realized_pnl_snapshot);
                    }
                    if !voters.is_empty() {
                        debug!(
                            "[WMA-ATTR] SELL fill attributed to {} voters | P&L snapshot: ${:.4}",
                            voters.len(), realized_pnl_snapshot
                        );
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
                info!("[OPT] Applied: {} | spacing={:.3}% | size={:.3} SOL",
                      result.reason, result.new_spacing * 100.0, result.new_position_size);
            } else {
                debug!("[OPT] No adjustment: {} | spacing={:.3}% size={:.3} SOL",
                       result.reason,
                       result.new_spacing       * 100.0,
                       result.new_position_size);
            }
            self.last_optimization_cycle = self.total_cycles;
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
            last_signal_strength:     self.last_signal_strength,
            orders_filtered_session:  self.orders_filtered_session,
            fee_filter_total_checked: fee_checked,
            fee_filter_total_passed:  fee_passed,
            fee_filter_total_blocked: fee_blocked,
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats  = self.get_stats().await;
        let border = "=".repeat(60);
        println!("\n{}", border);
        println!("   [BOT] GRID BOT V6.0 - STATUS REPORT");
        println!("{}", border);
        println!("\n[PERFORMANCE]");
        println!("  Total Cycles:      {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions:  {}", stats.grid_repositions);
        println!("  Open Orders:       {}", stats.open_orders);
        println!("  Fills Tracked:     {}", stats.total_fills_tracked);
        println!("  Orders Placed:     {}", self.total_orders_placed);
        println!("  Intent Conflicts:  {}", stats.intent_conflicts);
        println!("  Optimizer Cadence: {} cycles",
                 self.config.trading.optimizer_interval_cycles);
        if self.config.trading.enable_smart_position_sizing {
            let live_mult = 1.0
                + self.last_signal_strength
                * (self.config.trading.signal_size_multiplier - 1.0);
            println!("  Signal Strength:   {:.3} | sizing {:.3}x (max {:.2}x)",
                     self.last_signal_strength,
                     live_mult,
                     self.config.trading.signal_size_multiplier);
        }
        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl   = self.grid_state.total_realized_pnl().await;
        println!("\n[GRID]");
        println!("  Active Levels:     {}", grid_levels);
        println!("  Filled Buys:       {}", filled_buys);
        println!("  Realized P&L:      ${:.2}", total_pnl);
        if let Some(ffs) = self.grid_rebalancer.fee_filter_stats() {
            println!("\n[FEE FILTER]");
            println!("  Total Checked:     {}", ffs.total_checks);
            println!("  Passed:            {}", ffs.trades_passed);
            println!("  Blocked:           {}", ffs.trades_filtered);
            println!("  Pass Rate:         {:.1}%",
                     if ffs.total_checks > 0 {
                         ffs.trades_passed as f64 / ffs.total_checks as f64 * 100.0
                     } else { 100.0 });
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
            let remaining = cb.cooldown_remaining
                .map(|d| format!("{}s remaining", d.as_secs()))
                .unwrap_or_else(|| "resetting".to_string());
            println!("  [CB] CIRCUIT BREAKER TRIPPED - {} | {}",
                     cb.trip_reason.map(|r| r.to_string()).unwrap_or_default(),
                     remaining);
        } else {
            println!("  [CB] Circuit Breaker:  OK (losses={} drawdown={:.2}%)",
                     cb.consecutive_losses, cb.current_drawdown_pct);
        }
        println!("\n[METRICS]");
        self.enhanced_metrics.display();
        self.adaptive_optimizer.display();
        println!("\n[PRICE] Current SOL: ${:.4}", current_price);
        println!("\n{}", border);
        if grid_levels <= 10 {
            self.grid_state.display_all().await;
        }
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
    fn name(&self) -> &str {
        "GridBot"
    }

    fn instance_id(&self) -> &str {
        self.config.bot.instance_name()
    }

    fn set_intent_registry(&mut self, registry: IntentRegistry) {
        info!(
            "[BOT] Intent registry wired for instance '{}' - conflict detection active",
            self.instance_id()
        );
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
            warn!("[BOT::process_tick] Invalid price {:.4} - signalling shutdown", price);
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
        info!("[BOT] Graceful shutdown initiated for instance '{}'", self.instance_id());
        let final_price = self.last_price.unwrap_or(0.0);
        self.display_status(final_price).await;
        self.display_strategy_performance().await;
        info!(
            "[BOT] Shutdown complete | cycles={} fills={} orders={} repos={} \
             conflicts={} filtered={} uptime={}s pnl=${:.2}",
            self.total_cycles,
            self.total_fills_tracked,
            self.total_orders_placed,
            self.grid_repositions,
            self.intent_conflicts,
            self.orders_filtered_session,
            self.session_start.elapsed().as_secs(),
            self.last_known_pnl,
        );
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
    // ── PR #94 Commit 6: Items 1+2 observability ────────────────────────────────────
    pub last_signal_strength:       f64,
    pub orders_filtered_session:    u64,
    pub fee_filter_total_checked:   u64,
    pub fee_filter_total_passed:    u64,
    pub fee_filter_total_blocked:   u64,
}

impl GridBotStats {
    pub fn display_summary(&self) {
        println!("\n[STATS] GRID BOT STATISTICS SUMMARY V6.0");
        println!("   Cycles:            {}", self.total_cycles);
        println!("   Trades:            {}", self.successful_trades);
        println!("   Repositions:       {}", self.grid_repositions);
        println!("   Open Orders:       {}", self.open_orders);
        println!("   Fills Tracked:     {}", self.total_fills_tracked);
        println!("   Intent Conflicts:  {}", self.intent_conflicts);
        println!("   Total Value:       ${:.2}", self.total_value_usdc);
        println!("   P&L:               ${:.2}", self.pnl_usdc);
        println!("   ROI:               {:.2}%", self.roi_percent);
        if self.profitable_trades + self.unprofitable_trades == 0 {
            println!("   Win Rate:          - (no closed trades yet)");
        } else {
            println!("   Win Rate:          {:.2}%", self.win_rate);
        }
        println!("   Fees:              ${:.2}", self.total_fees);
        if self.trading_paused {
            println!("   [CB] Status:       CIRCUIT BREAKER TRIPPED");
        } else {
            println!("   [CB] Status:       OK");
        }
        println!("\n[ANALYTICS]");
        println!("   Profitable Trades: {}", self.profitable_trades);
        println!("   Losing Trades:     {}", self.unprofitable_trades);
        println!("   Max Drawdown:      {:.2}%", self.max_drawdown);
        println!("   Signal Exec Rate:  {:.2}%", self.signal_execution_ratio);
        println!("   Grid Efficiency:   {:.2}%", self.grid_efficiency * 100.0);
        println!("\n[SIGNAL SIZING]");
        println!("   Signal Strength:   {:.3}", self.last_signal_strength);
        println!("\n[FEE FILTER]");
        println!("   Orders Filtered:   {}", self.orders_filtered_session);
        println!("   Total Checked:     {}", self.fee_filter_total_checked);
        println!("   Passed:            {}", self.fee_filter_total_passed);
        println!("   Blocked:           {}", self.fee_filter_total_blocked);
        if self.fee_filter_total_checked > 0 {
            println!("   Pass Rate:         {:.1}%",
                self.fee_filter_total_passed as f64
                    / self.fee_filter_total_checked as f64
                    * 100.0);
        } else {
            println!("   Pass Rate:         - (no orders evaluated yet)");
        }
        println!("\n[OPTIMIZER]");
        println!("   Current Spacing:   {:.3}%", self.current_spacing_percent * 100.0);
        println!("   Current Size:      {:.3} SOL", self.current_position_size);
        println!("   Optimizations:     {}", self.optimization_count);
        if self.trading_paused {
            println!("   Status:            PAUSED - CB TRIPPED");
        } else {
            println!("   Status:            V6.0 ACTIVE");
        }
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
            total_cycles:            0,
            successful_trades:       0,
            grid_repositions:        0,
            open_orders:             0,
            total_value_usdc:        0.0,
            pnl_usdc:                0.0,
            roi_percent:             0.0,
            win_rate:                0.0,
            total_fees:              0.0,
            trading_paused:          false,
            profitable_trades:       0,
            unprofitable_trades:     0,
            max_drawdown:            0.0,
            signal_execution_ratio:  0.0,
            grid_efficiency:         0.0,
            current_spacing_percent: 0.0,
            current_position_size:   0.0,
            optimization_count:      0,
            total_fills_tracked:     0,
            intent_conflicts:        0,
            last_signal_strength:    0.0,
            orders_filtered_session: 0,
            fee_filter_total_checked: 0,
            fee_filter_total_passed:  0,
            fee_filter_total_blocked: 0,
        }
    }

    #[test]
    fn test_gridbotstats_fields() {
        let stats = GridBotStats {
            total_cycles:            100,
            successful_trades:       42,
            grid_repositions:        3,
            open_orders:             6,
            total_value_usdc:        1050.0,
            pnl_usdc:                50.0,
            roi_percent:             5.0,
            win_rate:                0.65,
            total_fees:              1.25,
            trading_paused:          false,
            profitable_trades:       28,
            unprofitable_trades:     14,
            max_drawdown:            2.1,
            signal_execution_ratio:  0.88,
            grid_efficiency:         0.91,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            optimization_count:      2,
            total_fills_tracked:     42,
            intent_conflicts:        0,
            last_signal_strength:    0.0,
            orders_filtered_session: 0,
            fee_filter_total_checked: 0,
            fee_filter_total_passed:  0,
            fee_filter_total_blocked: 0,
        };
        assert_eq!(stats.total_cycles, 100);
        assert_eq!(stats.successful_trades, 42);
        assert!(!stats.trading_paused);
        assert!((stats.pnl_usdc - 50.0).abs() < 1e-9);
        assert_eq!(stats.intent_conflicts, 0);
    }

    #[test]
    fn test_gridbotstats_intent_conflicts_tracked() {
        let stats = GridBotStats {
            intent_conflicts: 3,
            ..zero_stats()
        };
        assert_eq!(stats.intent_conflicts, 3);
    }

    #[test]
    fn test_win_rate_guard_zero_closed_trades() {
        let stats = GridBotStats {
            total_cycles:        100,
            successful_trades:   3,
            open_orders:         8,
            total_value_usdc:    1000.0,
            total_fees:          0.05,
            signal_execution_ratio: 1.0,
            grid_efficiency:     0.5,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            total_fills_tracked: 3,
            ..zero_stats()
        };
        assert_eq!(stats.profitable_trades + stats.unprofitable_trades, 0);
        assert!((stats.win_rate - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_win_rate_guard_with_closed_trades() {
        let stats = GridBotStats {
            total_cycles:        200,
            successful_trades:   10,
            grid_repositions:    1,
            open_orders:         6,
            total_value_usdc:    1020.0,
            pnl_usdc:            20.0,
            roi_percent:         2.0,
            win_rate:            75.0,
            total_fees:          0.25,
            profitable_trades:   6,
            unprofitable_trades: 2,
            max_drawdown:        0.5,
            signal_execution_ratio: 99.8,
            grid_efficiency:     0.7,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            optimization_count:  1,
            total_fills_tracked: 10,
            ..zero_stats()
        };
        assert!(stats.profitable_trades + stats.unprofitable_trades > 0);
        assert!((stats.win_rate - 75.0).abs() < 1e-9);
    }

    #[test]
    fn test_tick_result_paused_reason() {
        let r = TickResult::paused("regime gate / circuit breaker");
        assert!(r.active);
        assert_eq!(r.fills, 0);
        assert!(r.pause_reason.as_deref()
            .unwrap_or("").contains("regime gate"));
    }

    #[test]
    fn test_tick_result_shutdown_on_bad_price() {
        let r = TickResult::shutdown();
        assert!(!r.active);
        assert_eq!(r.fills, 0);
        assert_eq!(r.orders_placed, 0);
    }

    #[test]
    fn test_bot_stats_default_zero() {
        let s = BotStats::default();
        assert_eq!(s.total_cycles, 0);
        assert_eq!(s.total_fills, 0);
        assert_eq!(s.total_orders, 0);
        assert_eq!(s.uptime_secs, 0);
        assert!(!s.is_paused);
        assert_eq!(s.current_pnl, 0.0);
        assert_eq!(s.intent_conflicts, 0);
    }

    #[test]
    fn test_tick_result_orders_placed_field() {
        let r = TickResult::active(2, 6);
        assert_eq!(r.fills, 2);
        assert_eq!(r.orders_placed, 6);
        assert!(r.active);
        assert!(r.pause_reason.is_none());
    }

    #[test]
    fn test_registry_cleanup_on_reposition() {
        use crate::bots::bot_trait::new_intent_registry;
        let registry = new_intent_registry();
        let pair = "SOL/USDC".to_string();
        registry.insert((pair.clone(), 1u64), "sol-usdc-grid-01".into());
        registry.insert((pair.clone(), 2u64), "sol-usdc-grid-01".into());
        registry.insert((pair.clone(), 3u64), "sol-usdc-grid-01".into());
        assert_eq!(registry.len(), 3);
        let cancelled_levels = vec![1u64, 2u64];
        for level_id in &cancelled_levels {
            registry.remove(&(pair.clone(), *level_id));
        }
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains_key(&(pair.clone(), 1u64)));
        assert!(!registry.contains_key(&(pair.clone(), 2u64)));
        assert!(registry.contains_key(&(pair.clone(), 3u64)));
    }

    #[test]
    fn test_circuit_breaker_field_initialized() {
        use crate::config::*;
        let config = Config {
            bot: BotConfig {
                name: "test".to_string(),
                version: "1.0".to_string(),
                environment: "test".to_string(),
                execution_mode: "paper".to_string(),
                instance_id: None,
            },
            network: NetworkConfig {
                cluster: "devnet".to_string(),
                rpc_url: "http://localhost".to_string(),
                commitment: "confirmed".to_string(),
                ws_url: None,
            },
            security: SecurityConfig::default(),
            trading: TradingConfig::default(),
            strategies: StrategiesConfig::default(),
            execution: ExecutionConfig::default(),
            risk: RiskConfig {
                max_position_size_pct: 80.0,
                max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0,
                take_profit_pct: 10.0,
                enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0,
                circuit_breaker_cooldown_secs: 60,
                max_consecutive_losses: 5,
                enable_trailing_stop: false,
            },
            fees: FeesConfig::default(),
            priority_fees: PriorityFeeConfig::default(),
            pyth: PythConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(),
            alerts: AlertsConfig::default(),
        };
        let cb = CircuitBreaker::new(&config);
        let status = cb.status();
        assert!(!status.is_tripped);
        assert_eq!(status.consecutive_losses, 0);
        assert_eq!(status.daily_pnl, 0.0);
    }

    #[test]
    fn test_circuit_breaker_trading_paused_in_stats() {
        let stats_paused = GridBotStats {
            total_cycles:        10,
            open_orders:         0,
            total_value_usdc:    800.0,
            pnl_usdc:            -200.0,
            roi_percent:         -20.0,
            total_fees:          0.5,
            trading_paused:      true,
            unprofitable_trades: 5,
            max_drawdown:        20.0,
            signal_execution_ratio: 1.0,
            grid_efficiency:     0.3,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            total_fills_tracked: 5,
            ..zero_stats()
        };
        assert!(stats_paused.trading_paused);
        let stats_ok = GridBotStats { trading_paused: false, ..stats_paused.clone() };
        assert!(!stats_ok.trading_paused);
    }

    #[test]
    fn test_circuit_breaker_record_trade_real_pnl() {
        use crate::config::*;
        let config = Config {
            bot: BotConfig {
                name: "test".to_string(),
                version: "1.0".to_string(),
                environment: "test".to_string(),
                execution_mode: "paper".to_string(),
                instance_id: None,
            },
            network: NetworkConfig {
                cluster: "devnet".to_string(),
                rpc_url: "http://localhost".to_string(),
                commitment: "confirmed".to_string(),
                ws_url: None,
            },
            security: SecurityConfig::default(),
            trading: TradingConfig::default(),
            strategies: StrategiesConfig::default(),
            execution: ExecutionConfig::default(),
            risk: RiskConfig {
                max_position_size_pct: 80.0,
                max_drawdown_pct: 10.0,
                stop_loss_pct: 5.0,
                take_profit_pct: 10.0,
                enable_circuit_breaker: true,
                circuit_breaker_threshold_pct: 15.0,
                circuit_breaker_cooldown_secs: 60,
                max_consecutive_losses: 3,
                enable_trailing_stop: false,
            },
            fees: FeesConfig::default(),
            priority_fees: PriorityFeeConfig::default(),
            pyth: PythConfig::default(),
            performance: PerformanceConfig::default(),
            logging: LoggingConfig::default(),
            metrics: MetricsConfig::default(),
            paper_trading: PaperTradingConfig::default(),
            database: DatabaseConfig::default(),
            alerts: AlertsConfig::default(),
        };
        let mut cb = CircuitBreaker::new(&config);
        cb.record_trade(-10.0, 990.0);
        assert!(!cb.status().is_tripped);
        cb.record_trade(-10.0, 980.0);
        assert!(!cb.status().is_tripped);
        cb.record_trade(-10.0, 970.0);
        assert!(cb.status().is_tripped);
        assert!(matches!(cb.status().trip_reason, Some(TripReason::ConsecutiveLosses)));
    }

    fn compute_effective_size(
        enable_smart: bool,
        order_size: f64,
        signal_strength: f64,
        signal_size_multiplier: f64,
        min_order_sol: f64,
        max_position_size: f64,
    ) -> f64 {
        if enable_smart {
            let multiplier = 1.0 + signal_strength * (signal_size_multiplier - 1.0);
            (order_size * multiplier).clamp(min_order_sol, max_position_size)
        } else {
            order_size
        }
    }

    #[test]
    fn test_smart_sizing_disabled_uses_base_size() {
        let base = 0.1_f64;
        let effective = compute_effective_size(false, base, 1.0, 2.0, 0.05, 10.0);
        assert!((effective - base).abs() < 1e-12);
    }

    #[test]
    fn test_smart_sizing_hold_signal_no_size_change() {
        let base = 0.1_f64;
        for multiplier_cfg in [1.0_f64, 1.5, 2.0, 3.0] {
            let effective = compute_effective_size(true, base, 0.0, multiplier_cfg, 0.05, 10.0);
            assert!((effective - base).abs() < 1e-12,
                "hold signal: expected {base} at mult_cfg={multiplier_cfg}, got {effective}");
        }
    }

    #[test]
    fn test_smart_sizing_strong_signal_scales_up() {
        let base = 0.1_f64;
        let effective = compute_effective_size(true, base, 1.0, 2.0, 0.05, 10.0);
        assert!((effective - 0.2_f64).abs() < 1e-12);
    }

    #[test]
    fn test_smart_sizing_clamp_respects_max_position() {
        let base = 5.0_f64;
        let max  = 8.0_f64;
        let effective = compute_effective_size(true, base, 1.0, 3.0, 0.05, max);
        assert!((effective - max).abs() < 1e-12);
    }

    #[test]
    fn test_smart_sizing_clamp_respects_min_order() {
        let base    = 0.1_f64;
        let min_sol = 0.08_f64;
        let effective = compute_effective_size(true, base, 1.0, 0.5, min_sol, 10.0);
        assert!((effective - min_sol).abs() < 1e-12);
    }

    #[test]
    fn test_gridbotstats_fee_filter_fields_zero_default() {
        let stats = zero_stats();
        assert_eq!(stats.orders_filtered_session,  0);
        assert_eq!(stats.fee_filter_total_checked, 0);
        assert_eq!(stats.fee_filter_total_passed,  0);
        assert_eq!(stats.fee_filter_total_blocked, 0);
        assert!((stats.last_signal_strength - 0.0).abs() < 1e-12);
    }

    #[test]
    fn test_gridbotstats_fee_filter_fields_populated() {
        let stats = GridBotStats {
            fee_filter_total_checked: 120,
            fee_filter_total_passed:   95,
            fee_filter_total_blocked:  25,
            orders_filtered_session:   25,
            ..zero_stats()
        };
        assert_eq!(stats.fee_filter_total_checked, 120);
        assert_eq!(stats.fee_filter_total_passed,   95);
        assert_eq!(stats.fee_filter_total_blocked,  25);
        assert_eq!(stats.orders_filtered_session,   25);
        assert_eq!(
            stats.fee_filter_total_passed + stats.fee_filter_total_blocked,
            stats.fee_filter_total_checked
        );
    }

    #[test]
    fn test_gridbotstats_signal_strength_field() {
        let stats = GridBotStats {
            last_signal_strength: 0.75,
            ..zero_stats()
        };
        assert!((stats.last_signal_strength - 0.75).abs() < 1e-12);
    }
}
