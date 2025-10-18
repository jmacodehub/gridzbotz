//! ═══════════════════════════════════════════════════════════════════════════
//! PAPER TRADING ENGINE V3.0 - Risk-Free Strategy Testing
//! Production-Ready | Enhanced | Optimized | Modular
//! October 16, 2025
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

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};
use anyhow::{Result, bail};
use log::{info, debug, warn};

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
        
        info!("💰 Virtual wallet initialized: ${:.2} USDC, {:.4} SOL", 
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
    /// 
    /// # Arguments
    /// * `initial_usdc` - Starting USDC balance
    /// * `initial_sol` - Starting SOL balance
    pub fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        info!("🎮 Initializing Paper Trading Engine V3.0");
        
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
        info!("💸 Custom fees: Maker {:.4}%, Taker {:.4}%", 
              maker_fee * 100.0, taker_fee * 100.0);
        self
    }
    
    /// Create engine with custom slippage
    pub fn with_slippage(mut self, slippage: f64) -> Self {
        self.slippage = slippage;
        info!("📉 Custom slippage: {:.4}%", slippage * 100.0);
        self
    }
    
    /// Place a limit order
    pub async fn place_limit_order(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
    ) -> Result<String> {
        // Generate order ID
        let order_id = {
            let mut next_id = self.next_order_id.write().await;
            let id = format!("ORDER-{:06}", *next_id);
            *next_id += 1;
            id
        };
        
        // Check if we have enough balance
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
        orders.insert(order_id.clone(), order.clone());
        
        debug!("📝 {:?} limit order placed: {:.4} SOL @ ${:.4} (ID: {})",
            side, size, price, order_id
        );
        
        Ok(order_id)
    }
    
    /// Cancel an order
    pub async fn cancel_order(&self, order_id: &str) -> Result<()> {
        let mut orders = self.open_orders.write().await;
        
        if let Some(mut order) = orders.remove(order_id) {
            order.status = OrderStatus::Cancelled;
            debug!("❌ Cancelled order: {}", order_id);
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
            info!("❌ Cancelled {} orders", count);
        }
        Ok(count)
    }
    
    /// Process price update and execute matching orders
    pub async fn process_price_update(&self, current_price: f64) -> Result<Vec<String>> {
        let mut filled_orders = Vec::new();
        let mut orders = self.open_orders.write().await;
        let mut wallet = self.wallet.write().await;
        let mut history = self.trade_history.write().await;
        
        let order_ids: Vec<String> = orders.keys().cloned().collect();
        
        for order_id in order_ids {
            if let Some(mut order) = orders.remove(&order_id) {
                let should_fill = match order.side {
                    OrderSide::Buy => current_price <= order.price,
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
                    
                    let trade = Trade {
                        order_id: order_id.clone(),
                        side: order.side,
                        price: execution_price,
                        size: order.size,
                        fee,
                        timestamp: order.filled_at.unwrap(),
                        pnl: None,
                    };
                    
                    history.push_back(trade);
                    if history.len() > MAX_TRADE_HISTORY {
                        history.pop_front();
                    }
                    
                    filled_orders.push(order_id.clone());
                    
                    debug!("✅ {:?} order filled: {:.4} SOL @ ${:.4} (fee: ${:.4})",
                        order.side, order.size, execution_price, fee
                    );
                } else {
                    orders.insert(order_id, order);
                }
            }
        }
        
        Ok(filled_orders)
    }
    
    fn apply_slippage(&self, price: f64, side: OrderSide) -> f64 {
        match side {
            OrderSide::Buy => price * (1.0 + self.slippage),
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
        
        println!("\n╔═══════════════════════════════════════╗");
        println!("║   📊 PAPER TRADING STATUS             ║");
        println!("╚═══════════════════════════════════════╝");
        
        println!("\n💰 Wallet:");
        println!("  USDC: ${:.2}", wallet.get_balance("USDC"));
        println!("  SOL:  {:.4} SOL (${:.2})", wallet.get_balance("SOL"), wallet.get_balance("SOL") * current_price);
        println!("  ─────────────────────────");
        println!("  Total Value: ${:.2}", wallet.total_value_usdc(current_price));
        println!("  P&L: ${:.2}", wallet.pnl_usdc(current_price));
        println!("  ROI: {:.2}%", wallet.roi(current_price));
        
        drop(wallet);
        
        let stats = self.get_performance_stats().await;
        
        println!("\n📈 Performance:");
        println!("  Total Trades: {} ({} pairs)", trade_count, stats.winning_trades + stats.losing_trades);
        println!("  Win Rate: {:.2}%", stats.win_rate);
        println!("  Total P&L: ${:.2}", stats.total_pnl);
        println!("  Total Fees: ${:.2}", stats.total_fees);
        
        if stats.winning_trades + stats.losing_trades > 0 {
            println!("  Profit Factor: {:.2}", stats.profit_factor);
        }
        
        println!("\n📝 Open Orders: {}", open_orders.len());
        println!("\n💵 Current SOL Price: ${:.4}", current_price);
    }
}

// Tests remain the same...
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
}
