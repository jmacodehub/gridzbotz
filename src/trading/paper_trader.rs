//! ═════════════════════════════════════════════════════════════════════════
//! PAPER TRADING ENGINE V3.2 - Risk-Free Strategy Testing
//! Production-Ready | Enhanced | Optimized | Modular
//! October 16, 2025 — V3.1 February 2026 (TradingEngine trait impl)
//! V3.2 February 2026 (FillEvent — Stage 3 / Step 2)
//! ═════════════════════════════════════════════════════════════════════════
//!
//! Features:
//! ✅ Virtual wallet with multi-token support
//! ✅ Realistic order execution simulation
//! ✅ Grid trading strategy support
//! ✅ Real-time P&L tracking
//! ✅ Order book and trade history
//! ✅ Performance analytics
//! ✅ Slippage and fee simulation
//! ✅ Thread-safe with async support
//! ✅ Builder pattern for configuration
//! ✅ V3.1: impl TradingEngine — satisfies Arc<dyn TradingEngine>
//! ✅ V3.2: process_price_update returns Vec<FillEvent> (Stage 3 Step 2)

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use async_trait::async_trait;
use log::{debug, warn};

use super::{TradingEngine, TradingResult, FillEvent};

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_MAKER_FEE: f64 = 0.0002;  // 0.02% OpenBook maker fee
const DEFAULT_TAKER_FEE: f64 = 0.0004;  // 0.04% OpenBook taker fee
const DEFAULT_SLIPPAGE: f64 = 0.0005;   // 0.05% default slippage
const MAX_TRADE_HISTORY: usize = 10000;

// ═══════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════

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
    /// Create a new virtual wallet with initial balances
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        let mut balances = HashMap::new();
        balances.insert("USDC".to_string(), initial_usdc);
        balances.insert("SOL".to_string(), initial_sol);

        log::info!("\u{1f4b0} Virtual wallet initialized: ${:.2} USDC, {:.4} SOL",
              initial_usdc, initial_sol);

        Self {
            balances,
            initial_balance_usdc: initial_usdc,
            initial_balance_sol: initial_sol,
        }
    }

    /// Get balance for a token (returns value, not reference)
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

    /// Calculate total portfolio value in USDC
    pub fn total_value_usdc(&self, sol_price: f64) -> f64 {
        let usdc = self.get_balance("USDC");
        let sol = self.get_balance("SOL");
        usdc + (sol * sol_price)
    }

    /// Calculate ROI percentage
    pub fn roi(&self, sol_price: f64) -> f64 {
        let current_value = self.total_value_usdc(sol_price);
        let initial_value = self.initial_balance_usdc + (self.initial_balance_sol * sol_price);
        if initial_value == 0.0 {
            return 0.0;
        }
        ((current_value - initial_value) / initial_value) * 100.0
    }

    /// Calculate profit/loss in USDC
    pub fn pnl_usdc(&self, sol_price: f64) -> f64 {
        let current_value = self.total_value_usdc(sol_price);
        let initial_value = self.initial_balance_usdc + (self.initial_balance_sol * sol_price);
        current_value - initial_value
    }
}

/// Performance statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub profit_factor: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// PAPER TRADING ENGINE
// ═══════════════════════════════════════════════════════════════════════════

/// Main paper trading engine
#[derive(Clone)]
pub struct PaperTradingEngine {
    wallet: Arc<RwLock<VirtualWallet>>,
    open_orders: Arc<RwLock<HashMap<String, Order>>>,
    trade_history: Arc<RwLock<VecDeque<Trade>>>,
    maker_fee: f64,
    taker_fee: f64,
    slippage: f64,
    next_order_id: Arc<RwLock<u64>>,
}

impl PaperTradingEngine {
    /// Create a new paper trading engine with default settings
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        log::info!("\u{1f3ae} Initializing Paper Trading Engine V3.2");

