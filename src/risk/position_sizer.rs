//! 💰 Position Sizing with Kelly Criterion

use crate::Config;
use anyhow::Result;
use log::info;

pub struct PositionSizer {
    max_position_usd: f64,
    risk_per_trade_pct: f64,
    kelly_fraction: f64,
}

impl PositionSizer {
    pub fn new(config: &Config) -> Self {
        info!("💰 Initializing Position Sizer");
        info!("   Max capital per trade: ${:.2}", config.risk.max_position_size_pct * 10.0);
        info!("   Risk per trade: {:.2}%", config.risk.max_position_size_pct);
        
        Self {
            max_position_usd: config.risk.max_position_size_pct * 10.0,
            risk_per_trade_pct: config.risk.max_position_size_pct,
            kelly_fraction: 0.25, // Conservative Kelly (1/4 Kelly)
        }
    }
    
    /// Calculate position size based on price and volatility
    pub fn calculate_size(&self, price: f64, volatility: f64, win_rate: f64) -> f64 {
        // Kelly Criterion: f = (bp - q) / b
        // where b = odds, p = win probability, q = loss probability
        let kelly = if win_rate > 0.5 {
            (win_rate - (1.0 - win_rate)) / 1.0 * self.kelly_fraction
        } else {
            self.kelly_fraction * 0.5 // Conservative for low win rate
        };
        
        // Calculate size in base currency (SOL)
        let risk_amount = self.max_position_usd * (self.risk_per_trade_pct / 100.0);
        let kelly_adjusted = risk_amount * kelly;
        
        // Adjust for volatility (higher vol = smaller size)
        let vol_adjusted = kelly_adjusted / (1.0 + volatility);
        
        let size = (vol_adjusted / price).min(self.max_position_usd / price);
        
        info!("📏 Position size calculated: {:.4} SOL", size);
        info!("   Price: ${:.2} | Vol: {:.2}% | Win rate: {:.1}%", 
              price, volatility * 100.0, win_rate * 100.0);
        
        size
    }
    
    /// Validate position size against limits
    pub fn validate_size(&self, size: f64, price: f64) -> Result<()> {
        let position_value = size * price;
        
        if position_value > self.max_position_usd {
            anyhow::bail!("Position size ${:.2} exceeds max ${:.2}", 
                         position_value, self.max_position_usd);
        }
        
        if size < 0.001 {
            anyhow::bail!("Position size {:.4} below minimum 0.001", size);
        }
        
        Ok(())
    }
}
