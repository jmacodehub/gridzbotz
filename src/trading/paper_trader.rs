//! ═══════════════════════════════════════════════════════════════════════════
//! PAPER TRADING ENGINE V3.2 - Risk-Free Strategy Testing
//! Production-Ready | Enhanced | Optimized | Modular
//! October 16, 2025 — V3.2 February 2026 (fill accumulator + drain_fills)
//! ═══════════════════════════════════════════════════════════════════════════
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
//! ✅ V3.2: drain_fills() — FillEvent accumulator for strategy fan-out

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use async_trait::async_trait;
use log::{info, debug, warn};

use super::{TradingEngine, TradingResult, FillEvent};


// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

const DEFAULT_MAKER_FEE: f64 = 0.0002;
const DEFAULT_TAKER_FEE: f64 = 0.0004;
const DEFAULT_SLIPPAGE: f64 = 0.0005;
const MAX_TRADE_HISTORY: usize = 10000;

// ═══════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide { Buy, Sell }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus { Open, PartiallyFilled, Filled, Cancelled }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType { Limit, Market }

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
            id, side, order_type,
            status: OrderStatus::Open,
            price, size,
            filled_size: 0.0,
            created_at: chrono::Utc::now().timestamp(),
            filled_at: None,
            fee_paid: 0.0,
        }
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualWallet {
    pub balances: HashMap<String, f64>,
    pub initial_balance_usdc: f64,
    pub initial_balance_sol: f64,
}

impl VirtualWallet {
    /// Create a new wallet and log its initial balances at INFO level.
    /// Use this when starting a new paper trading session.
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        let mut balances = HashMap::new();
        balances.insert("USDC".to_string(), initial_usdc);
        balances.insert("SOL".to_string(), initial_sol);
        info!("[WALLET] Initialized: ${:.2} USDC, {:.4} SOL",
              initial_usdc, initial_sol);
        Self { balances, initial_balance_usdc: initial_usdc, initial_balance_sol: initial_sol }
    }

    /// Create a wallet snapshot without emitting a log line.
    ///
    /// Use this when reconstructing a view of current balances from an
    /// already-running engine (e.g. `RealTradingEngine::get_wallet`).
    /// Calling `new()` in that path would log "[WALLET] Initialized" on
    /// every price cycle, flooding the output.
    pub fn new_silent(initial_usdc: f64, initial_sol: f64) -> Self {
        let mut balances = HashMap::new();
        balances.insert("USDC".to_string(), initial_usdc);
        balances.insert("SOL".to_string(), initial_sol);
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
        let current_value = self.total_value_usdc(sol_price);
        let initial_value = self.initial_balance_usdc + (self.initial_balance_sol * sol_price);
        if initial_value == 0.0 { return 0.0; }
        ((current_value - initial_value) / initial_value) * 100.0
    }

    pub fn pnl_usdc(&self, sol_price: f64) -> f64 {
        self.total_value_usdc(sol_price)
            - (self.initial_balance_usdc + self.initial_balance_sol * sol_price)
    }
}

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

#[derive(Clone)]
pub struct PaperTradingEngine {
    wallet: Arc<RwLock<VirtualWallet>>,
    open_orders: Arc<RwLock<HashMap<String, Order>>>,
    trade_history: Arc<RwLock<VecDeque<Trade>>>,
    maker_fee: f64,
    taker_fee: f64,
    slippage: f64,
    next_order_id: Arc<RwLock<u64>>,
    // V3.2: Fill accumulator
    pending_fills: Arc<RwLock<Vec<FillEvent>>>,
}

