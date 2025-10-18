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
    
    /// Check if stop-loss should trigger
    pub fn should_stop_loss(&mut self, entry_price: f64, current_price: f64) -> bool {
        if !self.enabled {
            return false;
        }
        
        // Update trailing stop
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
    
    /// Reset for new position
    pub fn reset(&mut self, entry_price: f64) {
        self.highest_price = entry_price;
    }
}
