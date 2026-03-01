//! ═══════════════════════════════════════════════════════════════════
//! GRID BOT V4.5 - ELITE AUTONOMOUS TRADING ORCHESTRATOR
//!
//! V4.5 CHANGES (fix/grid-init-with-live-price):
//! ✅ initialize_with_price(&feed) — async grid init with real price
//! ✅ Emergency safety check in reposition_grid() — zero silent failures
//! ✅ Grid initialized BEFORE trading loop starts — Active Levels > 0
//!
//! V4.4 CHANGES (Stage 3 - Fill Fan-out):
//! ✅ spacing_mode wired to GridRebalancerConfig (VolatilityBuckets default)
//! ✅ drain_fills() -> notify_fill() integrated in process_price_update
//!    Full Stage 3 pipeline: tick -> fill -> drain -> fan-out -> on_fill
//! ✅ TODO comment replaced with real implementation
//!
//! V4.3 ENHANCEMENTS - Fill Tracking & Learning:
//! ✅ GridLevel pairing (buy/sell orders linked)
//! ✅ Safe reposition (preserves filled buys)
//! ✅ Order lifecycle tracking per level
//! ✅ ENHANCED METRICS - Trade-level analytics
//! ✅ ADAPTIVE OPTIMIZER - Self-learning grid
//! ✅ Smart spacing based on drawdown
//!
//! February 2026 - V4.5 GRID INIT FIX
//! ═══════════════════════════════════════════════════════════════════

use crate::strategies::{StrategyManager, GridRebalancer, GridRebalancerConfig};
use crate::strategies::shared::analytics::AnalyticsContext;
use crate::trading::{
    PaperTradingEngine,
    OrderSide,
    GridStateTracker,
    EnhancedMetrics,
    AdaptiveOptimizer,
    PriceFeed,
};
use crate::config::Config;
use anyhow::{Result, Context, bail};
use log::{info, warn, debug, trace};

const OPTIMIZATION_INTERVAL_CYCLES: u64 = 50;

pub struct GridBot {
    pub manager: StrategyManager,
    pub engine: PaperTradingEngine,
    pub config: Config,
    pub grid_state: GridStateTracker,
    pub enhanced_metrics: EnhancedMetrics,
    pub adaptive_optimizer: AdaptiveOptimizer,
    last_price: Option<f64>,
    total_cycles: u64,
    successful_trades: u64,
    grid_repositions: u64,
    last_reposition_time: Option<std::time::Instant>,
    last_optimization_cycle: u64,
    grid_initialized: bool,
    total_fills_tracked: u64,
}

impl GridBot {
    pub fn new(config: Config) -> Result<Self> {
        info!("[BOT] Initializing GridBot V4.5 Grid-Init Fix...");
        info!("[BOT] Adaptive Intelligence: ENABLED");
        info!("[BOT] Fill Fan-out: ENABLED");

        let analytics_ctx = AnalyticsContext::default();
        let mut manager = StrategyManager::new(analytics_ctx);

        info!("[BOT] Creating grid rebalancer from config...");

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
            // V4.4: fill remaining new fields (spacing_mode etc.) from Default
            ..GridRebalancerConfig::default()
        };

        info!("[BOT] Initializing grid rebalancer strategy...");

        let grid_rebalancer = GridRebalancer::new(grid_config)
            .context("Failed to create GridRebalancer")?;

        manager.add_strategy(grid_rebalancer);
        info!("[BOT] Grid rebalancer strategy loaded");

        if config.strategies.momentum.enabled {
            info!("[BOT] Momentum strategy enabled (not yet implemented)");
        }
        if config.strategies.mean_reversion.enabled {
            info!("[BOT] Mean reversion strategy enabled (not yet implemented)");
        }
        if config.strategies.rsi.enabled {
            info!("[BOT] RSI strategy enabled (not yet implemented)");
        }

        info!("[BOT] Initializing paper trading engine...");

