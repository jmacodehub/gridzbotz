//! 🔗 DEX Integration Module
//! 
//! Production-grade interfaces for decentralized exchange trading on Solana:
//! - Unified `Trader` trait for paper, Serum/OpenBook, and Jupiter trading
//! - Order placement and cancellation with retry logic
//! - Market state monitoring and caching
//! - Position tracking and real-time P&L calculation
//! - Order lifecycle management with status tracking
//! - Cross-DEX support via trait-based architecture
//! 
//! # Architecture
//! 
//! ```
//! ┌─────────────────────────────────────────────────────┐
//! │              Trader Trait (Unified API)              │
//! └─────────────────┬───────────────────────────────────┘
//!                   │
//!       ┌───────────┼───────────┐
//!       │           │           │
//!  ┌────▼────┐ ┌───▼────┐ ┌───▼──────┐
//!  │  Paper  │ │ Serum  │ │ Jupiter  │
//!  │ Trader  │ │ Client │ │  Client  │
//!  └─────────┘ └────────┘ └──────────┘
//! ```
//! 
//! # Example
//! ```
//! use solana_grid_bot::dex::{SerumClient, OrderSide, Order, OrderType, Trader};
//! use solana_sdk::signature::Keypair;
//! use solana_sdk::pubkey::Pubkey;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let wallet = Keypair::new();
//! let market = Pubkey::new_unique();
//! 
//! // Create Serum client
//! let mut client = SerumClient::new(
//!     "https://api.mainnet-beta.solana.com".to_string(),
//!     wallet,
//!     market,
//! )?;
//! 
//! // Place order using Trader trait
//! let order = Order {
//!     side: OrderSide::Bid,
//!     price: 193.50,
//!     size: 1.0,
//!     order_type: OrderType::Limit,
//!     client_order_id: 1,
//! };
//! 
//! let placed = client.place_order(order).await?;
//! println!("Order placed: {}", placed.order_id);
//! 
//! // Get position
//! let position = client.get_position().await?;
//! position.display(193.50);
//! # Ok(())
//! # }
//! ```

pub mod serum_client;
pub mod order_manager;
pub mod market_state;
pub mod jupiter_client;

// Re-export main types for convenience
pub use serum_client::{SerumClient, ClientStats, MarketInfo};
pub use order_manager::{OrderManager, OrderStats};
pub use market_state::MarketState;
pub use jupiter_client::JupiterClient;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// ═══════════════════════════════════════════════════════════════════════════
// TRADER TRAIT - Unified Trading Interface
// ═══════════════════════════════════════════════════════════════════════════

/// Unified trading interface for all backend implementations
/// 
/// This trait provides a common API for:
/// - Paper trading (simulation for testing)
/// - Direct DEX trading (Serum/OpenBook)
/// - Aggregator trading (Jupiter for best prices)
/// 
/// # Benefits
/// - Seamless backend switching
/// - Easy testing (paper → real)
/// - Strategy-agnostic code
/// - Future-proof architecture
/// 
/// # Example
/// ```
/// # use solana_grid_bot::dex::{Trader, Order, OrderSide, OrderType};
/// # async fn example<T: Trader>(mut trader: T) -> anyhow::Result<()> {
/// // Works with ANY trader implementation!
/// let order = Order {
///     side: OrderSide::Bid,
///     price: 100.0,
///     size: 1.0,
///     order_type: OrderType::Limit,
///     client_order_id: 1,
/// };
/// 
/// let placed = trader.place_order(order).await?;
/// println!("{} placed order: {}", trader.trader_type(), placed.order_id);
/// # Ok(())
/// # }
/// ```
#[async_trait]
pub trait Trader: Send + Sync {
    /// Place an order (limit, market, or post-only)
    /// 
    /// # Arguments
    /// * `order` - Order specification with side, price, size, type
    /// 
    /// # Returns
    /// * `PlacedOrder` with on-chain order ID and timestamp
    /// 
    /// # Errors
    /// * Invalid order parameters (negative price/size)
    /// * Insufficient balance
    /// * RPC/network failures
    async fn place_order(&mut self, order: Order) -> anyhow::Result<PlacedOrder>;
    
    /// Cancel an existing order by ID
    /// 
    /// # Arguments
    /// * `order_id` - On-chain order ID to cancel
    /// 
    /// # Errors
    /// * Order not found
    /// * Already filled/cancelled
    /// * RPC failures
    async fn cancel_order(&mut self, order_id: u128) -> anyhow::Result<()>;
    