impl PaperTradingEngine {
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        info!("[PAPER] Initializing Paper Trading Engine V3.2");
        Self {
            wallet: Arc::new(RwLock::new(VirtualWallet::new(initial_usdc, initial_sol))),
            open_orders: Arc::new(RwLock::new(HashMap::new())),
            trade_history: Arc::new(RwLock::new(VecDeque::new())),
            maker_fee: DEFAULT_MAKER_FEE,
            taker_fee: DEFAULT_TAKER_FEE,
            slippage: DEFAULT_SLIPPAGE,
            next_order_id: Arc::new(RwLock::new(1)),
            pending_fills: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_fees(mut self, maker_fee: f64, taker_fee: f64) -> Self {
        self.maker_fee = maker_fee;
        self.taker_fee = taker_fee;
        info!("[FEES] Custom fees: Maker {:.4}%, Taker {:.4}%",
              maker_fee * 100.0, taker_fee * 100.0);
        self
    }

    pub fn with_slippage(mut self, slippage: f64) -> Self {
        self.slippage = slippage;
        info!("[SLIP] Custom slippage: {:.4}%", slippage * 100.0);
        self
    }

    pub async fn place_limit_order(
        &self, side: OrderSide, price: f64, size: f64,
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
        self.open_orders.write().await.insert(order_id.clone(), order);
        debug!("[ORDER] {:?} limit order placed: {:.4} SOL @ ${:.4} (ID: {})",
            side, size, price, order_id);
        Ok(order_id)
    }

    /// Cancel an order by ID (strips optional -L<N> level suffix).
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let base_id = order_id
            .rsplit_once("-L")
            .filter(|(_, suffix)| !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()))
            .map(|(base, _)| base)
            .unwrap_or(order_id);
        let mut orders = self.open_orders.write().await;
        if let Some(mut order) = orders.remove(base_id) {
            order.status = OrderStatus::Cancelled;
            debug!("[CANCEL] Cancelled order: {}", base_id);
            Ok(())
        } else {
            bail!("Order not found: {}", order_id);
        }
    }

    pub async fn cancel_all_orders(&self) -> Result<usize> {
        let mut orders = self.open_orders.write().await;
        let count = orders.len();
        orders.clear();
        if count > 0 { info!("[CANCEL] Cancelled {} orders", count); }
        Ok(count)
    }

    /// Process a price tick, fill matching orders, and accumulate FillEvents.
    ///
    /// After calling this, drain fills and fan-out to StrategyManager:
    /// ```ignore
    /// for fill in engine.drain_fills().await {
    ///     strategy_manager.notify_fill(&fill);
    /// }
    /// ```
    pub async fn process_price_update(&self, current_price: f64) -> Result<Vec<String>> {
        let mut filled_orders = Vec::new();
        let mut orders = self.open_orders.write().await;
        let mut wallet = self.wallet.write().await;
        let mut history = self.trade_history.write().await;
        let mut fills = self.pending_fills.write().await;

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

                    order.status = OrderStatus::Filled;
                    order.filled_size = order.size;
                    order.filled_at = Some(chrono::Utc::now().timestamp());
                    order.fee_paid = fee;

                    let ts = order.filled_at.unwrap();

                    history.push_back(Trade {
                        order_id: order_id.clone(),
                        side: order.side,
                        price: execution_price,
                        size: order.size,
                        fee,
                        timestamp: ts,
                        pnl: None,
                    });
                    if history.len() > MAX_TRADE_HISTORY {
                        history.pop_front();
                    }

                    // V3.2: accumulate FillEvent for strategy fan-out
                    fills.push(FillEvent::new(
                        order_id.clone(),
                        order.side,
                        execution_price,
                        order.size,
                        fee,
                        None,
                        ts,
                    ));

                    filled_orders.push(order_id.clone());
                    debug!("[FILL] {:?} order filled: {:.4} SOL @ ${:.4} (fee: ${:.4})",
                        order.side, order.size, execution_price, fee);
                } else {
                    orders.insert(order_id, order);
                }
            }
        }
        Ok(filled_orders)
    }

    /// Drain and return all FillEvents accumulated since the last call.
    ///
    /// Idempotent: a second call immediately after returns an empty vec.
    /// Typical orchestrator loop:
    /// ```ignore
    /// engine.process_price_update(price).await?;
    /// for fill in engine.drain_fills().await {
    ///     strategy_mgr.notify_fill(&fill);
    /// }
    /// ```
    pub async fn drain_fills(&self) -> Vec<FillEvent> {
        std::mem::take(&mut *self.pending_fills.write().await)
    }

    fn apply_slippage(&self, price: f64, side: OrderSide) -> f64 {
        match side {
            OrderSide::Buy  => price * (1.0 + self.slippage),
            OrderSide::Sell => price * (1.0 - self.slippage),
        }
    }

    pub async fn get_balances(&self) -> HashMap<String, f64> {
        self.wallet.read().await.balances.clone()
    }

    pub async fn get_wallet(&self) -> VirtualWallet {
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

    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let history = self.trade_history.read().await;
        if history.is_empty() { return PerformanceStats::default(); }

        let mut stats = PerformanceStats::default();
        stats.total_trades = history.len();
        let mut wins = Vec::new();
        let mut losses = Vec::new();
        let mut buy_prices = Vec::new();

        for trade in history.iter() {
            stats.total_fees += trade.fee;
            match trade.side {
                OrderSide::Buy => buy_prices.push(trade.price),
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
        if pairs > 0 {
            stats.win_rate = (stats.winning_trades as f64 / pairs as f64) * 100.0;
        }
        if !wins.is_empty() {
            stats.avg_win = wins.iter().sum::<f64>() / wins.len() as f64;
        }
        if !losses.is_empty() {
            stats.avg_loss = losses.iter().sum::<f64>() / losses.len() as f64;
        }
        let tw: f64 = wins.iter().sum();
        let tl: f64 = losses.iter().sum::<f64>().abs();
        if tl > 0.0 { stats.profit_factor = tw / tl; }
        stats
    }

    pub async fn display_status(&self, current_price: f64) {
        let wallet = self.wallet.read().await;
        let open_orders = self.open_orders.read().await;
        let trade_count = self.trade_history.read().await.len();

        println!("\n+=======================================+");
        println!("| [PAPER] PAPER TRADING STATUS          |");
        println!("+=======================================+");
        println!("\n[WALLET]");
        println!("  USDC: ${:.2}", wallet.get_balance("USDC"));
        println!("  SOL:  {:.4} SOL (${:.2})",
                 wallet.get_balance("SOL"),
                 wallet.get_balance("SOL") * current_price);
        println!("  Total Value: ${:.2}", wallet.total_value_usdc(current_price));
        println!("  P&L: ${:.2}", wallet.pnl_usdc(current_price));
        println!("  ROI: {:.2}%", wallet.roi(current_price));
        drop(wallet);

        let stats = self.get_performance_stats().await;
        println!("\n[PERFORMANCE]");
        println!("  Total Trades: {} ({} pairs)",
                 trade_count, stats.winning_trades + stats.losing_trades);
        println!("  Win Rate: {:.2}%", stats.win_rate);
        println!("  Total P&L: ${:.2}", stats.total_pnl);
        println!("  Total Fees: ${:.2}", stats.total_fees);
        if stats.winning_trades + stats.losing_trades > 0 {
            println!("  Profit Factor: {:.2}", stats.profit_factor);
        }
        println!("\n[ORDERS] Open: {}", open_orders.len());
        println!("[PRICE]  Current SOL: ${:.4}", current_price);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADING ENGINE TRAIT IMPLEMENTATION (V3.1 / V5.2)
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl TradingEngine for PaperTradingEngine {
    async fn place_limit_order_with_level(
        &self, side: OrderSide, price: f64, size: f64, grid_level_id: Option<u64>,
    ) -> TradingResult<String> {
        let order_id = self.place_limit_order(side, price, size).await?;
        Ok(match grid_level_id {
            Some(level) => format!("{}-L{}", order_id, level),
            None => order_id,
        })
    }

    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        self.cancel_order(order_id).await
    }

    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        self.cancel_all_orders().await
    }

    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        let _filled_order_ids = self.process_price_update(current_price).await?;
        Ok(self.drain_fills().await)
    }

    async fn open_order_count(&self) -> usize {
        self.open_order_count().await
    }

    async fn is_trading_allowed(&self) -> bool { true }

    // ── V5.2.2: Add these two methods (PR #37) ────────────────────────────
    async fn get_wallet(&self) -> VirtualWallet {
        self.get_wallet().await
    }

    async fn get_performance_stats(&self) -> PerformanceStats {

        self.get_performance_stats().await
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
    async fn test_wallet_new_silent_no_log() {
        // new_silent must produce the same balances as new() without panicking.
        let w = VirtualWallet::new_silent(500.0, 2.5);
        assert_eq!(w.get_balance("USDC"), 500.0);
        assert_eq!(w.get_balance("SOL"), 2.5);
        assert_eq!(w.initial_balance_usdc, 500.0);
        assert_eq!(w.initial_balance_sol, 2.5);
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
        assert!(order_id.ends_with("-L3"), "Expected level tag: {}", order_id);
        assert_eq!(engine.open_order_count().await, 1);
        assert!(engine.cancel_order(&order_id).await.is_ok());
        assert_eq!(engine.open_order_count().await, 0);
    }

    #[tokio::test]
    async fn test_drain_fills_returns_fill_event() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine.place_limit_order(OrderSide::Buy, 100.0, 1.0).await.unwrap();
        engine.process_price_update(98.0).await.unwrap();
        let fills = engine.drain_fills().await;
        assert_eq!(fills.len(), 1, "Expected 1 filled order");
        let fill = &fills[0];
        assert_eq!(fill.side, OrderSide::Buy);
        assert!(fill.fill_price > 0.0);
        assert!(fill.fill_size > 0.0);
        assert!(fill.fee_usdc > 0.0);
        assert!(fill.pnl.is_none());
        assert!(engine.drain_fills().await.is_empty());
    }

    #[tokio::test]
    async fn test_no_fills_when_price_does_not_match() {
        let engine = PaperTradingEngine::new(10_000.0, 0.0);
        engine.place_limit_order(OrderSide::Buy, 100.0, 1.0).await.unwrap();
        engine.process_price_update(105.0).await.unwrap();
        assert!(engine.drain_fills().await.is_empty());
    }
}
