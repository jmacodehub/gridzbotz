//! 📊 DEX Market State Management
//! 
//! Tracks market information including:
//! - Token mints and lot sizes
//! - Best bid/ask prices
//! - Last traded price
//! - Spread calculations

use serde::{Deserialize, Serialize};
use solana_sdk::pubkey::Pubkey;

// ═══════════════════════════════════════════════════════════════════════════
// MARKET STATE
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketState {
    /// Market public key
    pub market: Pubkey,
    
    /// Base token mint (e.g., SOL)
    pub base_mint: Pubkey,
    
    /// Quote token mint (e.g., USDC)
    pub quote_mint: Pubkey,
    
    /// Base currency lot size
    pub base_lot_size: u64,
    
    /// Quote currency lot size
    pub quote_lot_size: u64,
    
    /// Current best bid price
    pub best_bid: f64,
    
    /// Current best ask price
    pub best_ask: f64,
    
    /// Last traded price
    pub last_price: f64,
    
    /// 24h trading volume (optional)
    pub volume_24h: Option<f64>,
}

impl MarketState {
    /// Create a new MarketState with default values
    pub fn new(market: Pubkey) -> Self {
        Self {
            market,
            base_mint: Pubkey::default(),
            quote_mint: Pubkey::default(),
            base_lot_size: 100_000,
            quote_lot_size: 100,
            best_bid: 0.0,
            best_ask: 0.0,
            last_price: 0.0,
            volume_24h: None,
        }
    }
    
    /// Create with full initialization
    pub fn with_config(
        market: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        base_lot_size: u64,
        quote_lot_size: u64,
    ) -> Self {
        Self {
            market,
            base_mint,
            quote_mint,
            base_lot_size,
            quote_lot_size,
            best_bid: 0.0,
            best_ask: 0.0,
            last_price: 0.0,
            volume_24h: None,
        }
    }
    
    /// Update market prices
    pub fn update_prices(&mut self, bid: f64, ask: f64, last: f64) {
        self.best_bid = bid;
        self.best_ask = ask;
        self.last_price = last;
    }
    
    /// Calculate spread percentage
    pub fn spread_pct(&self) -> f64 {
        if self.best_bid > 0.0 {
            ((self.best_ask - self.best_bid) / self.best_bid) * 100.0
        } else {
            0.0
        }
    }
    
    /// Calculate mid price
    pub fn mid_price(&self) -> f64 {
        if self.best_bid > 0.0 && self.best_ask > 0.0 {
            (self.best_bid + self.best_ask) / 2.0
        } else {
            self.last_price
        }
    }
    
    /// Check if market data is valid
    pub fn is_valid(&self) -> bool {
        self.best_bid > 0.0 && self.best_ask > 0.0 && self.best_ask >= self.best_bid
    }
    
    /// Display market info
    pub fn display(&self) {
        println!("📊 Market State:");
        println!("   Market:     {}", self.market);
        println!("   Best Bid:   ${:.4}", self.best_bid);
        println!("   Best Ask:   ${:.4}", self.best_ask);
        println!("   Mid Price:  ${:.4}", self.mid_price());
        println!("   Spread:     {:.2}%", self.spread_pct());
        
        if let Some(vol) = self.volume_24h {
            println!("   24h Volume: ${:.2}", vol);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spread_calculation() {
        let mut state = MarketState::new(Pubkey::new_unique());
        state.update_prices(100.0, 101.0, 100.5);
        
        assert!((state.spread_pct() - 1.0).abs() < 0.01);
    }
    
    #[test]
    fn test_mid_price() {
        let mut state = MarketState::new(Pubkey::new_unique());
        state.update_prices(100.0, 102.0, 101.0);
        
        assert_eq!(state.mid_price(), 101.0);
    }
    
    #[test]
    fn test_is_valid() {
        let mut state = MarketState::new(Pubkey::new_unique());
        
        // Invalid: no prices set
        assert!(!state.is_valid());
        
        // Valid prices
        state.update_prices(100.0, 101.0, 100.5);
        assert!(state.is_valid());
        
        // Invalid: bid > ask
        state.update_prices(102.0, 101.0, 101.5);
        assert!(!state.is_valid());
    }
}