    /// Get current token balances
    /// 
    /// # Returns
    /// * Tuple of (base_amount, quote_amount)
    /// 
    /// # Example
    /// ```
    /// # use solana_grid_bot::dex::Trader;
    /// # async fn example<T: Trader>(trader: T) -> anyhow::Result<()> {
    /// let (base, quote) = trader.get_balance().await?;
    /// println!("Base: {:.4}, Quote: ${:.2}", base, quote);
    /// # Ok(())
    /// # }
    /// ```
    async fn get_balance(&self) -> anyhow::Result<(f64, f64)>;
    
    /// Get current position with P&L
    /// 
    /// # Returns
    /// * `Position` with holdings, entry price, and P&L
    async fn get_position(&self) -> anyhow::Result<Position>;
    
    /// Get trader implementation name for logging
    /// 
    /// # Returns
    /// * Human-readable trader type (e.g., "Serum DEX", "Jupiter Aggregator")
    fn trader_type(&self) -> &'static str;
}

// ═══════════════════════════════════════════════════════════════════════════
// ORDER TYPES & ENUMS
// ═══════════════════════════════════════════════════════════════════════════

/// Order side: Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum OrderSide {
    /// Buy order (bid) - acquire base currency with quote
    Bid,
    /// Sell order (ask) - sell base currency for quote
    Ask,
}

impl OrderSide {
    /// Convert to human-readable string
    /// 
    /// # Returns
    /// * "BUY" for Bid, "SELL" for Ask
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Bid => "BUY",
            OrderSide::Ask => "SELL",
        }
    }
    
    /// Get opposite side
    /// 
    /// # Returns
    /// * Ask if Bid, Bid if Ask
    pub fn opposite(&self) -> Self {
        match self {
            OrderSide::Bid => OrderSide::Ask,
            OrderSide::Ask => OrderSide::Bid,
        }
    }
    
    /// Check if this is a buy order
    pub fn is_buy(&self) -> bool {
        matches!(self, OrderSide::Bid)
    }
    
    /// Check if this is a sell order
    pub fn is_sell(&self) -> bool {
        matches!(self, OrderSide::Ask)
    }
}

/// Order type and execution behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Standard limit order - stays on book until filled or cancelled
    Limit,
    /// Immediate or cancel - fills immediately or cancels (market-like)
    ImmediateOrCancel,
    /// Post-only - must be maker, never taker (saves fees)
    PostOnly,
}

impl OrderType {
    /// Check if order is maker-only (no taker fees)
    pub fn is_maker_only(&self) -> bool {
        matches!(self, OrderType::PostOnly)
    }
    
    /// Check if order allows taker execution
    pub fn allows_taker(&self) -> bool {
        !self.is_maker_only()
    }
    
    /// Get human-readable description
    pub fn description(&self) -> &'static str {
        match self {
            OrderType::Limit => "Limit order (maker/taker)",
            OrderType::ImmediateOrCancel => "IOC (immediate execution)",
            OrderType::PostOnly => "Post-only (maker only)",
        }
    }
}

/// Order status lifecycle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    /// Order submitted but not yet confirmed on-chain
    Pending,
    /// Order confirmed and open on orderbook
    Open,
    /// Order partially filled (some size remaining)
    PartiallyFilled,
    /// Order completely filled (no size remaining)
    Filled,
    /// Order cancelled by user
    Cancelled,
    /// Order failed to submit (validation or RPC error)
    Failed,
}

impl OrderStatus {
    /// Check if order is in a terminal state (no further changes)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            OrderStatus::Filled | OrderStatus::Cancelled | OrderStatus::Failed
        )
    }
    
    /// Check if order is active (can still be filled/cancelled)
    pub fn is_active(&self) -> bool {
        matches!(self, OrderStatus::Open | OrderStatus::PartiallyFilled)
    }
    
    /// Check if order is pending confirmation
    pub fn is_pending(&self) -> bool {
        matches!(self, OrderStatus::Pending)
    }
    
    /// Get human-readable description
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderStatus::Pending => "Pending",
            OrderStatus::Open => "Open",
            OrderStatus::PartiallyFilled => "Partially Filled",
            OrderStatus::Filled => "Filled",
            OrderStatus::Cancelled => "Cancelled",
            OrderStatus::Failed => "Failed",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ORDER STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════

/// Order request specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Buy or sell
    pub side: OrderSide,
    /// Limit price in quote currency (e.g., USDC per SOL)
    pub price: f64,
    /// Order size in base currency (e.g., SOL)
    pub size: f64,
    /// Order type (limit, IOC, post-only)
    pub order_type: OrderType,
    /// Client-generated order ID for tracking
    pub client_order_id: u64,
}

