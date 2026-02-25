//! =============================================================================
//! PAPER TRADING ENGINE V3.5 - Risk-Free Strategy Testing
//! Production-Ready | Enhanced | Optimized | Modular
//! October 16, 2025 -- V3.1 February 2026 (TradingEngine trait impl)
//! V3.2 February 2026 (FillEvent -- Stage 3 / Step 2)
//! V3.3 February 2026 (Realised P&L per grid level -- Stage 3 / Step 3A)
//! V3.4 February 2026 (Fill persistence -- Stage 3 / Step 3B)
//! V3.5 February 2026 (Wire grid_spacing into CSV -- Stage 3 / Step 3C)
//! =============================================================================
//!
//! Features:
//! - Virtual wallet with multi-token support
//! - Realistic order execution simulation
//! - Grid trading strategy support
//! - Real-time P&L tracking
//! - Order book and trade history
//! - Performance analytics
//! - Slippage and fee simulation
//! - Thread-safe with async support
//! - Builder pattern for configuration
//! - V3.1: impl TradingEngine -- satisfies Arc<dyn TradingEngine>
//! - V3.2: process_price_update returns Vec<FillEvent> (Stage 3 Step 2)
//! - V3.3: FillEvent.pnl populated on paired sell fills (Stage 3 Step 3A)
//! - V3.4: Optional FillLogger appends every fill to CSV (Stage 3 Step 3B)
//! - V3.5: grid_spacing wired into CSV rows (Stage 3 Step 3C)

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use async_trait::async_trait;
use log::{debug, warn};

use super::{TradingEngine, TradingResult, FillEvent};
use super::fill_logger::FillLogger;

// =============================================================================
// CONSTANTS
// =============================================================================

const DEFAULT_MAKER_FEE: f64 = 0.0002;  // 0.02% OpenBook maker fee
const DEFAULT_TAKER_FEE: f64 = 0.0004;  // 0.04% OpenBook taker fee
const DEFAULT_SLIPPAGE: f64  = 0.0005;  // 0.05% default slippage
const MAX_TRADE_HISTORY: usize = 10000;

// =============================================================================
// DATA STRUCTURES
// =============================================================================

/// Order side (buy or sell)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    Buy,
    Sell,
}

/// Order status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    Open,
    PartiallyFilled,
    Filled,
    Cancelled,
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    Limit,
    Market,
}

/// An order in the paper trading system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// The canonical order ID.  For orders placed via
    /// `place_limit_order_with_level`, this is the TAGGED form
    /// (`ORDER-000001-L5`), not the bare HashMap key (`ORDER-000001`).
    pub id: String,
    pub side: OrderSide,
    pub order_type: OrderType,
    pub status: OrderStatus,
    pub price: f64,
    pub size: f64,
    pub filled_size: f64,
    pub created_at: i64,
    pub filled_at: Option<i64>,
    pub fee_paid: f64,
}

impl Order {
    fn new(id: String, side: OrderSide, order_type: OrderType, price: f64, size: f64) -> Self {
        Self {
            id,
            side,
            order_type,
            status: OrderStatus::Open,
            price,
            size,
            filled_size: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            filled_at: None,
            fee_paid: 0.0,
        }
    }
}

/// A completed trade
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    pub order_id: String,
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub fee: f64,
    pub timestamp: i64,
    pub pnl: Option<f64>,
}

/// Virtual wallet holding token balances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualWallet {
    pub balances: HashMap<String, f64>,
    pub initial_balance_usdc: f64,
    pub initial_balance_sol: f64,
}

