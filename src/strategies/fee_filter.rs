//! Fee-Aware Trade Filtering
//! Prevents unprofitable trades that get eaten by fees
//! Based on GIGA test results: activity paradox discovered!

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeFilterConfig {
    /// Base trading fee (typically 0.04% on Solana DEXs)
    pub base_fee_percent: f64,
    
    /// Minimum profit multiplier (2x = profit must exceed 2x fees)
    pub min_profit_multiplier: f64,
    
    /// Enable/disable fee filtering
    pub enabled: bool,
    
    /// Slippage tolerance
    pub max_slippage_percent: f64,
}

impl Default for FeeFilterConfig {
    fn default() -> Self {
        Self {
            base_fee_percent: 0.04,        // 0.04% standard Solana fee
            min_profit_multiplier: 2.0,    // Must profit 2x fees
            enabled: true,
            max_slippage_percent: 0.1,     // 0.1% max slippage
        }
    }
}

pub struct FeeFilter {
    config: FeeFilterConfig,
}

impl FeeFilter {
    pub fn new(config: FeeFilterConfig) -> Self {
        Self { config }
    }
    
    /// Check if a trade should be executed based on expected profit vs fees
    pub fn should_execute_trade(
        &self,
        entry_price: f64,
        exit_price: f64,
        _position_size: f64,
    ) -> bool {
        if !self.config.enabled {
            return true; // Filter disabled, allow all trades
        }
        
        // Calculate expected profit percentage
        let price_diff = (exit_price - entry_price).abs();
        let profit_percent = (price_diff / entry_price) * 100.0;
        
        // Calculate total fees (buy + sell)
        let total_fee_percent = self.config.base_fee_percent * 2.0;
        
        // Calculate minimum required profit
        let min_profit_required = total_fee_percent * self.config.min_profit_multiplier;
        
        // Add slippage buffer
        let profit_with_slippage = profit_percent - self.config.max_slippage_percent;
        
        // Decision
        let should_trade = profit_with_slippage >= min_profit_required;
        
        if !should_trade {
            log::debug!(
                "🚫 Trade filtered! Expected profit: {:.3}%, Required: {:.3}%, Fees: {:.3}%",
                profit_percent,
                min_profit_required,
                total_fee_percent
            );
        } else {
            log::debug!(
                "✅ Trade approved! Profit: {:.3}%, After fees: {:.3}%",
                profit_percent,
                profit_with_slippage - total_fee_percent
            );
        }
        
        should_trade
    }
    
    /// Calculate expected net profit after all fees
    pub fn calculate_net_profit(
        &self,
        entry_price: f64,
        exit_price: f64,
        position_size_usdc: f64,
    ) -> f64 {
        let gross_profit = (exit_price - entry_price) * (position_size_usdc / entry_price);
        
        // Deduct fees (buy + sell)
        let buy_fee = position_size_usdc * (self.config.base_fee_percent / 100.0);
        let sell_value = position_size_usdc + gross_profit;
        let sell_fee = sell_value * (self.config.base_fee_percent / 100.0);
        
        let net_profit = gross_profit - buy_fee - sell_fee;
        
        net_profit
    }
    
    /// Get minimum spread required for profitable trade
    pub fn get_min_profitable_spread(&self) -> f64 {
        let total_fee = self.config.base_fee_percent * 2.0;
        let min_spread = total_fee * self.config.min_profit_multiplier;
        min_spread + self.config.max_slippage_percent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fee_filter_blocks_unprofitable_trades() {
        let filter = FeeFilter::new(FeeFilterConfig::default());
        
        // Small spread that won't cover fees
        let result = filter.should_execute_trade(100.0, 100.10, 1000.0);
        assert_eq!(result, false, "Should block trades with insufficient profit");
    }
    
    #[test]
    fn test_fee_filter_allows_profitable_trades() {
        let filter = FeeFilter::new(FeeFilterConfig::default());
        
        // Good spread that covers fees + profit
        let result = filter.should_execute_trade(100.0, 100.30, 1000.0);
        assert_eq!(result, true, "Should allow trades with sufficient profit");
    }
    
    #[test]
    fn test_net_profit_calculation() {
        let filter = FeeFilter::new(FeeFilterConfig::default());
        
        let net_profit = filter.calculate_net_profit(100.0, 101.0, 1000.0);
        
        // Gross profit: $10 (1% on $1000)
        // Buy fee: $0.40 (0.04% of $1000)
        // Sell fee: $0.40 (0.04% of $1010)
        // Net: ~$9.20
        assert!(net_profit > 9.0 && net_profit < 10.0);
    }
}