impl Order {
    /// Create a new order with automatic client ID
    pub fn new(side: OrderSide, price: f64, size: f64, order_type: OrderType) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        let client_order_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        Self {
            side,
            price,
            size,
            order_type,
            client_order_id,
        }
    }
    
    /// Calculate order value in quote currency
    /// 
    /// # Returns
    /// * Total value (price × size)
    pub fn value(&self) -> f64 {
        self.price * self.size
    }
    
    /// Validate order parameters
    /// 
    /// # Returns
    /// * `Ok(())` if valid
    /// * `Err` if price or size is invalid
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.price <= 0.0 {
            anyhow::bail!("Price must be positive, got: {}", self.price);
        }
        if self.size <= 0.0 {
            anyhow::bail!("Size must be positive, got: {}", self.size);
        }
        Ok(())
    }
    
    /// Display order details to console
    pub fn display(&self) {
        println!("📝 Order Details:");
        println!("   Side:       {}", self.side.as_str());
        println!("   Price:      ${:.4}", self.price);
        println!("   Size:       {:.4}", self.size);
        println!("   Value:      ${:.2}", self.value());
        println!("   Type:       {}", self.order_type.description());
        println!("   Client ID:  {}", self.client_order_id);
    }
}

/// Placed order (confirmed on-chain)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacedOrder {
    /// Original order details
    pub order: Order,
    /// On-chain order ID
    pub order_id: u128,
    /// Market address
    pub market: Pubkey,
    /// Order owner (trader wallet)
    pub owner: Pubkey,
    /// Timestamp when placed (Unix timestamp)
    pub timestamp: i64,
}

impl PlacedOrder {
    /// Check if order is buy
    pub fn is_buy(&self) -> bool {
        self.order.side.is_buy()
    }
    
    /// Check if order is sell
    pub fn is_sell(&self) -> bool {
        self.order.side.is_sell()
    }
    
    /// Get order age in seconds
    pub fn age_seconds(&self) -> i64 {
        chrono::Utc::now().timestamp() - self.timestamp
    }
    
