//! =============================================================================
//! GRID BOT V4.5 - STRATEGY-AS-SOURCE-OF-TRUTH
//!
//! V4.3 ENHANCEMENTS - Fill Tracking & Learning:
//! - GridLevel pairing (buy/sell orders linked)
//! - Safe reposition (preserves filled buys)
//! - Order lifecycle tracking per level
//! - No orphaned positions
//! - Production-ready state management
//! - ENHANCED METRICS - Trade-level analytics
//! - ADAPTIVE OPTIMIZER - Self-learning grid
//! - Smart spacing based on drawdown
//! - Dynamic position sizing based on efficiency
//! - Win/Loss streak detection
//! - FILL TRACKING - ML training dataset
//!
//! Stage 3 / Step 1 (Feb 2026):
//! - engine is now Box<dyn TradingEngine> -- paper or live, decided by caller
//! - Engine injected via constructor -- GridBot has zero knowledge of engine type
//! - place_limit_order_with_level() used for all grid orders (level ID tagged)
//! - get_engine_stats() replaces get_wallet() / get_performance_stats()
//!
//! Stage 3 / Step 2 (Feb 2026):
//! - process_price_update returns Vec<FillEvent> -- no string-sniffing
//! - fill.side used directly; to_lowercase().contains("buy") hack removed
//! - manager.notify_fill(&fill) wired for every fill
//!
//! Stage 3 / Step 5B (Feb 2026):
//! - last_reposition_price: dedicated anchor for crossing gate
//! - signal_active: record_signal() reflects real consensus (not hardcoded true)
//! - is_trading_allowed() guards every reposition attempt
//! - should_reposition() + reposition_grid() wired into price tick loop
//! - Signal::Hold respected -- grid held, debug log emitted
//!
//! Stage 3 / Step 5D (Feb 2026):
//! - update_grid_stats() now called after every fill batch, not just on reposition
//!   -> AdaptiveOptimizer always receives live level utilisation data
//!
//! Stage 3 / Step 6 (Feb 2026) - STRATEGY AS SOURCE OF TRUTH:
//! - process_price_update Step 3 now matches on Signal variant:
//!     Buy { level_id: None }  -> reposition_grid() (init or anchor drift)
//!     Buy { level_id: Some }  -> place single order for that level
//!     Sell { level_id: Some } -> place single order for that level
//!     Hold                    -> nothing
//! - Old blunt signal_active gate removed; GridRebalancer owns crossing logic.
//! - sync_levels_to_strategy(): pushes level snapshots + anchor into strategy
//!   after every grid placement so GridRebalancer is always up to date.
//! =============================================================================

use crate::strategies::{StrategyManager, GridRebalancer, GridRebalancerConfig, Signal};
use crate::strategies::grid_rebalancer::LevelSnapshot;
use crate::strategies::shared::analytics::AnalyticsContext;
use crate::trading::{
    TradingEngine,
    OrderSide,
    FillEvent,
    GridStateTracker,
    EnhancedMetrics,
    AdaptiveOptimizer,
};
use crate::config::Config;
use anyhow::{Result, Context};
use log::{info, warn, debug, trace};

// Optimization frequency: run optimizer every N cycles
const OPTIMIZATION_INTERVAL_CYCLES: u64 = 50;  // Every 50 cycles (~5 mins at 100ms)

// =============================================================================
// GRID BOT - Strategy-as-Source-of-Truth Orchestrator
// =============================================================================

pub struct GridBot {
    pub manager:              StrategyManager,
    engine:                   Box<dyn TradingEngine>,
    pub config:               Config,
    pub grid_state:           GridStateTracker,
    pub enhanced_metrics:     EnhancedMetrics,
    pub adaptive_optimizer:   AdaptiveOptimizer,
    last_price:               Option<f64>,
    /// Kept for potential future use (e.g. emergency reposition override).
    /// Primary reposition logic now lives in GridRebalancer.analyze().
    #[allow(dead_code)]
    last_reposition_price:    Option<f64>,
    total_cycles:             u64,
    successful_trades:        u64,
    grid_repositions:         u64,
    last_reposition_time:     Option<std::time::Instant>,
    last_optimization_cycle:  u64,
    grid_initialized:         bool,
    total_fills_tracked:      u64,
}