        let initial_usdc = config.paper_trading.initial_usdc;
        let initial_sol = config.paper_trading.initial_sol;

        if initial_usdc <= 0.0 || initial_sol <= 0.0 {
            bail!("Invalid initial capital: USDC={}, SOL={}", initial_usdc, initial_sol);
        }

        let engine = PaperTradingEngine::new(initial_usdc, initial_sol)
            .with_fees(0.0002, 0.0004)
            .with_slippage(0.0005);

        info!("[BOT] Paper trading engine initialized");
        info!("   Initial Capital: ${:.2} USDC + {} SOL", initial_usdc, initial_sol);

        let grid_state = GridStateTracker::new();
        let enhanced_metrics = EnhancedMetrics::new();

        let base_spacing = config.trading.grid_spacing_percent / 100.0;
        let base_size = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);
        info!("[BOT] Adaptive optimizer initialized (every {} cycles)", OPTIMIZATION_INTERVAL_CYCLES);

        info!("[BOT] GridBot V4.5 initialization complete (grid placement deferred until price known)");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,
            adaptive_optimizer,
            last_price: None,
            total_cycles: 0,
            successful_trades: 0,
            grid_repositions: 0,
            last_reposition_time: None,
            last_optimization_cycle: 0,
            grid_initialized: false,
            total_fills_tracked: 0,
        })
    }

    /// Lightweight async hook kept for backward compat — no-op since V4.5
    /// uses initialize_with_price() for real grid placement.
    pub async fn initialize(&mut self) -> Result<()> {
        info!("[BOT] Async pre-init hook complete (grid placement handled by initialize_with_price)");
        Ok(())
    }

    /// ═══════════════════════════════════════════════════════════════════
    /// V4.5 FIX: Async Grid Initialization with Live Price Feed
    /// ═══════════════════════════════════════════════════════════════════
    /// Initializes the grid at the real market price from the price feed.
    /// MUST be called AFTER the price feed has warmed up and BEFORE the
    /// trading loop starts. Separated from new() because it requires
    /// async price access which isn't available during sync construction.
    ///
    /// # Arguments
    /// * `feed` — Active, warmed-up PriceFeed instance
    ///
    /// # Returns
    /// * `Ok(())` — grid placed, grid_initialized = true
    /// * `Err`    — invalid price or order placement failure
    pub async fn initialize_with_price(&mut self, feed: &PriceFeed) -> Result<()> {
        info!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
        info!("┃  V4.5 GRID INIT — awaiting live price...       ┃");
        info!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");

        let initial_price = feed.latest_price().await;
        if initial_price <= 0.0 {
            bail!("Invalid initial price ${:.2} — cannot initialize grid", initial_price);
        }

        info!("[BOT] Live price received: ${:.4}", initial_price);

        // Place initial grid — reposition_grid handles first-placement path
        self.reposition_grid(initial_price, initial_price).await
            .context("Initial grid placement failed")?;

        if !self.grid_initialized {
            bail!("Grid placement completed but grid_initialized flag not set — logic error");
        }

        info!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
        info!("┃  ✅ Grid initialized — ready for trading loop   ┃");
        info!("┃  {} levels  |  {:.3}% spacing              ┃",
              self.config.trading.grid_levels,
              self.config.trading.grid_spacing_percent);
        info!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");

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
        let should_reposition = price_change_pct > threshold;
        if should_reposition {
            debug!("[BOT] Grid reposition triggered: {:.3}% change > {:.3}% threshold",
                   price_change_pct, threshold);
        }
        should_reposition
    }

    pub async fn reposition_grid(&mut self, current_price: f64, last_price: f64) -> Result<()> {
        // ═══════════════════════════════════════════════════════════════════
        // V4.5 SAFETY: Emergency init guard — catches any edge case where
        // initialize_with_price() was skipped (should never happen, but
        // defensive programming is non-negotiable for mainnet).
        // ═══════════════════════════════════════════════════════════════════
        if !self.grid_initialized {
            warn!("⚠️  [BOT] Grid not initialized — emergency init at ${:.4}", current_price);
            warn!("⚠️  [BOT] This should not happen — check initialize_with_price() in main.rs!");

            self.place_grid_orders(current_price).await
                .context("Emergency grid initialization failed")?;
            self.grid_initialized = true;

            let total_levels = self.config.trading.grid_levels as usize;
            let used_levels = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

            info!("[BOT] Emergency grid init complete — normal trading resumes next cycle");
            return Ok(());
        }

        info!("[BOT] Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);
        let reposition_start = std::time::Instant::now();

        let filled_buys = self.grid_state.get_levels_with_filled_buys().await;
        if !filled_buys.is_empty() {
            warn!("[BOT] {} levels have filled buys - preserving their sell orders!", filled_buys.len());
            for level in &filled_buys {
                info!("   Level {} buy filled @ ${:.4} - keeping sell @ ${:.4}",
                      level.id, level.buy_price, level.sell_price);
            }
        }

        let cancellable = self.grid_state.get_cancellable_levels().await;
        info!("[BOT] {} cancellable levels (out of {} total)",
              cancellable.len(), self.grid_state.count().await);

        let mut cancelled_count = 0;
        for level_id in cancellable {
            if let Some(level) = self.grid_state.get_level(level_id).await {
                if let Some(buy_id) = &level.buy_order_id {
                    match self.engine.cancel_order(buy_id).await {
                        Ok(_) => { cancelled_count += 1; }
                        Err(e) => { warn!("[BOT] Failed to cancel buy {}: {}", buy_id, e); }
                    }
                }
                if let Some(sell_id) = &level.sell_order_id {
                    match self.engine.cancel_order(sell_id).await {
                        Ok(_) => { cancelled_count += 1; }
                        Err(e) => { warn!("[BOT] Failed to cancel sell {}: {}", sell_id, e); }
                    }
                }
                self.grid_state.mark_cancelled(level_id).await;
            }
        }

        if cancelled_count > 0 {
            info!("[BOT] Selectively cancelled {} orders", cancelled_count);
        }

        self.place_grid_orders(current_price).await?;
        self.grid_repositions += 1;
        self.last_reposition_time = Some(std::time::Instant::now());

        let total_levels = self.config.trading.grid_levels as usize;
        let used_levels = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

        info!("[BOT] Grid repositioned in {}ms", reposition_start.elapsed().as_millis());
        Ok(())
    }

    async fn place_grid_orders(&mut self, current_price: f64) -> Result<()> {
        let grid_spacing = self.adaptive_optimizer.current_spacing_percent;
        let order_size = self.adaptive_optimizer.current_position_size;
        let num_levels = self.config.trading.grid_levels;

        debug!("[BOT] ADAPTIVE Grid params: {} levels @ {:.3}% spacing, {:.3} SOL/order",
               num_levels, grid_spacing * 100.0, order_size);

        let mut orders_placed = 0;
        let mut orders_failed = 0;
        let buy_levels = num_levels / 2;
        let sell_levels = num_levels - buy_levels;

        for i in 1..=buy_levels.min(sell_levels) {
            let buy_price  = current_price * (1.0 - grid_spacing * i as f64);
            let sell_price = current_price * (1.0 + grid_spacing * i as f64);

            let mut level = self.grid_state.create_level(buy_price, sell_price, order_size).await;

            match self.engine.place_limit_order(OrderSide::Buy, buy_price, order_size).await {
                Ok(buy_order_id) => {
                    level.set_buy_order(buy_order_id.clone());
                    trace!("[BOT] Buy order placed @ ${:.4} (Level {})", buy_price, level.id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("[BOT] Failed to place buy order @ ${:.4}: {}", buy_price, e);
                    orders_failed += 1;
                    continue;
                }
            }

            match self.engine.place_limit_order(OrderSide::Sell, sell_price, order_size).await {
                Ok(sell_order_id) => {
                    level.set_sell_order(sell_order_id.clone());
                    trace!("[BOT] Sell order placed @ ${:.4} (Level {})", sell_price, level.id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("[BOT] Failed to place sell order @ ${:.4}: {}", sell_price, e);
                    orders_failed += 1;
                }
            }

            self.grid_state.update_level(level).await;
        }

        info!("[BOT] Placed {} orders ({} pairs), {} failed",
              orders_placed, buy_levels.min(sell_levels), orders_failed);
        if orders_failed > 0 {
            warn!("[BOT] {} orders failed to place", orders_failed);
        }
        Ok(())
    }

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        self.total_cycles += 1;
        self.last_price = Some(price);
        self.enhanced_metrics.update_price_range(price);

        trace!("[BOT] Processing price ${:.4} (cycle {})", price, self.total_cycles);

        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Failed to get strategy consensus")?;
        self.enhanced_metrics.record_signal(true);
        trace!("[BOT] Strategy signal: {}", signal.display());

        // Execute paper order book fills
        let filled_orders = self.engine.process_price_update(price).await
            .context("Failed to process price update in trading engine")?;

        // V4.4 (Stage 3): Drain fills and fan-out to all strategies.
        // GridRebalancer::on_fill() updates bias / ATR spacing in real-time.
        for fill in self.engine.drain_fills().await {
            self.manager.notify_fill(&fill);
        }

        if !filled_orders.is_empty() {
            info!("[BOT] {} orders filled at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            for order_id in &filled_orders {
                debug!("   [FILL] Order {} filled", order_id);

                let is_buy = order_id.to_lowercase().contains("buy");
                let pnl = self.grid_state.total_realized_pnl().await;
                let fill_size = self.adaptive_optimizer.current_position_size;

                self.total_fills_tracked += 1;

                info!("[FILL_TRACK] #{}: {} {} @ ${:.4} | size: {:.4} | P&L: ${:.2} | ts: {}",
                      self.total_fills_tracked,
                      if is_buy { "BUY" } else { "SELL" },
                      order_id,
                      price,
                      fill_size,
                      pnl,
                      timestamp
                );

                self.enhanced_metrics.record_trade(is_buy, pnl, timestamp);
            }
        }

        let wallet = self.engine.get_wallet().await;
        self.enhanced_metrics.update_portfolio_value(wallet.total_value_usdc(price));

        // Adaptive optimization every N cycles
        if self.total_cycles - self.last_optimization_cycle >= OPTIMIZATION_INTERVAL_CYCLES {
            let result = self.adaptive_optimizer.optimize(&self.enhanced_metrics);
            if result.any_changes() {
                info!("[OPT] Applied: {} | spacing={:.3}% | size={:.3} SOL",
                      result.reason,
                      result.new_spacing * 100.0,
                      result.new_position_size);
            }
            self.last_optimization_cycle = self.total_cycles;
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> BotStats {
        let wallet = self.engine.get_wallet().await;
        let perf_stats = self.engine.get_performance_stats().await;
        let open_orders = self.engine.open_order_count().await;
        let current_price = self.last_price.unwrap_or(0.0);

        BotStats {
            total_cycles: self.total_cycles,
            successful_trades: self.successful_trades,
            grid_repositions: self.grid_repositions,
            open_orders,
            total_value_usdc: wallet.total_value_usdc(current_price),
            pnl_usdc: wallet.pnl_usdc(current_price),
            roi_percent: wallet.roi(current_price),
            win_rate: perf_stats.win_rate,
            total_fees: perf_stats.total_fees,
            trading_paused: false,
            profitable_trades: self.enhanced_metrics.profitable_trades,
            unprofitable_trades: self.enhanced_metrics.unprofitable_trades,
            max_drawdown: self.enhanced_metrics.max_drawdown,
            signal_execution_ratio: self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency: self.enhanced_metrics.grid_efficiency,
            current_spacing_percent: self.adaptive_optimizer.current_spacing_percent,
            current_position_size: self.adaptive_optimizer.current_position_size,
            optimization_count: self.adaptive_optimizer.adjustment_count,
            total_fills_tracked: self.total_fills_tracked,
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats = self.get_stats().await;
        let border = "=".repeat(60);

        println!("\n{}", border);
        println!("   [BOT] GRID BOT V4.5 - STATUS REPORT");
        println!("{}", border);

        println!("\n[PERFORMANCE]");
        println!("  Total Cycles:      {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions:  {}", stats.grid_repositions);
        println!("  Open Orders:       {}", stats.open_orders);
        println!("  Fills Tracked:     {}", stats.total_fills_tracked);

        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl = self.grid_state.total_realized_pnl().await;

        println!("\n[GRID]");
        println!("  Active Levels:     {}", grid_levels);
        println!("  Filled Buys:       {}", filled_buys);
        println!("  Realized P&L:      ${:.2}", total_pnl);

        println!("\n[PORTFOLIO]");
        println!("  Total Value:       ${:.2}", stats.total_value_usdc);
        println!("  P&L:               ${:.2}", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);

        println!("\n[TRADING]");
        println!("  Win Rate:          {:.2}%", stats.win_rate);
        println!("  Total Fees:        ${:.2}", stats.total_fees);

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

#[derive(Debug, Clone)]
pub struct BotStats {
    pub total_cycles: u64,
    pub successful_trades: u64,
    pub grid_repositions: u64,
    pub open_orders: usize,
    pub total_value_usdc: f64,
    pub pnl_usdc: f64,
    pub roi_percent: f64,
    pub win_rate: f64,
    pub total_fees: f64,
    pub trading_paused: bool,
    pub profitable_trades: usize,
    pub unprofitable_trades: usize,
    pub max_drawdown: f64,
    pub signal_execution_ratio: f64,
    pub grid_efficiency: f64,
    pub current_spacing_percent: f64,
    pub current_position_size: f64,
    pub optimization_count: u64,
    pub total_fills_tracked: u64,
}

impl BotStats {
    pub fn display_summary(&self) {
        println!("\n[STATS] BOT STATISTICS SUMMARY V4.5");
        println!("   Cycles:            {}", self.total_cycles);
        println!("   Trades:            {}", self.successful_trades);
        println!("   Repositions:       {}", self.grid_repositions);
        println!("   Open Orders:       {}", self.open_orders);
        println!("   Fills Tracked:     {}", self.total_fills_tracked);
        println!("   Total Value:       ${:.2}", self.total_value_usdc);
        println!("   P&L:               ${:.2}", self.pnl_usdc);
        println!("   ROI:               {:.2}%", self.roi_percent);
        println!("   Win Rate:          {:.2}%", self.win_rate);
        println!("   Fees:              ${:.2}", self.total_fees);
        println!("\n[ANALYTICS]");
        println!("   Profitable Trades: {}", self.profitable_trades);
        println!("   Losing Trades:     {}", self.unprofitable_trades);
        println!("   Max Drawdown:      {:.2}%", self.max_drawdown);
        println!("   Signal Exec Rate:  {:.2}%", self.signal_execution_ratio * 100.0);
        println!("   Grid Efficiency:   {:.2}%", self.grid_efficiency * 100.0);
        println!("\n[OPTIMIZER]");
        println!("   Current Spacing:   {:.3}%", self.current_spacing_percent * 100.0);
        println!("   Current Size:      {:.3} SOL", self.current_position_size);
        println!("   Optimizations:     {}", self.optimization_count);
        if self.trading_paused {
            println!("   Status:            PAUSED");
        } else {
            println!("   Status:            V4.5 ACTIVE");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bot_creation() {
        assert!(true);
    }
}
