//! 🛡️ Slippage Guardian - Adaptive Slippage Protection
//!
//! Validates trades before execution to prevent excessive slippage.
//! Adapts tolerance based on market volatility and liquidity conditions.

use anyhow::{bail, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::fmt;

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageConfig {
    /// Maximum allowed slippage in basis points (100 bps = 1%)
    pub max_slippage_bps: u16,
    
    /// Enable adaptive slippage based on volatility
    pub adaptive_mode: bool,
    
    /// Multiplier for high volatility periods (e.g., 1.5x = 50% more tolerance)
    pub volatility_multiplier: f64,
    
    /// Minimum slippage tolerance in basis points (floor)
    pub min_slippage_bps: u16,
    
    /// Alert threshold in basis points (warn if slippage > this)
    pub alert_threshold_bps: u16,
}

impl Default for SlippageConfig {
    fn default() -> Self {
        Self {
            max_slippage_bps: 50,        // 0.5% max (conservative)
            adaptive_mode: true,         // Enable smart adjustment
            volatility_multiplier: 1.5,  // Allow 50% more in volatile markets
            min_slippage_bps: 10,        // 0.1% minimum
            alert_threshold_bps: 30,     // Alert at 0.3%
        }
    }
}

impl SlippageConfig {
    /// Create aggressive config (higher tolerance)
    pub fn aggressive() -> Self {
        Self {
            max_slippage_bps: 100,       // 1% max
            volatility_multiplier: 2.0,  // Allow 100% more
            ..Default::default()
        }
    }
    
    /// Create ultra-conservative config (minimal tolerance)
    pub fn ultra_conservative() -> Self {
        Self {
            max_slippage_bps: 20,        // 0.2% max
            volatility_multiplier: 1.2,  // Only 20% more
            min_slippage_bps: 5,         // 0.05% minimum
            ..Default::default()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Validation Result
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageValidation {
    /// Whether the trade passed validation
    pub is_valid: bool,
    
    /// Actual slippage in basis points
    pub actual_slippage_bps: u16,
    
    /// Maximum allowed slippage for this trade (may be adjusted)
    pub allowed_slippage_bps: u16,
    
    /// Expected execution price
    pub expected_price: f64,
    
    /// Actual/quoted execution price
    pub actual_price: f64,
    
    /// Reason for rejection (if invalid)
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlippageError {
    ExcessiveSlippage {
        actual_bps: u16,
        max_bps: u16,
    },
    InvalidPrice {
        reason: String,
    },
}

impl fmt::Display for SlippageError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SlippageError::ExcessiveSlippage { actual_bps, max_bps } => {
                write!(
                    f,
                    "Excessive slippage: {:.2}% > {:.2}% max",
                    actual_bps as f64 / 100.0,
                    max_bps as f64 / 100.0
                )
            }
            SlippageError::InvalidPrice { reason } => {
                write!(f, "Invalid price: {}", reason)
            }
        }
    }
}

impl std::error::Error for SlippageError {}

// ═══════════════════════════════════════════════════════════════════════════
// Slippage Guardian
// ═══════════════════════════════════════════════════════════════════════════

pub struct SlippageGuardian {
    config: SlippageConfig,
    rejections_count: std::sync::atomic::AtomicU64,
}

impl SlippageGuardian {
    /// Create new slippage guardian
    pub fn new(config: SlippageConfig) -> Self {
        info!("🛡️  Initializing Slippage Guardian");
        info!("   Max slippage: {:.2}%", config.max_slippage_bps as f64 / 100.0);
        info!("   Adaptive mode: {}", if config.adaptive_mode { "ON" } else { "OFF" });
        info!("   Alert threshold: {:.2}%", config.alert_threshold_bps as f64 / 100.0);
        
        Self {
            config,
            rejections_count: std::sync::atomic::AtomicU64::new(0),
        }
    }
    
    /// Validate trade before execution
    pub fn validate_trade(
        &self,
        expected_price: f64,
        actual_price: f64,
        current_volatility_pct: Option<f64>,
    ) -> Result<SlippageValidation> {
        // Validate inputs
        if expected_price <= 0.0 || actual_price <= 0.0 {
            return Ok(SlippageValidation {
                is_valid: false,
                actual_slippage_bps: 0,
                allowed_slippage_bps: 0,
                expected_price,
                actual_price,
                rejection_reason: Some("Invalid price: must be positive".to_string()),
            });
        }
        
        // Calculate actual slippage in basis points
        let slippage_pct = ((actual_price - expected_price) / expected_price).abs() * 100.0;
        let actual_slippage_bps = (slippage_pct * 100.0) as u16;
        
        // Determine allowed slippage (may be adjusted for volatility)
        let allowed_slippage_bps = if self.config.adaptive_mode {
            self.calculate_adaptive_slippage(current_volatility_pct)
        } else {
            self.config.max_slippage_bps
        };
        
        // Check if slippage exceeds limit
        let is_valid = actual_slippage_bps <= allowed_slippage_bps;
        
        let rejection_reason = if !is_valid {
            self.rejections_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            warn!(
                "⚠️  SLIPPAGE REJECTED: {:.2}% > {:.2}% max (Expected: ${:.4}, Actual: ${:.4})",
                actual_slippage_bps as f64 / 100.0,
                allowed_slippage_bps as f64 / 100.0,
                expected_price,
                actual_price
            );
            Some(format!(
                "Slippage {:.2}% exceeds maximum {:.2}%",
                actual_slippage_bps as f64 / 100.0,
                allowed_slippage_bps as f64 / 100.0
            ))
        } else if actual_slippage_bps > self.config.alert_threshold_bps {
            warn!(
                "⚠️  HIGH SLIPPAGE: {:.2}% (Expected: ${:.4}, Actual: ${:.4})",
                actual_slippage_bps as f64 / 100.0,
                expected_price,
                actual_price
            );
            None
        } else {
            debug!(
                "✅ Slippage OK: {:.2}% < {:.2}% max",
                actual_slippage_bps as f64 / 100.0,
                allowed_slippage_bps as f64 / 100.0
            );
            None
        };
        
        Ok(SlippageValidation {
            is_valid,
            actual_slippage_bps,
            allowed_slippage_bps,
            expected_price,
            actual_price,
            rejection_reason,
        })
    }
    
    /// Calculate adaptive slippage tolerance based on volatility
    fn calculate_adaptive_slippage(&self, current_volatility_pct: Option<f64>) -> u16 {
        let base_slippage = self.config.max_slippage_bps;
        
        // If no volatility data, use base
        let volatility = match current_volatility_pct {
            Some(v) if v > 0.0 => v,
            _ => return base_slippage,
        };
        
        // Adjust based on volatility:
        // Low volatility (< 1%): use base
        // Medium volatility (1-3%): use base
        // High volatility (> 3%): use base * multiplier
        let adjusted = if volatility > 3.0 {
            (base_slippage as f64 * self.config.volatility_multiplier) as u16
        } else {
            base_slippage
        };
        
        // Clamp to configured bounds
        adjusted.max(self.config.min_slippage_bps)
    }
    
    /// Get rejection statistics
    pub fn get_rejection_count(&self) -> u64 {
        self.rejections_count.load(std::sync::atomic::Ordering::SeqCst)
    }
    
    /// Reset rejection counter
    pub fn reset_stats(&self) {
        self.rejections_count.store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_defaults() {
        let config = SlippageConfig::default();
        assert_eq!(config.max_slippage_bps, 50); // 0.5%
        assert!(config.adaptive_mode);
        assert!(config.min_slippage_bps < config.max_slippage_bps);
    }
    
    #[test]
    fn test_aggressive_config() {
        let config = SlippageConfig::aggressive();
        assert_eq!(config.max_slippage_bps, 100); // 1%
        assert!(config.max_slippage_bps > SlippageConfig::default().max_slippage_bps);
    }
    
    #[test]
    fn test_ultra_conservative_config() {
        let config = SlippageConfig::ultra_conservative();
        assert_eq!(config.max_slippage_bps, 20); // 0.2%
        assert!(config.max_slippage_bps < SlippageConfig::default().max_slippage_bps);
    }
    
    #[test]
    fn test_guardian_creation() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        assert_eq!(guardian.get_rejection_count(), 0);
    }
    
    #[test]
    fn test_acceptable_slippage() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // 0.3% slippage (30 bps) - should pass
        let validation = guardian.validate_trade(100.0, 100.3, None).unwrap();
        
        assert!(validation.is_valid);
        assert_eq!(validation.actual_slippage_bps, 30);
        assert!(validation.rejection_reason.is_none());
    }
    
    #[test]
    fn test_excessive_slippage() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // 1% slippage (100 bps) - should fail (max is 50 bps)
        let validation = guardian.validate_trade(100.0, 101.0, None).unwrap();
        
        assert!(!validation.is_valid);
        assert_eq!(validation.actual_slippage_bps, 100);
        assert!(validation.rejection_reason.is_some());
        assert_eq!(guardian.get_rejection_count(), 1);
    }
    
    #[test]
    fn test_negative_slippage() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // Better price than expected (negative slippage) - should pass
        let validation = guardian.validate_trade(100.0, 99.7, None).unwrap();
        
        assert!(validation.is_valid);
        assert_eq!(validation.actual_slippage_bps, 30);
    }
    
    #[test]
    fn test_invalid_prices() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // Zero price
        let validation = guardian.validate_trade(0.0, 100.0, None).unwrap();
        assert!(!validation.is_valid);
        
        // Negative price
        let validation = guardian.validate_trade(100.0, -1.0, None).unwrap();
        assert!(!validation.is_valid);
    }
    
    #[test]
    fn test_adaptive_slippage_low_volatility() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // Low volatility (0.5%) - use base slippage
        let allowed = guardian.calculate_adaptive_slippage(Some(0.5));
        assert_eq!(allowed, config.max_slippage_bps);
    }
    
    #[test]
    fn test_adaptive_slippage_high_volatility() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // High volatility (5%) - increased tolerance
        let allowed = guardian.calculate_adaptive_slippage(Some(5.0));
        assert!(allowed > config.max_slippage_bps);
        assert_eq!(allowed, (config.max_slippage_bps as f64 * config.volatility_multiplier) as u16);
    }
    
    #[test]
    fn test_adaptive_mode_integration() {
        let config = SlippageConfig {
            adaptive_mode: true,
            max_slippage_bps: 50,
            volatility_multiplier: 2.0,
            ..Default::default()
        };
        let guardian = SlippageGuardian::new(config);
        
        // High volatility allows more slippage
        // 0.7% slippage (70 bps) would fail in normal conditions (max 50 bps)
        // But passes in high volatility (50 * 2.0 = 100 bps max)
        let validation = guardian.validate_trade(100.0, 100.7, Some(5.0)).unwrap();
        
        assert!(validation.is_valid);
        assert_eq!(validation.actual_slippage_bps, 70);
        assert_eq!(validation.allowed_slippage_bps, 100);
    }
    
    #[test]
    fn test_rejection_counter() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        assert_eq!(guardian.get_rejection_count(), 0);
        
        // Reject 3 trades
        guardian.validate_trade(100.0, 101.0, None).unwrap(); // Reject
        guardian.validate_trade(100.0, 102.0, None).unwrap(); // Reject
        guardian.validate_trade(100.0, 100.2, None).unwrap(); // Accept
        guardian.validate_trade(100.0, 103.0, None).unwrap(); // Reject
        
        assert_eq!(guardian.get_rejection_count(), 3);
        
        guardian.reset_stats();
        assert_eq!(guardian.get_rejection_count(), 0);
    }
}
