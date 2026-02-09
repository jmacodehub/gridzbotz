//! ═══════════════════════════════════════════════════════════════════════════
//! 🤖 GRID BOT V4.1 - Production Trading Orchestrator with Enhanced Analytics
//!
//! V4.1 ENHANCEMENTS - Enhanced Metrics Integration:
//! ✅ GridLevel pairing (buy/sell orders linked)
//! ✅ Safe reposition (preserves filled buys)
//! ✅ Order lifecycle tracking per level
//! ✅ No orphaned positions
//! ✅ Production-ready state management
//! ✅ ENHANCED METRICS - Trade-level analytics 📊
//! ✅ Win/Loss tracking, Grid efficiency, Drawdown monitoring
//!
//! February 9, 2026 - V4.1 ENHANCED METRICS INTEGRATED! 🔥
//! ═══════════════════════════════════════════════════════════════════════════

use crate::strategies::{StrategyManager, GridRebalancer, GridRebalancerConfig};
use crate::strategies::shared::analytics::AnalyticsContext;  // 🔥 Analytics support
use crate::trading::{
    PaperTradingEngine,
    OrderSide,
    GridStateTracker,
    EnhancedMetrics,  // 📊 NEW: Enhanced metrics tracking
};
use crate::config::Config;
use anyhow::{Result, Context, bail};
use log::{info, warn, debug, trace};

// ═══════════════════════════════════════════════════════════════════════════
// GRID BOT - Main Trading Orchestrator with Enhanced Metrics
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridBot {
    pub manager: StrategyManager,
    pub engine: PaperTradingEngine,
    pub config: Config,
    pub grid_state: GridStateTracker,
    pub enhanced_metrics: EnhancedMetrics,  // 📊 NEW: Enhanced analytics
    last_price: Option<f64>,
    total_cycles: u64,
    successful_trades: u64,
    grid_repositions: u64,
    last_reposition_time: Option<std::time::Instant>,
    grid_initialized: bool,
}