impl GridBot {
    /// Create a new GridBot with an injected engine.
    /// The caller decides whether to pass a PaperTradingEngine or RealTradingEngine.
    pub fn new(config: Config, engine: Box<dyn TradingEngine>) -> Result<Self> {
        info!("[GridBot] Initializing V4.5 STRATEGY-AS-SOURCE-OF-TRUTH");
        info!("[GridBot] Adaptive Intelligence: ENABLED");
        info!("[GridBot] Fill Tracking:         ENABLED");
        info!("[GridBot] Level Crossing Gate:   ENABLED (V5.0)");
        info!("[GridBot] Circuit Breaker Gate:  ENABLED");

        let analytics_ctx = AnalyticsContext::default();
        let mut manager   = StrategyManager::new(analytics_ctx);

        info!("[GridBot] Creating grid rebalancer from config...");

        // V5.0: wire reposition_threshold_pct from config.trading
        // Falls back to 0.5% if not set (safe for paper runs)
        let reposition_threshold_pct = config.trading.reposition_threshold;

        let grid_config = GridRebalancerConfig {
            grid_spacing:                    config.trading.grid_spacing_percent / 100.0,
            order_size:                      config.trading.min_order_size,
            min_usdc_balance:                config.trading.min_usdc_reserve,
            min_sol_balance:                 config.trading.min_sol_reserve,
            enabled:                         config.strategies.grid.enabled,
            enable_dynamic_spacing:          config.trading.enable_dynamic_grid,
            enable_fee_filtering:            config.trading.enable_fee_optimization,
            volatility_window_seconds:       config.trading.volatility_window as u64,
            max_spacing:                     0.0075,
            min_spacing:                     0.001,
            enable_regime_gate:              config.trading.enable_regime_gate,
            min_volatility_to_trade:         config.trading.min_volatility_to_trade,
            pause_in_very_low_vol:           config.trading.pause_in_very_low_vol,
            enable_order_lifecycle:          config.trading.enable_order_lifecycle,
            order_max_age_minutes:           config.trading.order_max_age_minutes,
            order_refresh_interval_minutes:  config.trading.order_refresh_interval_minutes,
            min_orders_to_maintain:          config.trading.min_orders_to_maintain,
            reposition_threshold_pct,        // V5.0
        };

        let grid_rebalancer = GridRebalancer::new(grid_config)
            .context("Failed to create GridRebalancer")?;

        manager.add_strategy(grid_rebalancer);
        info!("[GridBot] Grid rebalancer V5.0 strategy loaded");

        if config.strategies.momentum.enabled {
            info!("[GridBot] Momentum strategy enabled (not yet implemented)");
        }
        if config.strategies.mean_reversion.enabled {
            info!("[GridBot] Mean reversion strategy enabled (not yet implemented)");
        }
        if config.strategies.rsi.enabled {
            info!("[GridBot] RSI strategy enabled (not yet implemented)");
        }

        info!("[GridBot] Trading engine injected (type decided by caller)");

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);

