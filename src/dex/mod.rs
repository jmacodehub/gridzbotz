//! 🔗 DEX Integration Module
//! 
//! Provides interfaces for decentralized exchange trading on Solana:
//! - Serum/OpenBook DEX integration
//! - Order placement and cancellation
//! - Market state monitoring and caching
//! - Position tracking and P&L calculation
//! - Order lifecycle management
//! 
//! # Example
//! ```
//! use solana_grid_bot::dex::{SerumClient, OrderSide};
//! use solana_sdk::signature::Keypair;
//! use solana_sdk::pubkey::Pubkey;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let wallet = Keypair::new();
//! let market = Pubkey::new_unique();
//! 
//! let mut client = SerumClient::new(
//!     "https://api.mainnet-beta.solana.com".to_string(),
//!     wallet,
//!     market,
//! )?;
//! 
//! // Place order
//! let order = client.place_limit_order(OrderSide::Bid, 193.50, 1.0).await?;
//! # Ok(())
//! # }
//! ```

pub mod serum_client;
pub mod order_manager;
pub mod market_state;

// Re-export main types for convenience
pub use serum_client::{SerumClient, ClientStats, MarketInfo};
pub use order_manager::{OrderManager, OrderStats};
pub use market_state::MarketState;

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// ═══════════════════════════════════════════════════════════════════════════
// ORDER TYPES
// ═══════════════════════════════════════════════════════════════════════════

/// Order side: Buy or Sell
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderSide {
    /// Buy order (bid)
    Bid,
    /// Sell order (ask)
    Ask,
}

impl OrderSide {
    /// Convert to human-readable string
    pub fn as_str(&self) -> &'static str {
        match self {
            OrderSide::Bid => "BUY",
            OrderSide::Ask => "SELL",
        }
    }
}

/// Order type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    /// Standard limit order
    Limit,
    /// Immediate or cancel (market-like behavior)
    ImmediateOrCancel,
    /// Post-only (maker-only, no taker fees)
    PostOnly,
}

impl OrderType {
    /// Check if order is maker-only
    pub fn is_maker_only(&self) -> bool {
        matches!(self, OrderType::PostOnly)
    }
}

/// Order structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Order {
    /// Buy or sell
    pub side: OrderSide,
    /// Limit price in quote currency
    pub price: f64,
    /// Order size in base currency
    pub size: f64,
    /// Order type (limit, IOC, post-only)
    pub order_type: OrderType,
    /// Client-generated order ID
    pub client_order_id: u64,
}

impl Order {
    /// Calculate order value in quote currency
    pub fn value(&self) -> f64 {
        self.price * self.size
    }
    
    /// Display order details
    pub fn display(&self) {
        println!("📝 Order Details:");
        println!("   Side:       {}", self.side.as_str());
        println!("   Price:      ${:.4}", self.price);
        println!("   Size:       {:.4}", self.size);
        println!("   Value:      ${:.2}", self.value());
        println!("   Type:       {:?}", self.order_type);
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
    /// Order owner
    pub owner: Pubkey,
    /// Timestamp when placed
    pub timestamp: i64,
}

impl PlacedOrder {
    /// Check if order is buy
    pub fn is_buy(&self) -> bool {
        matches!(self.order.side, OrderSide::Bid)
    }
    
    /// Check if order is sell
    pub fn is_sell(&self) -> bool {
        matches!(self.order.side, OrderSide::Ask)
    }
    
    /// Get order age in seconds
    pub fn age_seconds(&self) -> i64 {
        chrono::Utc::now().timestamp() - self.timestamp
    }
}

/// Trading position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    /// Amount of base currency held
    pub base_amount: f64,
    /// Amount of quote currency held
    pub quote_amount: f64,
    /// Weighted average entry price
    pub avg_entry_price: f64,
    /// Unrealized profit/loss
    pub unrealized_pnl: f64,
    /// Realized profit/loss
    pub realized_pnl: f64,
}

impl Position {
    /// Create empty position with starting capital
    pub fn new(starting_capital: f64) -> Self {
        Self {
            base_amount: 0.0,
            quote_amount: starting_capital,
            avg_entry_price: 0.0,
            unrealized_pnl: 0.0,
            realized_pnl: 0.0,
        }
    }
    
    /// Calculate current P&L given current price
    pub fn calculate_pnl(&self, current_price: f64) -> f64 {
        if self.base_amount > 0.0 && self.avg_entry_price > 0.0 {
            (current_price - self.avg_entry_price) * self.base_amount
        } else {
            0.0
        }
    }
    
    /// Calculate total value at current price
    pub fn total_value(&self, current_price: f64) -> f64 {
        (self.base_amount * current_price) + self.quote_amount
    }
    
    /// Calculate ROI percentage
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
    
    /// Display position info
    pub fn display(&self, current_price: f64) {
        println!("\n💼 Current Position:");
        println!("   Base:          {:.4}", self.base_amount);
        println!("   Quote:         ${:.2}", self.quote_amount);
        println!("   Entry Price:   ${:.4}", self.avg_entry_price);
        println!("   Current Price: ${:.4}", current_price);
        println!("   Unrealized:    ${:+.2}", self.calculate_pnl(current_price));
        println!("   Realized:      ${:+.2}", self.realized_pnl);
        println!("   Total Value:   ${:.2}", self.total_value(current_price));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════

/// Calculate fee for an order
pub fn calculate_fee(value: f64, fee_rate: f64) -> f64 {
    value * fee_rate
}

/// Calculate slippage between expected and actual price
pub fn calculate_slippage(expected_price: f64, actual_price: f64) -> f64 {
    ((actual_price - expected_price) / expected_price).abs() * 100.0
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_order_value() {
        let order = Order {
            side: OrderSide::Bid,
            price: 100.0,
            size: 2.5,
            order_type: OrderType::Limit,
            client_order_id: 12345,
        };
        
        assert_eq!(order.value(), 250.0);
    }
    
    #[test]
    fn test_position_pnl() {
        let mut position = Position::new(1000.0);
        
        // Buy 5 units at $100
        position.base_amount = 5.0;
        position.avg_entry_price = 100.0;
        position.quote_amount = 500.0; // 1000 - 500 spent
        
        // Price goes to $110
        let pnl = position.calculate_pnl(110.0);
        assert_eq!(pnl, 50.0); // (110 - 100) * 5 = 50
        
        // Total value should be 1050
        assert_eq!(position.total_value(110.0), 1050.0);
    }
    
    #[test]
    fn test_order_side() {
        assert_eq!(OrderSide::Bid.as_str(), "BUY");
        assert_eq!(OrderSide::Ask.as_str(), "SELL");
    }
    
    #[test]
    fn test_fee_calculation() {
        let order_value = 1000.0;
        let fee_rate = 0.001; // 0.1%
        
        let fee = calculate_fee(order_value, fee_rate);
        assert_eq!(fee, 1.0);
    }
    
    #[test]
    fn test_slippage() {
        let expected = 100.0;
        let actual = 101.0;
        
        let slippage = calculate_slippage(expected, actual);
        assert!((slippage - 1.0).abs() < 0.01);
    }
}