    /// Get order age as human-readable string
    pub fn age_str(&self) -> String {
        let age = self.age_seconds();
        if age < 60 {
            format!("{}s", age)
        } else if age < 3600 {
            format!("{}m {}s", age / 60, age % 60)
        } else {
            format!("{}h {}m", age / 3600, (age % 3600) / 60)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// POSITION TRACKING
// ═══════════════════════════════════════════════════════════════════════════

/// Trading position with P&L tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Amount of base currency held (e.g., SOL)
    pub base_amount: f64,
    /// Amount of quote currency held (e.g., USDC)
    pub quote_amount: f64,
    /// Weighted average entry price
    pub avg_entry_price: f64,
    /// Unrealized profit/loss (mark-to-market)
    pub unrealized_pnl: f64,
    /// Realized profit/loss (from closed positions)
    pub realized_pnl: f64,
}

impl Position {
    /// Create empty position with starting capital
    /// 
    /// # Arguments
    /// * `starting_capital` - Initial quote currency amount
    pub fn new(starting_capital: f64) -> Self {
        Self {
            base_amount: 0.0,
            quote_amount: starting_capital,
            avg_entry_price: 0.0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
        }
    }
    
    /// Calculate current unrealized P&L given current price
    /// 
    /// # Arguments
    /// * `current_price` - Current market price
    /// 
    /// # Returns
    /// * Unrealized profit/loss in quote currency
    pub fn calculate_pnl(&self, current_price: f64) -> f64 {
        if self.base_amount > 0.0 && self.avg_entry_price > 0.0 {
            (current_price - self.avg_entry_price) * self.base_amount
        } else {
            0.0
        }
    }
    
    /// Calculate total portfolio value at current price
    /// 
    /// # Arguments
    /// * `current_price` - Current market price
    /// 
    /// # Returns
    /// * Total value (base × price + quote)
    pub fn total_value(&self, current_price: f64) -> f64 {
        (self.base_amount * current_price) + self.quote_amount
    }
    
    /// Calculate ROI percentage
    /// 
    /// # Arguments
    /// * `current_price` - Current market price
    /// * `initial_capital` - Starting capital amount
    /// 
    /// # Returns
    /// * ROI as percentage
    pub fn roi(&self, current_price: f64, initial_capital: f64) -> f64 {
        if initial_capital > 0.0 {
            ((self.total_value(current_price) - initial_capital) / initial_capital) * 100.0
        } else {
            0.0
        }
    }
    
    /// Check if position is flat (no base currency held)
    pub fn is_flat(&self) -> bool {
        self.base_amount.abs() < 0.0001
    }
    
    /// Check if position is long (positive base)
    pub fn is_long(&self) -> bool {
        self.base_amount > 0.0001
    }
    
    /// Calculate total P&L (realized + unrealized)
    /// 
    /// # Arguments
    /// * `current_price` - Current market price
    /// 
    /// # Returns
    /// * Total P&L
    pub fn total_pnl(&self, current_price: f64) -> f64 {
        self.realized_pnl + self.calculate_pnl(current_price)
    }
    
    /// Display position info to console
    pub fn display(&self, current_price: f64) {
        println!("\n💼 Current Position:");
        println!("   Base:          {:.4}", self.base_amount);
        println!("   Quote:         ${:.2}", self.quote_amount);
        println!("   Entry Price:   ${:.4}", self.avg_entry_price);
        println!("   Current Price: ${:.4}", current_price);
        println!("   Unrealized:    ${:+.2}", self.calculate_pnl(current_price));
        println!("   Realized:      ${:+.2}", self.realized_pnl);
        println!("   Total P&L:     ${:+.2}", self.total_pnl(current_price));
        println!("   Total Value:   ${:.2}", self.total_value(current_price));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate trading fee for an order
/// 
/// # Arguments
/// * `value` - Order value in quote currency
/// * `fee_rate` - Fee rate (e.g., 0.001 for 0.1%)
/// 
/// # Returns
/// * Fee amount in quote currency
pub fn calculate_fee(value: f64, fee_rate: f64) -> f64 {
    value * fee_rate
}

/// Calculate slippage between expected and actual price
/// 
/// # Arguments
/// * `expected_price` - Expected execution price
/// * `actual_price` - Actual execution price
/// 
/// # Returns
/// * Slippage as percentage (always positive)
pub fn calculate_slippage(expected_price: f64, actual_price: f64) -> f64 {
    ((actual_price - expected_price) / expected_price).abs() * 100.0
}

/// Calculate maker/taker fees based on order type
/// 
/// # Arguments
/// * `value` - Order value
/// * `order_type` - Order type (affects fee tier)
/// * `maker_rate` - Maker fee rate
/// * `taker_rate` - Taker fee rate
/// 
/// # Returns
/// * Estimated fee
pub fn calculate_trading_fee(
    value: f64,
    order_type: OrderType,
    maker_rate: f64,
    taker_rate: f64,
) -> f64 {
    let rate = if order_type.is_maker_only() {
        maker_rate
    } else {
        taker_rate // Conservative estimate
    };
    calculate_fee(value, rate)
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_value() {
        let order = Order::new(OrderSide::Bid, 100.0, 2.5, OrderType::Limit);
        assert_eq!(order.value(), 250.0);
    }
    
    #[test]
    fn test_order_validation() {
        let valid = Order::new(OrderSide::Bid, 100.0, 1.0, OrderType::Limit);
        assert!(valid.validate().is_ok());
        
        let invalid = Order {
            side: OrderSide::Bid,
            price: -1.0,
            size: 1.0,
            order_type: OrderType::Limit,
            client_order_id: 1,
        };
        assert!(invalid.validate().is_err());
    }
    
    #[test]
    fn test_position_pnl() {
        let mut position = Position::new(1000.0);
        
        // Buy 5 units at $100
        position.base_amount = 5.0;
        position.avg_entry_price = 100.0;
        position.quote_amount = 500.0;
        
        // Price goes to $110
        let pnl = position.calculate_pnl(110.0);
        assert_eq!(pnl, 50.0);
        
        // Total value
        assert_eq!(position.total_value(110.0), 1050.0);
        
        // ROI
        assert!((position.roi(110.0, 1000.0) - 5.0).abs() < 0.01);
    }
    
    #[test]
    fn test_order_side() {
        assert_eq!(OrderSide::Bid.as_str(), "BUY");
        assert_eq!(OrderSide::Ask.as_str(), "SELL");
        assert_eq!(OrderSide::Bid.opposite(), OrderSide::Ask);
        assert!(OrderSide::Bid.is_buy());
        assert!(OrderSide::Ask.is_sell());
    }
    
    #[test]
    fn test_order_status() {
        assert!(OrderStatus::Filled.is_terminal());
        assert!(OrderStatus::Open.is_active());
        assert!(OrderStatus::Pending.is_pending());
        assert!(!OrderStatus::Filled.is_active());
    }
    
    #[test]
    fn test_fee_calculation() {
        let order_value = 1000.0;
        let fee_rate = 0.001;
        assert_eq!(calculate_fee(order_value, fee_rate), 1.0);
        
        // Maker vs taker
        let maker_fee = calculate_trading_fee(
            1000.0,
            OrderType::PostOnly,
            0.0002,
            0.0004,
        );
        assert_eq!(maker_fee, 0.2);
        
        let taker_fee = calculate_trading_fee(
            1000.0,
            OrderType::Limit,
            0.0002,
            0.0004,
        );
        assert_eq!(taker_fee, 0.4);
    }
    
    #[test]
    fn test_slippage() {
        let expected = 100.0;
        let actual = 101.0;
        let slippage = calculate_slippage(expected, actual);
        assert!((slippage - 1.0).abs() < 0.01);
    }
}