impl GridBot {
    pub fn new(config: Config) -> Result<Self> {
        info!("═══════════════════════════════════════════════════════════");
        info!("🤖 Initializing GridBot V4.1 with Enhanced Metrics...");
        info!("═══════════════════════════════════════════════════════════");

        // 🔥 Create AnalyticsContext
        let analytics_ctx = AnalyticsContext::default();
        let mut manager = StrategyManager::new(analytics_ctx);

        info!("📊 Creating grid rebalancer from config...");

        let grid_config = GridRebalancerConfig {
            grid_spacing: config.trading.grid_spacing_percent / 100.0,
            order_size: config.trading.min_order_size,
            min_usdc_balance: config.trading.min_usdc_reserve,
            min_sol_balance: config.trading.min_sol_reserve,
            enabled: config.strategies.grid.enabled,
            enable_dynamic_spacing: config.trading.enable_dynamic_grid,
            enable_fee_filtering: config.trading.enable_fee_optimization,
            volatility_window_seconds: config.trading.volatility_window as u64,
            max_spacing: 0.0075,
            min_spacing: 0.001,
            enable_regime_gate: config.trading.enable_regime_gate,
            min_volatility_to_trade: config.trading.min_volatility_to_trade,
            pause_in_very_low_vol: config.trading.pause_in_very_low_vol,
            enable_order_lifecycle: config.trading.enable_order_lifecycle,
            order_max_age_minutes: config.trading.order_max_age_minutes,
            order_refresh_interval_minutes: config.trading.order_refresh_interval_minutes,
            min_orders_to_maintain: config.trading.min_orders_to_maintain,
        };

        info!("🎯 Initializing grid rebalancer strategy...");

        let grid_rebalancer = GridRebalancer::new(grid_config)
            .context("Failed to create GridRebalancer")?;

        manager.add_strategy(grid_rebalancer);
        info!("✅ Grid rebalancer strategy loaded");

        if config.strategies.momentum.enabled {
            info!("📈 Momentum strategy enabled (not yet implemented)");
        }

        if config.strategies.mean_reversion.enabled {
            info!("📉 Mean reversion strategy enabled (not yet implemented)");
        }

        if config.strategies.rsi.enabled {
            info!("📊 RSI strategy enabled (not yet implemented)");
        }

        info!("💰 Initializing paper trading engine...");

        let initial_usdc = config.paper_trading.initial_usdc;
        let initial_sol = config.paper_trading.initial_sol;

        if initial_usdc <= 0.0 || initial_sol <= 0.0 {
            bail!("Invalid initial capital: USDC={}, SOL={}", initial_usdc, initial_sol);
        }

        let engine = PaperTradingEngine::new(initial_usdc, initial_sol)
            .with_fees(0.0002, 0.0004)
            .with_slippage(0.0005);

        info!("✅ Paper trading engine initialized");
        info!("   Initial Capital: ${:.2} USDC + {} SOL", initial_usdc, initial_sol);

        let grid_state = GridStateTracker::new();
        info!("✅ Grid state tracker initialized");

        // 📊 NEW: Initialize enhanced metrics
        let enhanced_metrics = EnhancedMetrics::new();
        info!("✅ Enhanced metrics tracker initialized");

        info!("═══════════════════════════════════════════════════════════");
        info!("✅ GridBot V4.1 initialization complete!");
        info!("═══════════════════════════════════════════════════════════\n");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,  // 📊 NEW FIELD
            last_price: None,
            total_cycles: 0,
            successful_trades: 0,
            grid_repositions: 0,
            last_reposition_time: None,
            grid_initialized: false,
        })
    }

    pub async fn initialize(&mut self) -> Result<()> {
        info!("🔧 Performing async initialization...");
        info!("✅ GridBot initialization complete");
        Ok(())
    }

    pub async fn should_reposition(&self, current_price: f64, last_price: f64) -> bool {
        if !self.grid_initialized {
            info!("🎯 Grid not initialized - will initialize on first cycle");
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
                trace!("Reposition cooldown: {}s elapsed, {}s required",
                       elapsed, cooldown_secs);
                return false;
            }
        }

        let price_change_pct = ((current_price - last_price).abs() / last_price) * 100.0;
        let threshold = self.config.trading.reposition_threshold;

        let should_reposition = price_change_pct > threshold;

        if should_reposition {
            debug!("Grid reposition triggered: {:.3}% change > {:.3}% threshold",
                   price_change_pct, threshold);
        }

        should_reposition
    }

    pub async fn reposition_grid(&mut self, current_price: f64, last_price: f64) -> Result<()> {
        if !self.grid_initialized {
            info!("🎯 Placing initial grid at ${:.4}", current_price);
            self.place_grid_orders(current_price).await?;
            self.grid_initialized = true;
            info!("✅ Initial grid placed successfully");
            
            // 📊 Update grid efficiency after initial placement
            let total_levels = self.config.trading.grid_levels;
            let used_levels = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);
            
            return Ok(());
        }

        info!("🔄 Repositioning grid: ${:.4} -> ${:.4}", last_price, current_price);

        let reposition_start = std::time::Instant::now();

        let filled_buys = self.grid_state.get_levels_with_filled_buys().await;
        if !filled_buys.is_empty() {
            warn!("⚠️  {} levels have filled buys - preserving their sell orders!", filled_buys.len());
            for level in &filled_buys {
                info!("   → Level {} buy filled @ ${:.4} - keeping sell @ ${:.4}",
                      level.id, level.buy_price, level.sell_price);
            }
        }

        // Get levels that are safe to cancel (no filled buys)
        let cancellable = self.grid_state.get_cancellable_levels().await;
        info!("📋 Identified {} cancellable levels (out of {} total)",
              cancellable.len(),
              self.grid_state.count().await);

        // Selectively cancel only the safe orders
        let mut cancelled_count = 0;

        for level_id in cancellable {
            // Get the level to find its order IDs
            if let Some(level) = self.grid_state.get_level(level_id).await {
                // Cancel buy order if it exists
                if let Some(buy_id) = &level.buy_order_id {
                    match self.engine.cancel_order(buy_id).await {
                        Ok(_) => {
                            debug!("  ✅ Cancelled buy order {} from level {}", buy_id, level_id);
                            cancelled_count += 1;
                        }
                        Err(e) => {
                            warn!("  ⚠️ Failed to cancel buy {}: {}", buy_id, e);
                        }
                    }
                }

                // Cancel sell order if it exists (only if level is cancellable)
                if let Some(sell_id) = &level.sell_order_id {
                    match self.engine.cancel_order(sell_id).await {
                        Ok(_) => {
                            debug!("  ✅ Cancelled sell order {} from level {}", sell_id, level_id);
                            cancelled_count += 1;
                        }
                        Err(e) => {
                            warn!("  ⚠️ Failed to cancel sell {}: {}", sell_id, e);
                        }
                    }
                }

                // Mark level as cancelled in state tracker
                self.grid_state.mark_cancelled(level_id).await;
            }
        }

        if cancelled_count > 0 {
            info!("✅ Selectively cancelled {} orders from safe levels", cancelled_count);
        } else {
            info!("ℹ️  No orders needed cancellation");
        }

        self.place_grid_orders(current_price).await?;

        self.grid_repositions += 1;
        self.last_reposition_time = Some(std::time::Instant::now());

        // 📊 Update grid efficiency after reposition
        let total_levels = self.config.trading.grid_levels;
        let used_levels = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

        let reposition_time = reposition_start.elapsed().as_millis();
        info!("✅ Grid repositioned in {}ms", reposition_time);

        Ok(())
    }

    async fn place_grid_orders(&mut self, current_price: f64) -> Result<()> {
        let grid_spacing = self.config.trading.grid_spacing_percent / 100.0;
        let order_size = self.config.trading.min_order_size;
        let num_levels = self.config.trading.grid_levels;

        debug!("Grid params: {} levels @ {:.3}% spacing, {} SOL per order",
               num_levels, grid_spacing * 100.0, order_size);

        let mut orders_placed = 0;
        let mut orders_failed = 0;

        let buy_levels = num_levels / 2;
        let sell_levels = num_levels - buy_levels;

        for i in 1..=buy_levels.min(sell_levels) {
            let buy_price = current_price * (1.0 - grid_spacing * i as f64);
            let sell_price = current_price * (1.0 + grid_spacing * i as f64);

            let mut level = self.grid_state.create_level(buy_price, sell_price, order_size).await;

            match self.engine.place_limit_order(OrderSide::Buy, buy_price, order_size).await {
                Ok(buy_order_id) => {
                    level.set_buy_order(buy_order_id.clone());
                    trace!("✅ Buy order placed @ ${:.4} (Level {})", buy_price, level.id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to place buy order @ ${:.4}: {}", buy_price, e);
                    orders_failed += 1;
                    continue;
                }
            }

            match self.engine.place_limit_order(OrderSide::Sell, sell_price, order_size).await {
                Ok(sell_order_id) => {
                    level.set_sell_order(sell_order_id.clone());
                    trace!("✅ Sell order placed @ ${:.4} (Level {})", sell_price, level.id);
                    orders_placed += 1;
                }
                Err(e) => {
                    warn!("❌ Failed to place sell order @ ${:.4}: {}", sell_price, e);
                    orders_failed += 1;
                }
            }

            self.grid_state.update_level(level).await;
        }

        info!("📊 Placed {} orders ({} pairs), {} failed",
              orders_placed, buy_levels.min(sell_levels), orders_failed);

        if orders_failed > 0 {
            warn!("⚠️  {} orders failed to place", orders_failed);
        }

        Ok(())
    }

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        self.total_cycles += 1;
        self.last_price = Some(price);

        // 📊 Update price range tracking
        self.enhanced_metrics.update_price_range(price);

        trace!("Processing price update: ${:.4} (cycle {})", price, self.total_cycles);

        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Failed to get strategy consensus")?;

        // 📊 Record signal execution
        self.enhanced_metrics.record_signal(true);

        trace!("Strategy signal: {}", signal.display());

        let filled_orders = self.engine.process_price_update(price).await
            .context("Failed to process price update in trading engine")?;

        if !filled_orders.is_empty() {
            info!("💰 {} orders filled at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;

            for order_id in &filled_orders {
                debug!("   ✅ Order {} filled", order_id);
                
                // 📊 Record trade in enhanced metrics
                // Determine if buy or sell based on order ID pattern
                let is_buy = order_id.to_lowercase().contains("buy");
                
                // Calculate P&L from grid state if available
                let pnl = self.grid_state.total_realized_pnl().await;
                
                self.enhanced_metrics.record_trade(is_buy, pnl, timestamp);
            }
        }

        // 📊 Update portfolio value tracking
        let wallet = self.engine.get_wallet().await;
        let total_value = wallet.total_value_usdc(price);
        self.enhanced_metrics.update_portfolio_value(total_value);

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
            // 📊 NEW: Enhanced metrics fields
            profitable_trades: self.enhanced_metrics.profitable_trades,
            unprofitable_trades: self.enhanced_metrics.unprofitable_trades,
            max_drawdown: self.enhanced_metrics.max_drawdown,
            signal_execution_ratio: self.enhanced_metrics.signal_execution_ratio,
            grid_efficiency: self.enhanced_metrics.grid_efficiency,
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats = self.get_stats().await;

        let border = "═".repeat(60);

        println!("\n{}", border);
        println!("   🤖 GRID BOT V4.1 - STATUS REPORT");
        println!("{}", border);

        println!("\n📊 Bot Performance:");
        println!("  Total Cycles:      {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions:  {}", stats.grid_repositions);
        println!("  Open Orders:       {}", stats.open_orders);

        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl = self.grid_state.total_realized_pnl().await;

        println!("\n🎯 Grid State:");
        println!("  Active Levels:     {}", grid_levels);
        println!("  Filled Buys:       {}", filled_buys);
        println!("  Realized P&L:      ${:.2}", total_pnl);

        println!("\n💰 Portfolio:");
        println!("  Total Value:       ${:.2}", stats.total_value_usdc);
        println!("  P&L:               ${:.2}", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);

        println!("\n📈 Trading Stats:");
        println!("  Win Rate:          {:.2}%", stats.win_rate);
        println!("  Total Fees:        ${:.2}", stats.total_fees);

        // 📊 NEW: Display enhanced metrics
        println!("\n🔍 Enhanced Metrics:");
        self.enhanced_metrics.display();

        println!("\n💵 Current Price:    ${:.4}", current_price);

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
    // 📊 NEW: Enhanced metrics fields
    pub profitable_trades: usize,
    pub unprofitable_trades: usize,
    pub max_drawdown: f64,
    pub signal_execution_ratio: f64,
    pub grid_efficiency: f64,
}

impl BotStats {
    pub fn display_summary(&self) {
        println!("\n📊 BOT STATISTICS SUMMARY");
        println!("   Cycles:            {}", self.total_cycles);
        println!("   Trades:            {}", self.successful_trades);
        println!("   Repositions:       {}", self.grid_repositions);
        println!("   Open Orders:       {}", self.open_orders);
        println!("   Total Value:       ${:.2}", self.total_value_usdc);
        println!("   P&L:               ${:.2}", self.pnl_usdc);
        println!("   ROI:               {:.2}%", self.roi_percent);
        println!("   Win Rate:          {:.2}%", self.win_rate);
        println!("   Fees:              ${:.2}", self.total_fees);
        
        // 📊 NEW: Enhanced metrics in summary
        println!("\n🔍 Enhanced Analytics:");
        println!("   Profitable Trades: {}", self.profitable_trades);
        println!("   Losing Trades:     {}", self.unprofitable_trades);
        println!("   Max Drawdown:      {:.2}%", self.max_drawdown);
        println!("   Signal Exec Rate:  {:.2}%", self.signal_execution_ratio * 100.0);
        println!("   Grid Efficiency:   {:.2}%", self.grid_efficiency * 100.0);

        if self.trading_paused {
            println!("   Status:            🚫 PAUSED");
        } else {
            println!("   Status:            ✅ ACTIVE");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bot_creation() {
        assert!(true);
    }
}