impl VirtualWallet {
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        let mut balances = HashMap::new();
        balances.insert("USDC".to_string(), initial_usdc);
        balances.insert("SOL".to_string(), initial_sol);
        log::info!("[Wallet] Initialized: ${:.2} USDC, {:.4} SOL", initial_usdc, initial_sol);
        Self { balances, initial_balance_usdc: initial_usdc, initial_balance_sol: initial_sol }
    }

    pub fn get_balance(&self, token: &str) -> f64 {
        *self.balances.get(token).unwrap_or(&0.0)
    }

    pub fn set_balance(&mut self, token: &str, amount: f64) {
        self.balances.insert(token.to_string(), amount);
    }

    pub fn add_balance(&mut self, token: &str, amount: f64) {
        let current = self.get_balance(token);
        self.set_balance(token, current + amount);
    }

    pub fn sub_balance(&mut self, token: &str, amount: f64) -> Result<()> {
        let current = self.get_balance(token);
        if current < amount {
            bail!("Insufficient {} balance: have {:.4}, need {:.4}", token, current, amount);
        }
        self.set_balance(token, current - amount);
        Ok(())
    }

    pub fn total_value_usdc(&self, sol_price: f64) -> f64 {
        self.get_balance("USDC") + (self.get_balance("SOL") * sol_price)
    }

    pub fn roi(&self, sol_price: f64) -> f64 {
        let current = self.total_value_usdc(sol_price);
        let initial = self.initial_balance_usdc + (self.initial_balance_sol * sol_price);
        if initial == 0.0 { return 0.0; }
        ((current - initial) / initial) * 100.0
    }

    pub fn pnl_usdc(&self, sol_price: f64) -> f64 {
        self.total_value_usdc(sol_price)
            - (self.initial_balance_usdc + self.initial_balance_sol * sol_price)
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub total_trades:   usize,
    pub winning_trades: usize,
    pub losing_trades:  usize,
    pub total_pnl:      f64,
    pub total_fees:     f64,
    pub win_rate:       f64,
    pub avg_win:        f64,
    pub avg_loss:       f64,
    pub largest_win:    f64,
    pub largest_loss:   f64,
    pub profit_factor:  f64,
}

// =============================================================================
// PAPER TRADING ENGINE
// =============================================================================

/// Main paper trading engine
#[derive(Clone)]
pub struct PaperTradingEngine {
    wallet:        Arc<RwLock<VirtualWallet>>,
    open_orders:   Arc<RwLock<HashMap<String, Order>>>,
    trade_history: Arc<RwLock<VecDeque<Trade>>>,
    maker_fee:     f64,
    taker_fee:     f64,
    slippage:      f64,
    next_order_id: Arc<RwLock<u64>>,

    // Step 3A: pending buy fills awaiting a paired sell.
    // Key = grid_level_id.  Value = (execution_price, fee_paid).
    buy_fills: Arc<RwLock<HashMap<u64, (f64, f64)>>>,

    // Step 3A: cumulative realised P&L keyed by grid level.
    level_pnl: Arc<RwLock<HashMap<u64, f64>>>,

    // Step 3B: optional CSV logger.  None = logging disabled (default).
    fill_logger: Option<Arc<FillLogger>>,

    // Step 3C: grid spacing in USD between adjacent levels.
    // Written to every CSV row for ML feature extraction.
    // Set via .with_grid_spacing(step).
    grid_spacing: Option<f64>,
}

