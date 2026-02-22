//! 🛑 Stop-Loss & Take-Profit Manager

use crate::Config;
use log::{info, warn};

pub struct StopLossManager {
    enabled: bool,
    stop_loss_pct: f64,
    take_profit_pct: f64,
    trailing_stop: bool,
    highest_price: f64,
}

impl StopLossManager {
    pub fn new(config: &Config) -> Self {
        info!("🛑 Initializing Stop-Loss Manager");
        
        if config.risk.enable_circuit_breaker {
            info!("   Stop-loss: -{:.1}%", config.risk.stop_loss_pct);
        }
        
        if config.risk.enable_circuit_breaker {
            info!("   Take-profit: +{:.1}%", config.risk.take_profit_pct);
        }
        
        Self {
            enabled: config.risk.enable_circuit_breaker,
            stop_loss_pct: config.risk.stop_loss_pct,
            take_profit_pct: config.risk.take_profit_pct,
            trailing_stop: config.risk.enable_circuit_breaker,
            highest_price: 0.0,
        }
    }
    
    /// Check if stop-loss should trigger.
    ///
    /// When trailing stop is enabled, the high-water mark is lazily
    /// initialised to `entry_price` on the first call so the reference
    /// is the entry, not the current price.  The mark then only moves up.
    pub fn should_stop_loss(&mut self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }
        
        // Lazily initialise the trailing high-water mark on the first call
        // for this position.  Without this, highest_price starts at 0.0
        // and the first current_price always becomes the reference, giving
        // 0% loss and silently preventing the stop from ever firing.
        if self.trailing_stop && self.highest_price == 0.0 {
            self.highest_price = entry_price;
        }
        
        // Only advance the high watermark — never retreat it.
        if self.trailing_stop && current_price > self.highest_price {
            self.highest_price = current_price;
        }
        
        let reference_price = if self.trailing_stop {
            self.highest_price
        } else {
            entry_price
        };
        
        let loss_pct = ((current_price - reference_price) / reference_price) * 100.0;
        
        if loss_pct <= -self.stop_loss_pct {
            warn!("🛑 STOP-LOSS TRIGGERED!");
            warn!("   Entry: ${:.4} | Current: ${:.4} | Loss: {:.2}%", 
                  entry_price, current_price, loss_pct);
            return true;
        }
        
        false
    }
    
    /// Check if take-profit should trigger
    pub fn should_take_profit(&self, entry_price: f64, current_price: f64) -> bool {
        let profit_pct = ((current_price - entry_price) / entry_price) * 100.0;
        
        if profit_pct >= self.take_profit_pct {
            info!("🎯 TAKE-PROFIT TRIGGERED!");
            info!("   Entry: ${:.4} | Current: ${:.4} | Profit: {:.2}%", 
                  entry_price, current_price, profit_pct);
            return true;
        }
        
        false
    }
    
    /// Reset for new position — call this when entering a new trade.
    pub fn reset(&mut self, entry_price: f64) {
        self.highest_price = entry_price;
    }
}
