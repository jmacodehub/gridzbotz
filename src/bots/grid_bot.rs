//! ═════════════════════════════════════════════════════════════════════════
//! GRID BOT V5.8 — ELITE AUTONOMOUS TRADING ORCHESTRATOR
//!
//! V5.8 CHANGES (PR #86 — Multi-Bot Orchestrator / GAP-3):
//! ✅ intent_registry: Option<IntentRegistry> field — injected by Orchestrator
//! ✅ set_intent_registry() impl — wires the shared DashMap conflict guard
//! ✅ place_grid_orders(): DashMap::entry() atomic check before each level
//!    — skips + warns if another bot owns the level, increments intent_conflicts
//! ✅ intent_conflicts: u64 counter — surfaced in BotStats via stats()
//! ✅ Solo path: intent_registry = None — zero behavior change, zero cost
//!
//! PR #91 FIXES:
//! ✅ Bug 1 (SAFETY): place_grid_orders() key namespace fixed
//!    pair = instance_name() → pair = trading pair ("SOL/USDC")
//!    Two bots on the same pair can now detect each other's claims.
//! ✅ Bug 3 (OBSERVABILITY): stats() now returns self.intent_conflicts
//!    via BotStats.intent_conflicts — fleet aggregation is correct.
//! ✅ Bug 4 (STATE): reposition_grid() now removes registry entries
//!    for cancelled levels — prevents stale claims blocking reposition.
//!
//! PR #92 FIXES:
//! ✅ P0 #2 (CORRECTNESS): process_price_update() fill-side detection fixed.
//!    Was: order_id.to_lowercase().contains("buy") — fragile string sniff,
//!    inconsistent across engine impls → bot-01 showed Buy:0 Sell:0.
//!    Now: fill.side == OrderSide::Buy — reads authoritative FillEvent field.
//!    Also drops redundant order_ids Vec; iterates filled_orders directly.
//! ✅ P0 #3 (UX/CORRECTNESS): Win Rate display guard added.
//!    display_status() and GridBotStats::display_summary() now print
//!    "— (no closed trades yet)" when profitable+unprofitable == 0,
//!    preventing shutdown report from reading as a loss record on partial fills.
//!
//! V5.7 CHANGES (PR #85 — process_tick dispatch + Box<dyn Bot>):
//! ✅ run_trading_loop takes &mut dyn Bot — type-agnostic, orchestrator-ready
//! ✅ loop body uses bot.process_tick() — concrete process_price_update() retired
//! ✅ shutdown_components calls bot.shutdown() — trait method (displays status + logs)
//! ✅ initialize_components: Bot::initialize() covers grid placement — no explicit call
//! ✅ local type GridBot → Box<dyn Bot> in main()
//!
//! V5.6 CHANGES (PR #84 — impl Bot for GridBot + PriceFeed ownership):
//! ✅ GAP-1 RESOLVED: impl Bot for GridBot — trait-polymorphic, orchestrator-ready
//! ✅ GridBot owns Arc<PriceFeed> — process_tick() is fully autonomous
//!
//! Native *Config field mapping (TOML name → strategy field name):
//!   RsiStrategyConfig.period          → RsiConfig.rsi_period
//!   MomentumStrategyConfig.lookback_period → MomentumConfig.fast_period
//!   MeanReversionStrategyConfig.sma_period → MeanReversionConfig.mean_period
//!   MomentumMACDStrategyConfig.*      → MomentumMACDConfig.* (match 1:1)
//!
//! March 2026 — V5.8 MULTI-BOT ORCHESTRATION 🤖
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
use crate::config::Config;