impl PaperTradingEngine {
    /// Create a new paper trading engine with default settings.
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        log::info!("[PaperEngine] Initializing V3.5");
        Self {
            wallet:        Arc::new(RwLock::new(VirtualWallet::new(initial_usdc, initial_sol))),
            open_orders:   Arc::new(RwLock::new(HashMap::new())),
            trade_history: Arc::new(RwLock::new(VecDeque::new())),
            maker_fee:     DEFAULT_MAKER_FEE,
            taker_fee:     DEFAULT_TAKER_FEE,
            slippage:      DEFAULT_SLIPPAGE,
            next_order_id: Arc::new(RwLock::new(1)),
            buy_fills:     Arc::new(RwLock::new(HashMap::new())),
            level_pnl:     Arc::new(RwLock::new(HashMap::new())),
            fill_logger:   None,
            grid_spacing:  None,
        }
    }

    /// Enable CSV fill logging.
    ///
    /// Every `FillEvent` will be appended to `{dir}/fills_YYYYMMDD.csv`.
    /// The directory is created if it does not exist.  Write errors are
    /// logged as warnings and never propagate.
    ///
    /// Call `.with_grid_spacing()` after this to populate the `spacing`
    /// column in every CSV row.
    pub fn with_fill_logging(mut self, dir: impl Into<PathBuf>) -> Self {
        match FillLogger::new(dir) {
            Ok(logger) => { self.fill_logger = Some(Arc::new(logger)); }
            Err(e)     => { warn!("[PaperEngine] Could not initialise fill logger: {}", e); }
        }
        self
    }

    /// Set the grid spacing (USD between adjacent price levels).
    ///
    /// Typically computed as `(upper - lower) / (num_levels - 1)` in the
    /// caller (e.g. `grid_bot.rs` or `gridz_bot.rs`).
    ///
    /// Written to the `spacing` column of every fill CSV row so the ML
    /// dataset captures the grid regime each trade occurred in.
    pub fn with_grid_spacing(mut self, step: f64) -> Self {
        self.grid_spacing = Some(step);
        log::info!("[PaperEngine] Grid spacing set to ${:.4} per level", step);
        self
    }

    pub fn with_fees(mut self, maker_fee: f64, taker_fee: f64) -> Self {
        self.maker_fee = maker_fee;
        self.taker_fee = taker_fee;
        log::info!("[PaperEngine] Custom fees: Maker {:.4}%, Taker {:.4}%",
            maker_fee * 100.0, taker_fee * 100.0);
        self
    }

    pub fn with_slippage(mut self, slippage: f64) -> Self {
        self.slippage = slippage;
        log::info!("[PaperEngine] Custom slippage: {:.4}%", slippage * 100.0);
        self
    }

    pub async fn place_limit_order(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
    ) -> Result<String> {
        let order_id = {
            let mut next_id = self.next_order_id.write().await;
            let id = format!("ORDER-{:06}", *next_id);
            *next_id += 1;
            id
        };

        let wallet = self.wallet.read().await;
        match side {
            OrderSide::Buy => {
                let required = price * size * (1.0 + self.taker_fee);
                if wallet.get_balance("USDC") < required {
                    bail!("Insufficient USDC: have ${:.2}, need ${:.2}",
                        wallet.get_balance("USDC"), required);
                }
            }
            OrderSide::Sell => {
                if wallet.get_balance("SOL") < size {
                    bail!("Insufficient SOL: have {:.4}, need {:.4}",
                        wallet.get_balance("SOL"), size);
                }
            }
        }
        drop(wallet);

        let order = Order::new(order_id.clone(), side, OrderType::Limit, price, size);
        self.open_orders.write().await.insert(order_id.clone(), order);
        debug!("[Order] {:?} placed: {:.4} SOL @ ${:.4} (ID: {})", side, size, price, order_id);
        Ok(order_id)
    }

    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let base_id = order_id
            .rsplit_once("-L")
            .filter(|(_, s)| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()))
            .map(|(base, _)| base)
            .unwrap_or(order_id);

        let mut orders = self.open_orders.write().await;
        if let Some(mut order) = orders.remove(base_id) {
            order.status = OrderStatus::Cancelled;
            debug!("[Order] Cancelled: {}", base_id);
            Ok(())
        } else {
            bail!("Order not found: {}", order_id);
        }
    }

    pub async fn cancel_all_orders(&self) -> Result<usize> {
        let mut orders = self.open_orders.write().await;
        let count = orders.len();
        orders.clear();
        if count > 0 { log::info!("[Order] Cancelled {} orders", count); }
        Ok(count)
    }

    /// Process price update and execute matching orders.
    ///
    /// V3.5: passes `self.grid_spacing` into `logger.append()` so the
    /// `spacing` column in the CSV is populated on every fill.
    pub async fn process_price_update(&self, current_price: f64) -> Result<Vec<FillEvent>> {
        let mut filled_events = Vec::new();
        let mut orders    = self.open_orders.write().await;
        let mut wallet    = self.wallet.write().await;
        let mut history   = self.trade_history.write().await;
        let mut buy_fills = self.buy_fills.write().await;
        let mut level_pnl = self.level_pnl.write().await;

        let order_ids: Vec<String> = orders.keys().cloned().collect();

        for order_id in order_ids {
            if let Some(mut order) = orders.remove(&order_id) {
                let should_fill = match order.side {
                    OrderSide::Buy  => current_price <= order.price,
                    OrderSide::Sell => current_price >= order.price,
                };

                if should_fill {
                    let execution_price = self.apply_slippage(order.price, order.side);
                    let fee = order.size * execution_price * self.maker_fee;

                    match order.side {
                        OrderSide::Buy => {
                            let cost = order.size * execution_price + fee;
                            if let Err(e) = wallet.sub_balance("USDC", cost) {
                                warn!("[Order] Buy {} failed: {}", order_id, e);
                                orders.insert(order_id, order);
                                continue;
                            }
                            wallet.add_balance("SOL", order.size);
                        }
                        OrderSide::Sell => {
                            if let Err(e) = wallet.sub_balance("SOL", order.size) {
                                warn!("[Order] Sell {} failed: {}", order_id, e);
                                orders.insert(order_id, order);
                                continue;
                            }
                            let proceeds = order.size * execution_price - fee;
                            wallet.add_balance("USDC", proceeds);
                        }
                    }

                    order.status      = OrderStatus::Filled;
                    order.filled_size = order.size;
                    order.filled_at   = Some(chrono::Utc::now().timestamp());
                    order.fee_paid    = fee;

                    let fill_ts = order.filled_at.unwrap();

                    history.push_back(Trade {
                        order_id:  order_id.clone(),
                        side:      order.side,
                        price:     execution_price,
                        size:      order.size,
                        fee,
                        timestamp: fill_ts,
                        pnl:       None,
                    });
                    if history.len() > MAX_TRADE_HISTORY {
                        history.pop_front();
                    }

                    // Build FillEvent
                    let mut fill = FillEvent::new(
                        order.id.clone(),
                        order.side,
                        execution_price,
                        order.size,
                        fee,
                        None,
                        fill_ts,
                    );

                    // Step 3A: populate FillEvent.pnl for paired sell fills
                    match order.side {
                        OrderSide::Buy => {
                            if let Some(level_id) = fill.grid_level_id {
                                buy_fills.insert(level_id, (execution_price, fee));
                            }
                        }
                        OrderSide::Sell => {
                            if let Some(level_id) = fill.grid_level_id {
                                if let Some((buy_price, buy_fee)) = buy_fills.remove(&level_id) {
                                    let pnl = (execution_price - buy_price) * order.size
                                              - buy_fee - fee;
                                    fill.pnl = Some(pnl);
                                    *level_pnl.entry(level_id).or_insert(0.0) += pnl;
                                    debug!("[P&L] Level {}: pnl=${:.4} (buy@{:.4} sell@{:.4})",
                                        level_id, pnl, buy_price, execution_price);
                                }
                            }
                        }
                    }

                    // Step 3B/3C: persist fill to CSV with grid spacing
                    if let Some(logger) = &self.fill_logger {
                        let total_pnl = level_pnl.values().sum::<f64>();
                        if let Err(e) = logger.append(&fill, self.grid_spacing, total_pnl) {
                            warn!("[FillLogger] Failed to write fill {}: {}", fill.order_id, e);
                        }
                    }

                    debug!("[Fill] {:?} {:.4} SOL @ ${:.4} fee=${:.4} pnl={:?} spacing={:?}",
                        order.side, order.size, execution_price, fee, fill.pnl, self.grid_spacing);

                    filled_events.push(fill);
                } else {
                    orders.insert(order_id, order);
                }
            }
        }

        Ok(filled_events)
    }

    fn apply_slippage(&self, price: f64, side: OrderSide) -> f64 {
        match side {
            OrderSide::Buy  => price * (1.0 + self.slippage),
            OrderSide::Sell => price * (1.0 - self.slippage),
        }
    }

    pub async fn get_balances(&self)  -> HashMap<String, f64> {
        self.wallet.read().await.balances.clone()
    }
    pub async fn get_wallet(&self)    -> VirtualWallet {
        self.wallet.read().await.clone()
    }
    pub async fn get_open_orders(&self) -> Vec<Order> {
        self.open_orders.read().await.values().cloned().collect()
    }
    pub async fn open_order_count(&self) -> usize {
        self.open_orders.read().await.len()
    }
    pub async fn get_trade_history(&self, limit: usize) -> Vec<Trade> {
        self.trade_history.read().await.iter().rev().take(limit).cloned().collect()
    }
    pub async fn trade_count(&self) -> usize {
        self.trade_history.read().await.len()
    }

    /// Step 3A: snapshot of realised P&L per grid level.
    pub async fn get_per_level_pnl(&self) -> HashMap<u64, f64> {
        self.level_pnl.read().await.clone()
    }

    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let history = self.trade_history.read().await;
        if history.is_empty() { return PerformanceStats::default(); }

        let mut stats = PerformanceStats::default();
        stats.total_trades = history.len();

        let mut wins       = Vec::new();
        let mut losses     = Vec::new();
        let mut buy_prices = Vec::new();

        for trade in history.iter() {
            stats.total_fees += trade.fee;
            match trade.side {
                OrderSide::Buy  => { buy_prices.push(trade.price); }
                OrderSide::Sell => {
                    if let Some(buy_price) = buy_prices.pop() {
                        let pnl = (trade.price - buy_price) * trade.size - trade.fee;
                        stats.total_pnl += pnl;
                        if pnl > 0.0 {
                            stats.winning_trades += 1;
                            wins.push(pnl);
                            stats.largest_win = stats.largest_win.max(pnl);
                        } else {
                            stats.losing_trades += 1;
                            losses.push(pnl);
                            stats.largest_loss = stats.largest_loss.min(pnl);
                        }
                    }
                }
            }
        }

        let pairs = stats.winning_trades + stats.losing_trades;
        if pairs > 0 { stats.win_rate = (stats.winning_trades as f64 / pairs as f64) * 100.0; }
        if !wins.is_empty()   { stats.avg_win  = wins.iter().sum::<f64>()   / wins.len()   as f64; }
        if !losses.is_empty() { stats.avg_loss = losses.iter().sum::<f64>() / losses.len() as f64; }

        let total_wins:   f64 = wins.iter().sum();
        let total_losses: f64 = losses.iter().sum::<f64>().abs();
        if total_losses > 0.0 { stats.profit_factor = total_wins / total_losses; }

        stats
    }

    pub async fn display_status(&self, current_price: f64) {
        let wallet      = self.wallet.read().await;
        let open_orders = self.open_orders.read().await;
        let trade_count = self.trade_history.read().await.len();

        println!("\n+---------------------------------------+");
        println!("| PAPER TRADING STATUS                  |");
        println!("+---------------------------------------+");
        println!("\nWallet:");
        println!("  USDC : ${:.2}", wallet.get_balance("USDC"));
        println!("  SOL  : {:.4} (${:.2})",
            wallet.get_balance("SOL"),
            wallet.get_balance("SOL") * current_price);
        println!("  -------------------------");
        println!("  Total: ${:.2}", wallet.total_value_usdc(current_price));
        println!("  P&L  : ${:.2}", wallet.pnl_usdc(current_price));
        println!("  ROI  : {:.2}%", wallet.roi(current_price));
        drop(wallet);

        let stats = self.get_performance_stats().await;
        println!("\nPerformance:");
        println!("  Total Trades  : {} ({} pairs)",
            trade_count, stats.winning_trades + stats.losing_trades);
        println!("  Win Rate      : {:.2}%", stats.win_rate);
        println!("  Total P&L     : ${:.2}", stats.total_pnl);
        println!("  Total Fees    : ${:.2}", stats.total_fees);
        if stats.winning_trades + stats.losing_trades > 0 {
            println!("  Profit Factor : {:.2}", stats.profit_factor);
        }

        // Step 3A: per-level P&L table
        let level_snapshot = self.level_pnl.read().await;
        if !level_snapshot.is_empty() {
            println!("\nRealised P&L by Grid Level:");
            println!("  {:<8} {:>12}  {}", "Level", "P&L (USDC)", "Status");
            println!("  {}", "-".repeat(32));
            let mut levels: Vec<(&u64, &f64)> = level_snapshot.iter().collect();
            levels.sort_by_key(|(k, _)| *k);
            for (level, pnl) in levels {
                let status = if *pnl >= 0.0 { "WIN" } else { "LOSS" };
                println!("  L{:<7} {:>+12.4}  {}", level, pnl, status);
            }
        }
        drop(level_snapshot);

        // Step 3B/3C: show log file path + spacing
        if let Some(logger) = &self.fill_logger {
            print!("\nFill Log   : {}", logger.path().display());
            if let Some(s) = self.grid_spacing {
                println!(" (spacing ${:.4})", s);
            } else {
                println!();
            }
        }

        println!("\nOpen Orders: {}", open_orders.len());
        println!("SOL Price  : ${:.4}", current_price);
    }

    pub async fn get_engine_stats_inner(&self, sol_price: f64) -> super::EngineStats {
        let wallet = self.wallet.read().await;
        let stats  = self.get_performance_stats().await;
        super::EngineStats {
            total_value_usdc: wallet.total_value_usdc(sol_price),
            pnl_usdc:         wallet.pnl_usdc(sol_price),
            roi_percent:      wallet.roi(sol_price),
            win_rate:         stats.win_rate,
            total_fees:       stats.total_fees,
        }
    }
}