        info!("[GridBot] Optimization interval: every {} cycles", OPTIMIZATION_INTERVAL_CYCLES);
        info!("[GridBot] Reposition threshold:  {:.2}%", reposition_threshold_pct);
        info!("[GridBot] V4.5 initialization complete");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,
            adaptive_optimizer,
            last_price:              None,
            last_reposition_price:   None,
            total_cycles:            0,
            successful_trades:       0,
            grid_repositions:        0,
            last_reposition_time:    None,
            last_optimization_cycle: 0,
            grid_initialized:        false,
            total_fills_tracked:     0,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("[GridBot] Async initialization...");
        info!("[GridBot] Initialization complete");
        Ok(())
    }

    // =========================================================================
    // GRID PLACEMENT
    // =========================================================================

    pub async fn reposition_grid(&mut self, current_price: f64, last_price: f64) -> Result<()> {
        if !self.grid_initialized {
            info!("[GridBot] Placing initial grid at ${:.4}", current_price);
            self.place_grid_orders(current_price).await?;
            self.grid_initialized = true;
            info!("[GridBot] Initial grid placed");

            let total_levels = self.config.trading.grid_levels as usize;
            let used_levels  = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

            // V5.0: sync level snapshot into GridRebalancer after first placement
            self.sync_levels_to_strategy(current_price).await;
            return Ok(());
        }

        info!("[GridBot] Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);

        let reposition_start = std::time::Instant::now();

        let filled_buys = self.grid_state.get_levels_with_filled_buys().await;
        if !filled_buys.is_empty() {
            warn!("[GridBot] {} levels have filled buys - preserving sell orders", filled_buys.len());
            for level in &filled_buys {
                info!("  -> Level {} buy filled @ ${:.4} - keeping sell @ ${:.4}",
                      level.id, level.buy_price, level.sell_price);
            }
        }

        let cancellable = self.grid_state.get_cancellable_levels().await;
        info!("[GridBot] {} cancellable levels (of {} total)",
              cancellable.len(), self.grid_state.count().await);

        let mut cancelled_count = 0;

        for level_id in cancellable {
            if let Some(level) = self.grid_state.get_level(level_id).await {
                if let Some(buy_id) = &level.buy_order_id {
                    match self.engine.cancel_order(buy_id).await {
                        Ok(_)  => { cancelled_count += 1; }
                        Err(e) => warn!("[GridBot] Failed to cancel buy {}: {}", buy_id, e),
                    }
                }
                if let Some(sell_id) = &level.sell_order_id {
                    match self.engine.cancel_order(sell_id).await {
                        Ok(_)  => { cancelled_count += 1; }
                        Err(e) => warn!("[GridBot] Failed to cancel sell {}: {}", sell_id, e),
                    }
                }
                self.grid_state.mark_cancelled(level_id).await;
            }
        }

        if cancelled_count > 0 {
            info!("[GridBot] Cancelled {} orders", cancelled_count);
        }

        self.place_grid_orders(current_price).await?;

        self.grid_repositions    += 1;
        self.last_reposition_time = Some(std::time::Instant::now());

        let total_levels = self.config.trading.grid_levels as usize;
        let used_levels  = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

        // V5.0: sync updated level snapshot + new anchor into GridRebalancer
        self.sync_levels_to_strategy(current_price).await;

        let elapsed = reposition_start.elapsed().as_millis();
        info!("[GridBot] Grid repositioned in {}ms", elapsed);

        Ok(())
    }

    async fn place_grid_orders(&mut self, current_price: f64) -> Result<()> {
        let grid_spacing = self.adaptive_optimizer.current_spacing_percent;
        let order_size   = self.adaptive_optimizer.current_position_size;
        let num_levels   = self.config.trading.grid_levels;

        debug!("[GridBot] Placing {} levels @ {:.3}% spacing, {:.3} SOL/order",
               num_levels, grid_spacing * 100.0, order_size);

        let mut orders_placed = 0;
        let mut orders_failed = 0;
        let buy_levels  = num_levels / 2;
        let sell_levels = num_levels - buy_levels;

        for i in 1..=buy_levels.min(sell_levels) {
            let buy_price  = current_price * (1.0 - grid_spacing * i as f64);
            let sell_price = current_price * (1.0 + grid_spacing * i as f64);

            let mut level = self.grid_state.create_level(buy_price, sell_price, order_size).await;

            match self.engine.place_limit_order_with_level(
                OrderSide::Buy, buy_price, order_size, Some(level.id)
            ).await {
                Ok(buy_order_id) => {
                    level.set_buy_order(buy_order_id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("[GridBot] Failed buy @ ${:.4}: {}", buy_price, e);
                    orders_failed += 1;
                    continue;
                }
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Sell, sell_price, order_size, Some(level.id)
            ).await {
                Ok(sell_order_id) => {
                    level.set_sell_order(sell_order_id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("[GridBot] Failed sell @ ${:.4}: {}", sell_price, e);
                    orders_failed += 1;
                }
            }

            self.grid_state.update_level(level).await;
        }

        info!("[GridBot] Placed {} orders ({} pairs), {} failed",
              orders_placed, buy_levels.min(sell_levels), orders_failed);

        Ok(())
    }

    // =========================================================================
    // V5.0 - SYNC LEVEL SNAPSHOTS INTO STRATEGY
    //
    // Called after every grid placement / reposition.
    // Reads price boundaries from GridStateTracker and pushes them into
    // GridRebalancer via set_grid_levels() + set_anchor().
    //
    // This is the wire that makes the strategy the source of truth:
    // the strategy knows exactly where the current grid lines are, so
    // crossing detection in analyze() is always accurate.
    // =========================================================================

    async fn sync_levels_to_strategy(&mut self, anchor_price: f64) {
        // Collect price-only snapshots from all active levels
        let snapshots: Vec<LevelSnapshot> = self.grid_state
            .get_all_levels().await
            .into_iter()
            .map(|l| LevelSnapshot {
                id:         l.id,
                buy_price:  l.buy_price,
                sell_price: l.sell_price,
            })
            .collect();

        let count = snapshots.len();

        // Push into the GridRebalancer strategy
        // We use get_grid_rebalancer_mut() from StrategyManager to access it.
        if let Some(rebalancer) = self.manager.get_grid_rebalancer_mut() {
            rebalancer.set_grid_levels(snapshots).await;
            rebalancer.set_anchor(anchor_price).await;
            debug!("[GridBot] Synced {} level snapshots to strategy (anchor=${:.4})",
                   count, anchor_price);
        } else {
            warn!("[GridBot] Could not sync levels: GridRebalancer not found in manager");
        }
    }

    // =========================================================================
    // MAIN PRICE TICK HANDLER
    //
    // Execution order:
    //   1. Circuit breaker     - is the engine healthy?
    //   2. Strategy signal     - what does GridRebalancer say this tick?
    //   3. Act on signal       - dispatch based on signal variant:
    //        Buy(None)          -> reposition_grid() + sync levels
    //        Buy(Some(id))      -> place one limit buy for that level
    //        Sell(Some(id))     -> place one limit sell for that level
    //        Hold               -> nothing
    //   4. Fill collection     - engine.process_price_update()
    //   5. Grid stats refresh  - update_grid_stats()
    //   6. Portfolio snapshot  - engine.get_engine_stats()
    //   7. Adaptive optimizer  - runs every OPTIMIZATION_INTERVAL_CYCLES ticks
    // =========================================================================

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        self.total_cycles += 1;
        self.last_price    = Some(price);
        self.enhanced_metrics.update_price_range(price);

        trace!("[GridBot] Price update: ${:.4} (cycle {})", price, self.total_cycles);

        // ── 1. Circuit breaker / engine health gate ───────────────────────────
        let trading_ok = self.engine.is_trading_allowed().await;
        if !trading_ok {
            warn!("[GridBot] Engine halted — circuit breaker or emergency shutdown active");
            return Ok(());
        }

        // ── 2. Strategy signal ────────────────────────────────────────────────
        // GridRebalancer.analyze() now owns all the crossing/reposition logic.
        // It returns exactly what action to take this tick.
        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Failed to get strategy signal")?;

        // Record signal activity for metrics
        let signal_active = !matches!(signal, Signal::Hold { .. });
        self.enhanced_metrics.record_signal(signal_active);

        trace!("[GridBot] Signal: {}", signal.display());

        // ── 3. Act on signal ──────────────────────────────────────────────────
        match &signal {
            // Grid-wide action: bootstrap or anchor drift → reposition
            Signal::Buy { level_id: None, reason, price: sig_price, .. } => {
                info!("[GridBot] Grid action: '{}' @ ${:.4}", reason, sig_price);
                let anchor = self.last_reposition_price.unwrap_or(*sig_price);
                self.reposition_grid(*sig_price, anchor).await
                    .context("Grid action (reposition) failed")?;
                self.last_reposition_price = Some(*sig_price);
                // Note: sync_levels_to_strategy() is called inside reposition_grid()
            }

            // Level crossing → place a single limit buy for this level
            Signal::Buy { level_id: Some(id), price: level_price, size, reason, .. } => {
                debug!("[GridBot] BUY crossing L{} @ ${:.4} — {}", id, level_price, reason);
                if let Err(e) = self.engine.place_limit_order_with_level(
                    OrderSide::Buy, *level_price, *size, Some(*id)
                ).await {
                    warn!("[GridBot] BUY L{} order failed: {}", id, e);
                    // Non-fatal: level may already have an open order
                }
            }

            // Level crossing → place a single limit sell for this level
            Signal::Sell { level_id: Some(id), price: level_price, size, reason, .. } => {
                debug!("[GridBot] SELL crossing L{} @ ${:.4} — {}", id, level_price, reason);
                if let Err(e) = self.engine.place_limit_order_with_level(
                    OrderSide::Sell, *level_price, *size, Some(*id)
                ).await {
                    warn!("[GridBot] SELL L{} order failed: {}", id, e);
                }
            }

            // Sell { level_id: None } shouldn't happen in this bot but handle safely
            Signal::Sell { level_id: None, .. } => {
                debug!("[GridBot] Sell(level_id=None) signal ignored");
            }

            // No action this tick
            Signal::Hold { .. } => {
                trace!("[GridBot] Hold — no grid action this tick");
            }

            // StrongBuy / StrongSell: not emitted by GridRebalancer, but handle gracefully
            Signal::StrongBuy { .. } | Signal::StrongSell { .. } => {
                debug!("[GridBot] StrongBuy/StrongSell signal — no handler yet");
            }
        }

        // ── 4. Collect fills emitted by engine ────────────────────────────────
        let filled_orders: Vec<FillEvent> = self.engine.process_price_update(price).await
            .context("Failed to process price update in trading engine")?;

        if !filled_orders.is_empty() {
            info!("[GridBot] {} fill(s) at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            for fill in &filled_orders {
                let is_buy = matches!(fill.side, OrderSide::Buy);
                let pnl    = self.grid_state.total_realized_pnl().await;

                self.total_fills_tracked += 1;

                info!("[Fill] #{}: {:?} {} @ ${:.4} | size={:.4} SOL | fee=${:.4} | level={:?} | P&L=${:.2}",
                      self.total_fills_tracked,
                      fill.side,
                      fill.order_id,
                      fill.price,
                      fill.size,
                      fill.fee,
                      fill.grid_level_id,
                      pnl);

                debug!("[Fill] spacing={:.3}% total_fills={} cycles={}",
                       self.adaptive_optimizer.current_spacing_percent * 100.0,
                       self.total_fills_tracked,
                       self.total_cycles);

                self.enhanced_metrics.record_trade(is_buy, pnl, timestamp);
                self.manager.notify_fill(fill).await;
            }
        }

        // ── 5. Grid stats refresh ─────────────────────────────────────────────
        if self.grid_initialized {
            let total_levels = self.config.trading.grid_levels as usize;
            let used_levels  = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);
        }

        // ── 6. Portfolio snapshot ─────────────────────────────────────────────
        let engine_stats = self.engine.get_engine_stats(price).await;
        self.enhanced_metrics.update_portfolio_value(engine_stats.total_value_usdc);

        // ── 7. Adaptive optimization every N cycles ───────────────────────────
        if self.total_cycles - self.last_optimization_cycle >= OPTIMIZATION_INTERVAL_CYCLES {
            debug!("[GridBot] Running adaptive optimization cycle...");
            let result = self.adaptive_optimizer.optimize(&self.enhanced_metrics);

            if result.any_changes() {
                info!("[GridBot] Optimizer: {} | spacing={:.3}% size={:.3} SOL",
                      result.reason,
                      result.new_spacing * 100.0,
                      result.new_position_size);
            }

            self.last_optimization_cycle = self.total_cycles;
        }

        Ok(())
    }

    // =========================================================================
    // STATS & DISPLAY
    // =========================================================================

    pub async fn get_stats(&self) -> BotStats {
        let current_price = self.last_price.unwrap_or(0.0);
        let engine_stats  = self.engine.get_engine_stats(current_price).await;
        let open_orders   = self.engine.open_order_count().await;

        BotStats {
            total_cycles:            self.total_cycles,
            successful_trades:       self.successful_trades,
            grid_repositions:        self.grid_repositions,
            open_orders,
            total_value_usdc:        engine_stats.total_value_usdc,
            pnl_usdc:                engine_stats.pnl_usdc,
            roi_percent:             engine_stats.roi_percent,
            win_rate:                engine_stats.win_rate,
            total_fees:              engine_stats.total_fees,
            trading_paused:          !self.engine.is_trading_allowed().await,
            profitable_trades:       self.enhanced_metrics.profitable_trades,
            unprofitable_trades:     self.enhanced_metrics.unprofitable_trades,
            max_drawdown:            self.enhanced_metrics.max_drawdown,
            signal_execution_ratio:  self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency:         self.enhanced_metrics.grid_efficiency,
            current_spacing_percent: self.adaptive_optimizer.current_spacing_percent,
            current_position_size:   self.adaptive_optimizer.current_position_size,
            optimization_count:      self.adaptive_optimizer.adjustment_count,
            total_fills_tracked:     self.total_fills_tracked,
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats = self.get_stats().await;
        let sep   = "=".repeat(60);

        println!("\n{}", sep);
        println!("  GRID BOT V4.5 STRATEGY-AS-SOURCE-OF-TRUTH - STATUS");
        println!("{}", sep);

        println!("\nBot Performance:");
        println!("  Total Cycles     : {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions : {}", stats.grid_repositions);
        println!("  Open Orders      : {}", stats.open_orders);
        println!("  Fills Tracked    : {}", stats.total_fills_tracked);
        println!("  Trading Paused   : {}", stats.trading_paused);

        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl   = self.grid_state.total_realized_pnl().await;

        println!("\nGrid State:");
        println!("  Active Levels    : {}", grid_levels);
        println!("  Filled Buys      : {}", filled_buys);
        println!("  Realized P&L     : ${:.2}", total_pnl);

        println!("\nPortfolio:");
        println!("  Total Value      : ${:.2}", stats.total_value_usdc);
        println!("  P&L              : ${:.2}", stats.pnl_usdc);
        println!("  ROI              : {:.2}%", stats.roi_percent);

        println!("\nTrading Stats:");
        println!("  Win Rate         : {:.2}%", stats.win_rate);
        println!("  Total Fees       : ${:.2}", stats.total_fees);

        println!("\nEnhanced Metrics:");
        self.enhanced_metrics.display();
        self.adaptive_optimizer.display();

        println!("\nSOL Price: ${:.4}", current_price);
        println!("{}", sep);

        if grid_levels <= 10 {
            self.grid_state.display_all().await;
        }
    }

    pub async fn display_strategy_performance(&self) {
        self.manager.display_stats();
    }
}

#[derive(Debug, Clone)]
pub struct BotStats {
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
}

impl BotStats {
    pub fn display_summary(&self) {
        println!("\nBOT STATISTICS SUMMARY V4.5");
        println!("  Cycles           : {}", self.total_cycles);
        println!("  Trades           : {}", self.successful_trades);
        println!("  Repositions      : {}", self.grid_repositions);
        println!("  Open Orders      : {}", self.open_orders);
        println!("  Fills Tracked    : {}", self.total_fills_tracked);
        println!("  Total Value      : ${:.2}", self.total_value_usdc);
        println!("  P&L              : ${:.2}", self.pnl_usdc);
        println!("  ROI              : {:.2}%", self.roi_percent);
        println!("  Win Rate         : {:.2}%", self.win_rate);
        println!("  Fees             : ${:.2}", self.total_fees);
        println!("  Trading Paused   : {}", self.trading_paused);

        println!("\nEnhanced Analytics:");
        println!("  Profitable Trades: {}", self.profitable_trades);
        println!("  Losing Trades    : {}", self.unprofitable_trades);
        println!("  Max Drawdown     : {:.2}%", self.max_drawdown);
        println!("  Signal Exec Rate : {:.2}%", self.signal_execution_ratio * 100.0);
        println!("  Grid Efficiency  : {:.2}%", self.grid_efficiency * 100.0);

        println!("\nAdaptive Optimizer:");
        println!("  Current Spacing  : {:.3}%", self.current_spacing_percent * 100.0);
        println!("  Current Size     : {:.3} SOL", self.current_position_size);
        println!("  Adjustments      : {}", self.optimization_count);
    }
}