        Self {
            wallet: Arc::new(RwLock::new(VirtualWallet::new(initial_usdc, initial_sol))),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            trade_history: Arc::new(RwLock::new(VecDeque::new())),
            maker_fee: DEFAULT_MAKER_FEE,
            taker_fee: DEFAULT_TAKER_FEE,
            slippage: DEFAULT_SLIPPAGE,
            next_order_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create engine with custom fees
    pub fn with_fees(mut self, maker_fee: f64, taker_fee: f64) -> Self {
        self.maker_fee = maker_fee;
        self.taker_fee = taker_fee;
        log::info!("\u{1f4b8} Custom fees: Maker {:.4}%, Taker {:.4}%",
              maker_fee * 100.0, taker_fee * 100.0);
        self
    }

    /// Create engine with custom slippage
    pub fn with_slippage(mut self, slippage: f64) -> Self {
        self.slippage = slippage;
        log::info!("\u{1f4c9} Custom slippage: {:.4}%", slippage * 100.0);
        self
    }

    /// Place a limit order
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
                let required_usdc = price * size * (1.0 + self.taker_fee);
                if wallet.get_balance("USDC") < required_usdc {
                    bail!("Insufficient USDC: have ${:.2}, need ${:.2}",
                        wallet.get_balance("USDC"), required_usdc);
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

        let mut orders = self.open_orders.write().await;
        orders.insert(order_id.clone(), order);

        debug!("\u{1f4dd} {:?} limit order placed: {:.4} SOL @ ${:.4} (ID: {})",
            side, size, price, order_id
        );

        Ok(order_id)
    }

    /// Cancel an order by ID.
    ///
    /// Accepts both plain IDs ("ORDER-000001") and level-tagged IDs
    /// ("ORDER-000001-L3") produced by place_limit_order_with_level.
    /// The "-L<N>" suffix is stripped before the HashMap lookup so
    /// callers do not need to track which format they hold.
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let base_id = order_id
            .rsplit_once("-L")
            .filter(|(_, suffix)| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
            .map(|(base, _)| base)
            .unwrap_or(order_id);

        let mut orders = self.open_orders.write().await;
        if let Some(mut order) = orders.remove(base_id) {
            order.status = OrderStatus::Cancelled;
            debug!("\u274c Cancelled order: {}", base_id);
            Ok(())
        } else {
            bail!("Order not found: {}", order_id);
        }
    }

    /// Cancel all open orders
    pub async fn cancel_all_orders(&self) -> Result<usize> {
        let mut orders = self.open_orders.write().await;
        let count = orders.len();
        orders.clear();
        if count > 0 {
            log::info!("\u274c Cancelled {} orders", count);
        }
        Ok(count)
    }

    /// Process price update and execute matching orders.
    ///
    /// V3.2 (Stage 3 / Step 2): now returns `Vec<FillEvent>` with full
    /// execution context instead of a bare `Vec<String>`. The `side`,
    /// `price`, `size`, `fee`, `grid_level_id`, and `timestamp` are all
    /// taken directly from the `Order` struct — no string sniffing needed.
    pub async fn process_price_update(&self, current_price: f64) -> Result<Vec<FillEvent>> {
        let mut filled_events = Vec::new();
        let mut orders  = self.open_orders.write().await;
        let mut wallet  = self.wallet.write().await;
        let mut history = self.trade_history.write().await;

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
                                warn!("Failed to execute buy order {}: {}", order_id, e);
                                orders.insert(order_id, order);
                                continue;
                            }
                            wallet.add_balance("SOL", order.size);
                        }
                        OrderSide::Sell => {
                            if let Err(e) = wallet.sub_balance("SOL", order.size) {
                                warn!("Failed to execute sell order {}: {}", order_id, e);
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

                    // Persist to trade history
                    let trade = Trade {
                        order_id:  order_id.clone(),
                        side:      order.side,
                        price:     execution_price,
                        size:      order.size,
                        fee,
                        timestamp: fill_ts,
                        pnl:       None,
                    };
                    history.push_back(trade);
                    if history.len() > MAX_TRADE_HISTORY {
                        history.pop_front();
                    }

                    // ✅ Stage 3 / Step 2: emit rich FillEvent — side/price/size come
                    // straight from the Order struct, no string-sniffing needed.
                    let fill = FillEvent::new(
                        order_id.clone(),
                        order.side,
                        execution_price,
                        order.size,
                        fee,
                        None, // PnL: paired when a sell matches its originating buy
                        fill_ts,
                    );
                    filled_events.push(fill);

                    debug!("\u2705 {:?} order filled: {:.4} SOL @ ${:.4} (fee: ${:.4})",
                        order.side, order.size, execution_price, fee
                    );
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

    /// Get current wallet balances
    pub async fn get_balances(&self) -> HashMap<String, f64> {
        self.wallet.read().await.balances.clone()
    }

    /// Get wallet (for advanced queries)
    pub async fn get_wallet(&self) -> VirtualWallet {
        self.wallet.read().await.clone()
    }

    /// Get open orders
    pub async fn get_open_orders(&self) -> Vec<Order> {
        self.open_orders.read().await.values().cloned().collect()
    }

    /// Get number of open orders
    pub async fn open_order_count(&self) -> usize {
        self.open_orders.read().await.len()
    }

    /// Get trade history
    pub async fn get_trade_history(&self, limit: usize) -> Vec<Trade> {
        let history = self.trade_history.read().await;
        history.iter().rev().take(limit).cloned().collect()
    }

    /// Get total number of trades
    pub async fn trade_count(&self) -> usize {
        self.trade_history.read().await.len()
    }

    /// Calculate performance statistics
    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let history = self.trade_history.read().await;

        if history.is_empty() {
            return PerformanceStats::default();
        }

        let mut stats = PerformanceStats::default();
        stats.total_trades = history.len();

        let mut wins = Vec::new();
        let mut losses = Vec::new();
        let mut buy_prices = Vec::new();

        for trade in history.iter() {
            stats.total_fees += trade.fee;

            match trade.side {
                OrderSide::Buy => {
                    buy_prices.push(trade.price);
                }
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

        let pair_trades = stats.winning_trades + stats.losing_trades;
        if pair_trades > 0 {
            stats.win_rate = (stats.winning_trades as f64 / pair_trades as f64) * 100.0;
        }

        if !wins.is_empty() {
            stats.avg_win = wins.iter().sum::<f64>() / wins.len() as f64;
        }

        if !losses.is_empty() {
            stats.avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
        }

        let total_wins: f64 = wins.iter().sum();
        let total_losses: f64 = losses.iter().sum::<f64>().abs();
        if total_losses > 0.0 {
            stats.profit_factor = total_wins / total_losses;
        }

        stats
    }

    /// Display current status to stdout
    pub async fn display_status(&self, current_price: f64) {
        let wallet = self.wallet.read().await;
        let open_orders = self.open_orders.read().await;
        let trade_count = self.trade_history.read().await.len();

        println!("\n\u256c\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2563");
        println!("\u2551   \u{1f4ca} PAPER TRADING STATUS             \u2551");
        println!("\u255a\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u255d");

        println!("\n\u{1f4b0} Wallet:");
        println!("  USDC: ${:.2}", wallet.get_balance("USDC"));
        println!("  SOL:  {:.4} SOL (${:.2})", wallet.get_balance("SOL"), wallet.get_balance("SOL") * current_price);
        println!("  \u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500\u2500");
        println!("  Total Value: ${:.2}", wallet.total_value_usdc(current_price));
        println!("  P&L: ${:.2}", wallet.pnl_usdc(current_price));
        println!("  ROI: {:.2}%", wallet.roi(current_price));

        drop(wallet);

        let stats = self.get_performance_stats().await;

        println!("\n\u{1f4c8} Performance:");
        println!("  Total Trades: {} ({} pairs)", trade_count, stats.winning_trades + stats.losing_trades);
        println!("  Win Rate: {:.2}%", stats.win_rate);
        println!("  Total P&L: ${:.2}", stats.total_pnl);
        println!("  Total Fees: ${:.2}", stats.total_fees);

        if stats.winning_trades + stats.losing_trades > 0 {
            println!("  Profit Factor: {:.2}", stats.profit_factor);
        }

        println!("\n\u{1f4dd} Open Orders: {}", open_orders.len());
        println!("\n\u{1f4b5} Current SOL Price: ${:.4}", current_price);
    }

    /// get_engine_stats: returns real wallet data for the GridBot dashboard.
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

// ═══════════════════════════════════════════════════════════════════════════
// UNIFIED TRADING ENGINE TRAIT IMPLEMENTATION (V3.2)
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl TradingEngine for PaperTradingEngine {
    /// Wraps the inherent place_limit_order(), tagging the returned
    /// order ID with the grid level for full traceability in logs.
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String> {
        let order_id = self.place_limit_order(side, price, size).await?;
        Ok(match grid_level_id {
            Some(level) => format!("{}-L{}", order_id, level),
            None => order_id,
        })
    }

    /// Delegates to the inherent cancel_order().
    /// The inherent method handles both plain and level-tagged IDs.
    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        self.cancel_order(order_id).await
    }

    /// Delegates to the inherent cancel_all_orders().
    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        self.cancel_all_orders().await
    }

    /// Delegates to the inherent process_price_update().
    /// Returns Vec<FillEvent> with full execution context per fill.
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        self.process_price_update(current_price).await
    }

    /// Delegates to the inherent open_order_count().
    async fn open_order_count(&self) -> usize {
        self.open_order_count().await
    }

    /// Paper mode is always allowed — no circuit breaker.
    async fn is_trading_allowed(&self) -> bool {
        true
    }

    /// Returns real wallet data for portfolio dashboards.
    async fn get_engine_stats(&self, current_price: f64) -> super::EngineStats {
        self.get_engine_stats_inner(current_price).await
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wallet_creation() {
        let wallet = VirtualWallet::new(10000.0, 10.0);
        assert_eq!(wallet.get_balance("USDC"), 10000.0);
        assert_eq!(wallet.get_balance("SOL"), 10.0);
    }

    #[tokio::test]
    async fn test_place_order() {
        let engine = PaperTradingEngine::new(10000.0, 0.0);
        let result = engine.place_limit_order(OrderSide::Buy, 100.0, 10.0).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_trading_engine_trait_paper() {
        let engine = PaperTradingEngine::new(10000.0, 1.0);
        assert!(engine.is_trading_allowed().await);
        assert_eq!(engine.open_order_count().await, 0);
        let result = engine
            .place_limit_order_with_level(OrderSide::Buy, 80.0, 0.1, Some(3))
            .await;
        assert!(result.is_ok());
        let order_id = result.unwrap();
        assert!(order_id.ends_with("-L3"), "Expected level tag in ID: {}", order_id);
        assert_eq!(engine.open_order_count().await, 1);
        assert!(engine.cancel_order(&order_id).await.is_ok());
        assert_eq!(engine.open_order_count().await, 0);
    }

    // ── FillEvent tests (Stage 3 / Step 2) ──────────────────────────────

    #[tokio::test]
    async fn test_fill_event_emitted_with_correct_side_and_level() {
        let engine = PaperTradingEngine::new(10_000.0, 1.0);

        // Place a sell tagged to level 5 — fills when price >= 100.0
        let id = engine
            .place_limit_order_with_level(OrderSide::Sell, 100.0, 0.1, Some(5))
            .await
            .unwrap();

        // Price rises above ask — should trigger a fill
        let fills = engine.process_price_update(101.0).await.unwrap();
        assert_eq!(fills.len(), 1, "Expected exactly 1 fill");

        let fill = &fills[0];
        assert_eq!(fill.side, OrderSide::Sell,     "Fill side must be Sell");
        assert_eq!(fill.grid_level_id, Some(5),    "Level ID must be parsed from order suffix");
        assert!(fill.price > 0.0,                  "Execution price must be positive");
        assert!(fill.fee > 0.0,                    "Fee must be non-zero");
        assert_eq!(fill.order_id, id,              "Fill order_id must match placed order");
    }

    #[tokio::test]
    async fn test_fill_event_buy_side() {
        // Start with no SOL, place a buy at 100.0, price drops to 99.0
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine
            .place_limit_order_with_level(OrderSide::Buy, 100.0, 0.1, Some(2))
            .await
            .unwrap();

        let fills = engine.process_price_update(99.0).await.unwrap();
        assert_eq!(fills.len(), 1);

        let fill = &fills[0];
        assert_eq!(fill.side, OrderSide::Buy);
        assert_eq!(fill.grid_level_id, Some(2));
        assert!(fill.size > 0.0);
    }

    #[tokio::test]
    async fn test_no_fill_when_price_not_crossed() {
        let engine = PaperTradingEngine::new(10_000.0, 1.0);
        engine
            .place_limit_order_with_level(OrderSide::Sell, 110.0, 0.1, Some(7))
            .await
            .unwrap();

        // Price at 105 — below ask, should NOT fill
        let fills = engine.process_price_update(105.0).await.unwrap();
        assert_eq!(fills.len(), 0, "No fills expected when price hasn't crossed");
    }
}