// =============================================================================
// UNIFIED TRADING ENGINE TRAIT IMPLEMENTATION (V3.5)
// =============================================================================

#[async_trait]
impl TradingEngine for PaperTradingEngine {
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String> {
        let base_id = self.place_limit_order(side, price, size).await?;
        let level = match grid_level_id {
            None        => return Ok(base_id),
            Some(level) => level,
        };
        let tagged = format!("{}-L{}", base_id, level);
        {
            let mut orders = self.open_orders.write().await;
            if let Some(order) = orders.get_mut(&base_id) {
                order.id = tagged.clone();
            }
        }
        Ok(tagged)
    }

    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        self.cancel_order(order_id).await
    }

    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        self.cancel_all_orders().await
    }

    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        self.process_price_update(current_price).await
    }

    async fn open_order_count(&self) -> usize {
        self.open_order_count().await
    }

    async fn is_trading_allowed(&self) -> bool { true }

    async fn get_engine_stats(&self, current_price: f64) -> super::EngineStats {
        self.get_engine_stats_inner(current_price).await
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_creation() {
        let wallet = VirtualWallet::new(10000.0, 10.0);
        assert_eq!(wallet.get_balance("USDC"), 10000.0);
        assert_eq!(wallet.get_balance("SOL"),  10.0);
    }

    #[tokio::test]
    async fn test_place_order() {
        let engine = PaperTradingEngine::new(10000.0, 0.0);
        assert!(engine.place_limit_order(OrderSide::Buy, 100.0, 10.0).await.is_ok());
    }

    #[tokio::test]
    async fn test_trading_engine_trait_paper() {
        let engine = PaperTradingEngine::new(10000.0, 1.0);
        assert!(engine.is_trading_allowed().await);
        assert_eq!(engine.open_order_count().await, 0);

        let id = engine
            .place_limit_order_with_level(OrderSide::Buy, 80.0, 0.1, Some(3))
            .await.unwrap();
        assert!(id.ends_with("-L3"));
        assert_eq!(engine.open_order_count().await, 1);
        assert!(engine.cancel_order(&id).await.is_ok());
        assert_eq!(engine.open_order_count().await, 0);
    }

    // -- FillEvent tests (Stage 3 / Step 2) -----------------------------------

    #[tokio::test]
    async fn test_fill_event_emitted_with_correct_side_and_level() {
        let engine = PaperTradingEngine::new(10_000.0, 1.0);
        let id = engine
            .place_limit_order_with_level(OrderSide::Sell, 100.0, 0.1, Some(5))
            .await.unwrap();

        let fills = engine.process_price_update(101.0).await.unwrap();
        assert_eq!(fills.len(), 1);
        let fill = &fills[0];
        assert_eq!(fill.side,          OrderSide::Sell);
        assert_eq!(fill.grid_level_id, Some(5));
        assert!(fill.price > 0.0);
        assert!(fill.fee   > 0.0);
        assert_eq!(fill.order_id, id);
    }

    #[tokio::test]
    async fn test_fill_event_buy_side() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 0.1, Some(2)).await.unwrap();
        let fills = engine.process_price_update(99.0).await.unwrap();
        assert_eq!(fills.len(), 1);
        assert_eq!(fills[0].side,          OrderSide::Buy);
        assert_eq!(fills[0].grid_level_id, Some(2));
    }

    #[tokio::test]
    async fn test_no_fill_when_price_not_crossed() {
        let engine = PaperTradingEngine::new(10_000.0, 1.0);
        engine.place_limit_order_with_level(OrderSide::Sell, 110.0, 0.1, Some(7)).await.unwrap();
        let fills = engine.process_price_update(105.0).await.unwrap();
        assert_eq!(fills.len(), 0);
    }

    // -- Step 3A: P&L tests ---------------------------------------------------

    #[tokio::test]
    async fn test_pnl_computed_on_paired_sell() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0)
            .with_slippage(0.0)
            .with_fees(0.0001, 0.0001);

        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 1.0, Some(1)).await.unwrap();
        let buy_fills = engine.process_price_update(99.0).await.unwrap();
        assert_eq!(buy_fills.len(), 1);
        assert!(buy_fills[0].pnl.is_none(), "Buy fills must have pnl = None");

        engine.place_limit_order_with_level(OrderSide::Sell, 110.0, 1.0, Some(1)).await.unwrap();
        let sell_fills = engine.process_price_update(111.0).await.unwrap();
        assert_eq!(sell_fills.len(), 1);

        let pnl = sell_fills[0].pnl.unwrap();
        assert!(pnl > 0.0,  "pnl should be positive: sell@110 > buy@100, got {}", pnl);
        assert!(pnl < 10.1, "pnl should be near $10 minus fees, got {}", pnl);

        let level_map = engine.get_per_level_pnl().await;
        assert!(level_map.contains_key(&1));
        assert!((level_map[&1] - pnl).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_pnl_none_for_sell_without_paired_buy() {
        let engine = PaperTradingEngine::new(10_000.0, 5.0);
        engine.place_limit_order_with_level(OrderSide::Sell, 100.0, 0.1, Some(4)).await.unwrap();
        let fills = engine.process_price_update(101.0).await.unwrap();
        assert_eq!(fills[0].pnl, None);
        assert!(engine.get_per_level_pnl().await.is_empty());
    }

    #[tokio::test]
    async fn test_pnl_always_none_for_buy_fill() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 0.5, Some(9)).await.unwrap();
        let fills = engine.process_price_update(99.0).await.unwrap();
        assert_eq!(fills[0].pnl, None);
    }

    // -- Step 3B: fill logging tests ------------------------------------------

    #[tokio::test]
    async fn test_fill_logging_writes_to_csv() {
        let dir = std::env::temp_dir().join("gridzbotz_paper_trader_log_test");
        let _ = std::fs::remove_dir_all(&dir);

        let engine = PaperTradingEngine::new(10_000.0, 0.0)
            .with_fill_logging(&dir);

        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 0.5, Some(1)).await.unwrap();
        engine.process_price_update(99.0).await.unwrap();

        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1, "Exactly one CSV file should exist");

        let path    = entries[0].as_ref().unwrap().path();
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("timestamp,order_id,side"), "CSV header missing");
        assert!(content.contains("Buy"), "Buy fill must appear in CSV");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_fill_logging_disabled_by_default() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 0.5, Some(1)).await.unwrap();
        let fills = engine.process_price_update(99.0).await.unwrap();
        assert_eq!(fills.len(), 1, "Fills still emitted even without logger");
    }

    // -- Step 3C: grid spacing in CSV -----------------------------------------

    #[tokio::test]
    async fn test_fill_logging_records_spacing() {
        let dir = std::env::temp_dir().join("gridzbotz_paper_trader_spacing_test");
        let _ = std::fs::remove_dir_all(&dir);

        // spacing = (120 - 60) / (10 - 1) = $6.666...
        let spacing = (120.0_f64 - 60.0) / 9.0;
        let engine = PaperTradingEngine::new(10_000.0, 0.0)
            .with_fill_logging(&dir)
            .with_grid_spacing(spacing);

        engine.place_limit_order_with_level(OrderSide::Buy, 100.0, 0.5, Some(3)).await.unwrap();
        engine.process_price_update(99.0).await.unwrap();

        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        let path    = entries[0].as_ref().unwrap().path();
        let content = std::fs::read_to_string(&path).unwrap();

        // The spacing value (6.666...) must appear in the data row
        assert!(
            content.contains("6.666"),
            "Spacing value must appear in CSV row, got:\n{}",
            content
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}