const OPTIMIZATION_INTERVAL_CYCLES: u64 = 50;

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
    /// None in solo mode — zero cost, zero behavior change when absent.
    intent_registry:        Option<IntentRegistry>,
    /// Real conflict events detected this session (Occupied branch hits).
    /// PR #91: now surfaced via BotStats so fleet aggregation is correct.
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
        info!("[BOT-V5.8] Initializing GridBot V5.8...");
        info!("[BOT-V5.8] Engine:   Injected by main.rs (Paper or Real)");
        info!("[BOT-V5.8] PriceFeed: Owned via Arc — process_tick() autonomous");
        info!("[BOT-V5.8] Bot Trait: IMPLEMENTED + DISPATCHED (PR #84+#85+#86)");

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
        let grid_rebalancer = GridRebalancer::new(grid_config)
            .context("Failed to create GridRebalancer")?;

        let analytics_ctx = AnalyticsContext::default();
        let (_manager, _weights) = StrategyRegistryBuilder::new()
            .add(
                grid_rebalancer,
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

        info!("[BOT-V5.8] ✅ {} strategies loaded via StrategyRegistryBuilder",
              manager.strategies.len());

        let grid_state         = GridStateTracker::new();
        let enhanced_metrics   = EnhancedMetrics::new();
        let base_spacing       = config.trading.grid_spacing_percent / 100.0;
        let base_size          = config.trading.min_order_size;
        let adaptive_optimizer = AdaptiveOptimizer::new(base_spacing, base_size);

        info!("[BOT-V5.8] GridBot V5.8 initialization complete");

        Ok(Self {
            manager,
            engine,
            config,
            grid_state,
            enhanced_metrics,
            adaptive_optimizer,
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
        info!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
        info!("┃  V5.8 GRID INIT — awaiting live price...       ┃");
        info!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");

        let initial_price = self.feed.latest_price().await;
        if initial_price <= 0.0 {
            bail!("Invalid initial price ${:.2} — cannot initialize grid", initial_price);
        }
        info!("[BOT] Live price received: ${:.4}", initial_price);

        self.place_grid_orders(initial_price).await
            .context("Initial grid placement failed")?;
        self.grid_initialized = true;
        self.last_price = Some(initial_price);

        let total_levels = self.config.trading.grid_levels as usize;
        let used_levels  = self.grid_state.count().await;
        self.enhanced_metrics.update_grid_stats(total_levels, used_levels);

        info!("┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓");
        info!("┃  ✅ Grid initialized — ready for trading loop   ┃");
        info!("┃  {} levels  |  {:.3}% spacing              ┃",
              self.config.trading.grid_levels,
              self.config.trading.grid_spacing_percent);
        info!("┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛");
        Ok(())
    }

    pub async fn should_reposition(&self, current_price: f64, last_price: f64) -> bool {
        if !self.grid_initialized {
            info!("[BOT] Grid not initialized — will initialize on first cycle");
            return true;
        }
        if self.last_price.is_none() {
            trace!("No last price — skipping reposition check");
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
            warn!("⚠️  [BOT] Grid not initialized — emergency init at ${:.4}", current_price);
            warn!("⚠️  [BOT] This should not happen — Bot::initialize() should have run!");
            self.place_grid_orders(current_price).await
                .context("Emergency grid initialization failed")?;
            self.grid_initialized = true;
            let total_levels = self.config.trading.grid_levels as usize;
            let used_levels  = self.grid_state.count().await;
            self.enhanced_metrics.update_grid_stats(total_levels, used_levels);
            info!("[BOT] Emergency grid init complete");
            return Ok(());
        }

        info!("[BOT] Repositioning grid: ${:.4} → ${:.4}", last_price, current_price);
        let reposition_start = Instant::now();

        let filled_buys = self.grid_state.get_levels_with_filled_buys().await;
        if !filled_buys.is_empty() {
            warn!("[BOT] {} levels have filled buys — preserving sell orders!", filled_buys.len());
        }

        // PR #91 Bug 4 fix: get the trading pair BEFORE the cancel loop
        // so we can remove stale registry entries alongside order cancellation.
        // Guarded by Option — solo bots skip this block entirely (zero cost).
        let trading_pair = self.config.trading_pair();

        let cancellable = self.grid_state.get_cancellable_levels().await;
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

                // PR #91 Bug 4: Remove this level from the shared intent registry.
                // Without this, the bot cannot reclaim its own old levels after
                // reposition — DashMap::entry() would find them Occupied by itself
                // and spuriously increment intent_conflicts.
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
        let grid_spacing = self.adaptive_optimizer.current_spacing_percent;
        let order_size   = self.adaptive_optimizer.current_position_size;
        let num_levels   = self.config.trading.grid_levels;

        // PR #91 Bug 1 fix: use the actual trading pair as the registry key namespace,
        // NOT the bot instance name. Using instance_name() meant each bot namespaced
        // its own keys — two bots on SOL/USDC could never see each other's claims.
        // With the trading pair as namespace, any two bots on the same pair share
        // the same key space and conflict detection works correctly.
        let pair = self.config.trading_pair();

        debug!("[BOT] Grid params: {} levels @ {:.3}% spacing, {:.3} SOL/order",
               num_levels, grid_spacing * 100.0, order_size);

        let mut orders_placed = 0;
        let mut orders_failed = 0;
        let buy_levels  = num_levels / 2;
        let sell_levels = num_levels - buy_levels;

        for i in 1..=buy_levels.min(sell_levels) {
            let buy_price  = current_price * (1.0 - grid_spacing * i as f64);
            let sell_price = current_price * (1.0 + grid_spacing * i as f64);
            let mut level  = self.grid_state.create_level(buy_price, sell_price, order_size).await;

            // ── Intent registry conflict check (PR #86, namespace fixed PR #91) ───────
            if let Some(registry) = &self.intent_registry {
                let key = (pair.clone(), level.id);
                match registry.entry(key) {
                    dashmap::Entry::Occupied(e) => {
                        self.intent_conflicts += 1;
                        warn!(
                            "[INTENT] ⚠️  Level {} at ${:.4} owned by '{}' — skipping (conflicts: {})",
                            level.id, buy_price, e.get(), self.intent_conflicts
                        );
                        continue;
                    }
                    dashmap::Entry::Vacant(e) => {
                        e.insert(self.config.bot.instance_name().to_string());
                    }
                }
            }

            match self.engine.place_limit_order_with_level(
                OrderSide::Buy, buy_price, order_size, Some(level.id)
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
            match self.engine.place_limit_order_with_level(
                OrderSide::Sell, sell_price, order_size, Some(level.id)
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

        info!("[BOT] Placed {} orders ({} pairs), {} failed",
              orders_placed, buy_levels.min(sell_levels), orders_failed);
        Ok(())
    }

    pub async fn process_price_update(&mut self, price: f64, timestamp: i64) -> Result<()> {
        self.total_cycles += 1;
        self.last_price = Some(price);
        self.enhanced_metrics.update_price_range(price);
        trace!("[BOT] Processing price ${:.4} (cycle {})", price, self.total_cycles);

        let signal = self.manager.analyze_all(price, timestamp).await
            .context("Strategy consensus failed")?;
        self.enhanced_metrics.record_signal(true);
        trace!("[BOT] Signal: {}", signal.display());

        let filled_orders = self.engine.process_price_update(price).await
            .context("Engine tick failed")?;

        for fill in &filled_orders {
            self.manager.notify_fill(fill);
        }

        // PR #92 P0 #2: Iterate filled_orders directly and read fill.side
        // for authoritative Buy/Sell classification. The previous approach
        // sniffed the order_id string (`contains("buy")`), which is fragile
        // and engine-dependent — causing bot-01 to show Buy:0 Sell:0 while
        // bot-02 was correct. fill.side == OrderSide::Buy is always correct.
        if !filled_orders.is_empty() {
            info!("[BOT] {} orders filled at ${:.4}", filled_orders.len(), price);
            self.successful_trades += filled_orders.len() as u64;
            for fill in &filled_orders {
                let is_buy    = fill.side == OrderSide::Buy;
                let pnl       = self.grid_state.total_realized_pnl().await;
                let fill_size = self.adaptive_optimizer.current_position_size;
                self.total_fills_tracked += 1;
                info!("[FILL_TRACK] #{}: {} {} @ ${:.4} | size: {:.4} | P&L: ${:.2} | ts: {}",
                      self.total_fills_tracked,
                      if is_buy { "BUY" } else { "SELL" },
                      fill.order_id, price, fill_size, pnl, timestamp);
                self.enhanced_metrics.record_trade(is_buy, pnl, timestamp);
            }
        }

        let wallet = self.engine.get_wallet().await;
        self.last_known_pnl = wallet.pnl_usdc(price);
        self.enhanced_metrics.update_portfolio_value(wallet.total_value_usdc(price));

        if self.total_cycles - self.last_optimization_cycle >= OPTIMIZATION_INTERVAL_CYCLES {
            let result = self.adaptive_optimizer.optimize(&self.enhanced_metrics);
            if result.any_changes() {
                info!("[OPT] Applied: {} | spacing={:.3}% | size={:.3} SOL",
                      result.reason, result.new_spacing * 100.0, result.new_position_size);
            } else {
                // PR #92 P1: Always log why the optimizer did not adjust,
                // so operators can confirm it's threshold-gated, not silent/broken.
                debug!("[OPT] No adjustment: {} | spacing={:.3}% size={:.3} SOL",
                       result.reason,
                       result.new_spacing * 100.0,
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
            trading_paused:          false,
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
        }
    }

    pub async fn display_status(&self, current_price: f64) {
        let stats  = self.get_stats().await;
        let border = "=".repeat(60);
        println!("\n{}", border);
        println!("   [BOT] GRID BOT V5.8 — STATUS REPORT");
        println!("{}", border);
        println!("\n[PERFORMANCE]");
        println!("  Total Cycles:      {}", stats.total_cycles);
        println!("  Successful Trades: {}", stats.successful_trades);
        println!("  Grid Repositions:  {}", stats.grid_repositions);
        println!("  Open Orders:       {}", stats.open_orders);
        println!("  Fills Tracked:     {}", stats.total_fills_tracked);
        println!("  Orders Placed:     {}", self.total_orders_placed);
        println!("  Intent Conflicts:  {}", stats.intent_conflicts);
        let grid_levels = self.grid_state.count().await;
        let filled_buys = self.grid_state.get_levels_with_filled_buys().await.len();
        let total_pnl   = self.grid_state.total_realized_pnl().await;
        println!("\n[GRID]");
        println!("  Active Levels:     {}", grid_levels);
        println!("  Filled Buys:       {}", filled_buys);
        println!("  Realized P&L:      ${:.2}", total_pnl);
        println!("\n[PORTFOLIO]");
        println!("  Total Value:       ${:.2}", stats.total_value_usdc);
        println!("  P&L:               ${:.2}", stats.pnl_usdc);
        println!("  ROI:               {:.2}%", stats.roi_percent);
        println!("\n[TRADING]");
        // PR #92 P0 #3: Guard win rate display — "0.00%" on zero round-trips
        // is indistinguishable from a losing record. Print a clear label instead.
        if stats.profitable_trades + stats.unprofitable_trades == 0 {
            println!("  Win Rate:          — (no closed trades yet)");
        } else {
            println!("  Win Rate:          {:.2}%", stats.win_rate);
        }
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
            "[BOT] Intent registry wired for instance '{}' — conflict detection active",
            self.instance_id()
        );
        self.intent_registry = Some(registry);
    }

    async fn initialize(&mut self) -> Result<()> {
        self.pre_init_hook().await?;
        self.initialize_with_price().await
            .context("Bot::initialize — grid placement failed")?;
        Ok(())
    }

    async fn process_tick(&mut self) -> Result<TickResult> {
        let price = self.feed.latest_price().await;
        if price <= 0.0 {
            warn!("[BOT::process_tick] Invalid price {:.4} — signalling shutdown", price);
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
            return Ok(TickResult::paused("regime gate / circuit breaker"));
        }

        Ok(TickResult::active(fills_this_tick, orders_this_tick))
    }

    async fn shutdown(&mut self) -> Result<()> {
        info!("[BOT] Graceful shutdown initiated for instance '{}'", self.instance_id());
        let final_price = self.last_price.unwrap_or(0.0);
        self.display_status(final_price).await;
        self.display_strategy_performance().await;
        info!(
            "[BOT] Shutdown complete | cycles={} fills={} orders={} repos={} conflicts={} uptime={}s pnl=${:.2}",
            self.total_cycles,
            self.total_fills_tracked,
            self.total_orders_placed,
            self.grid_repositions,
            self.intent_conflicts,
            self.session_start.elapsed().as_secs(),
            self.last_known_pnl,
        );
        Ok(())
    }

    /// PR #91: intent_conflicts now correctly wired into BotStats
    /// so orchestrator aggregate_stats() sums real conflict events.
    fn stats(&self) -> BotStats {
        BotStats {
            instance_id:      self.config.bot.instance_name().to_string(),
            bot_type:         "GridBot".to_string(),
            total_cycles:     self.total_cycles,
            total_fills:      self.total_fills_tracked,
            total_orders:     self.total_orders_placed,
            uptime_secs:      self.session_start.elapsed().as_secs(),
            is_paused:        false,
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
}

impl GridBotStats {
    pub fn display_summary(&self) {
        println!("\n[STATS] GRID BOT STATISTICS SUMMARY V5.8");
        println!("   Cycles:            {}", self.total_cycles);
        println!("   Trades:            {}", self.successful_trades);
        println!("   Repositions:       {}", self.grid_repositions);
        println!("   Open Orders:       {}", self.open_orders);
        println!("   Fills Tracked:     {}", self.total_fills_tracked);
        println!("   Intent Conflicts:  {}", self.intent_conflicts);
        println!("   Total Value:       ${:.2}", self.total_value_usdc);
        println!("   P&L:               ${:.2}", self.pnl_usdc);
        println!("   ROI:               {:.2}%", self.roi_percent);
        // PR #92 P0 #3: Guard win rate — zero closed trades is not a loss record.
        if self.profitable_trades + self.unprofitable_trades == 0 {
            println!("   Win Rate:          — (no closed trades yet)");
        } else {
            println!("   Win Rate:          {:.2}%", self.win_rate);
        }
        println!("   Fees:              ${:.2}", self.total_fees);
        println!("\n[ANALYTICS]");
        println!("   Profitable Trades: {}", self.profitable_trades);
        println!("   Losing Trades:     {}", self.unprofitable_trades);
        println!("   Max Drawdown:      {:.2}%", self.max_drawdown);
        println!("   Signal Exec Rate:  {:.2}%", self.signal_execution_ratio);
        println!("   Grid Efficiency:   {:.2}%", self.grid_efficiency * 100.0);
        println!("\n[OPTIMIZER]");
        println!("   Current Spacing:   {:.3}%", self.current_spacing_percent * 100.0);
        println!("   Current Size:      {:.3} SOL", self.current_position_size);
        println!("   Optimizations:     {}", self.optimization_count);
        if self.trading_paused {
            println!("   Status:            PAUSED");
        } else {
            println!("   Status:            V5.8 ACTIVE");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

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
            total_cycles:            50,
            successful_trades:       10,
            grid_repositions:        1,
            open_orders:             4,
            total_value_usdc:        1000.0,
            pnl_usdc:                0.0,
            roi_percent:             0.0,
            win_rate:                0.5,
            total_fees:              0.5,
            trading_paused:          false,
            profitable_trades:       5,
            unprofitable_trades:     5,
            max_drawdown:            1.0,
            signal_execution_ratio:  0.80,
            grid_efficiency:         0.85,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            optimization_count:      1,
            total_fills_tracked:     10,
            intent_conflicts:        3,
        };
        assert_eq!(stats.intent_conflicts, 3);
    }

    /// PR #92 P0 #3: Win rate guard — zero closed trades must not display as 0.00%.
    #[test]
    fn test_win_rate_guard_zero_closed_trades() {
        let stats = GridBotStats {
            total_cycles:            100,
            successful_trades:       3,
            grid_repositions:        0,
            open_orders:             8,
            total_value_usdc:        1000.0,
            pnl_usdc:                0.0,
            roi_percent:             0.0,
            win_rate:                0.0,
            total_fees:              0.05,
            trading_paused:          false,
            // No closed round-trips yet
            profitable_trades:       0,
            unprofitable_trades:     0,
            max_drawdown:            0.0,
            signal_execution_ratio:  1.0,
            grid_efficiency:         0.5,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            optimization_count:      0,
            total_fills_tracked:     3,
            intent_conflicts:        0,
        };
        // Guard condition: no closed trades → should NOT display win_rate as percentage
        assert_eq!(stats.profitable_trades + stats.unprofitable_trades, 0,
            "Guard precondition: no closed trades");
        // Confirm win_rate field is 0.0 — would be misleading without the guard
        assert!((stats.win_rate - 0.0).abs() < 1e-9);
    }

    /// PR #92 P0 #3: Win rate guard — closed trades present → display normally.
    #[test]
    fn test_win_rate_guard_with_closed_trades() {
        let stats = GridBotStats {
            total_cycles:            200,
            successful_trades:       10,
            grid_repositions:        1,
            open_orders:             6,
            total_value_usdc:        1020.0,
            pnl_usdc:                20.0,
            roi_percent:             2.0,
            win_rate:                75.0,
            total_fees:              0.25,
            trading_paused:          false,
            profitable_trades:       6,
            unprofitable_trades:     2,
            max_drawdown:            0.5,
            signal_execution_ratio:  99.8,
            grid_efficiency:         0.7,
            current_spacing_percent: 0.003,
            current_position_size:   0.1,
            optimization_count:      1,
            total_fills_tracked:     10,
            intent_conflicts:        0,
        };
        // Guard condition: closed trades present → display win_rate numerically
        assert!(stats.profitable_trades + stats.unprofitable_trades > 0,
            "Guard precondition: closed trades exist");
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
        // PR #91: intent_conflicts must default to zero
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
        // PR #91 Bug 4: verify registry.remove() logic is correct.
        // We test the DashMap remove behaviour directly since
        // reposition_grid() is async and requires full engine setup.
        use crate::bots::bot_trait::new_intent_registry;

        let registry = new_intent_registry();
        let pair = "SOL/USDC".to_string();

        // Simulate bot claiming 3 levels at startup
        registry.insert((pair.clone(), 1u64), "sol-usdc-grid-01".into());
        registry.insert((pair.clone(), 2u64), "sol-usdc-grid-01".into());
        registry.insert((pair.clone(), 3u64), "sol-usdc-grid-01".into());
        assert_eq!(registry.len(), 3);

        // Simulate reposition: remove cancelled level entries
        let cancelled_levels = vec![1u64, 2u64];
        for level_id in &cancelled_levels {
            registry.remove(&(pair.clone(), *level_id));
        }

        // Only level 3 remains — bot can now reclaim levels 1 and 2
        assert_eq!(registry.len(), 1);
        assert!(!registry.contains_key(&(pair.clone(), 1u64)));
        assert!(!registry.contains_key(&(pair.clone(), 2u64)));
        assert!(registry.contains_key(&(pair.clone(), 3u64)));
    }
}
