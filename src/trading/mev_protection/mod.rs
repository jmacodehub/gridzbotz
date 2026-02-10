//! ═══════════════════════════════════════════════════════════════════════════
//! 🛡️ MEV PROTECTION - Production-Grade Frontrunning Defense
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! **THE PROBLEM:**
//! Grid bots are sitting ducks for MEV bots:
//! - Frontrunning: MEV bot sees your buy, buys first, sells to you at higher price
//! - Sandwiching: MEV bot buys before + sells after = you get worst price
//! - JIT Liquidity: Fake liquidity appears/disappears to extract fees
//!
//! **OUR SOLUTION (3-Layer Defense):**
//! 1. **Jito Bundles:** Atomic transaction execution (all-or-nothing)
//! 2. **Priority Fee Optimizer:** Pay optimal fees (not too much, not too little)
//! 3. **Slippage Guardian:** Reject trades with excessive slippage
//!
//! **CONSERVATIVE DEFAULTS:**
//! - Priority Fee: 50th percentile (balance cost vs speed)
//! - Max Slippage: 0.5% (50 bps)
//! - Jito Tip: 1,000 lamports (~$0.0002 per bundle)
//!
//! February 11, 2026 | GridzBotz V5.0 - MEV Protection
//! ═══════════════════════════════════════════════════════════════════════════

mod priority_fee;
mod slippage;
mod jito_client;

pub use priority_fee::{PriorityFeeOptimizer, PriorityFeeConfig, FeeRecommendation};
pub use slippage::{SlippageGuardian, SlippageConfig, SlippageValidation};
pub use jito_client::{JitoClient, JitoConfig, JitoBundleStatus};

use anyhow::Result;
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// 📊 MEV PROTECTION CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Complete MEV protection configuration with conservative defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MevProtectionConfig {
    /// Enable MEV protection (disable for testing only!)
    pub enabled: bool,
    
    /// Priority fee optimization settings
    pub priority_fee: PriorityFeeConfig,
    
    /// Slippage protection settings
    pub slippage: SlippageConfig,
    
    /// Jito bundle settings
    pub jito: JitoConfig,
}

impl Default for MevProtectionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            priority_fee: PriorityFeeConfig::default(),
            slippage: SlippageConfig::default(),
            jito: JitoConfig::default(),
        }
    }
}

impl MevProtectionConfig {
    /// Create conservative config for production
    pub fn conservative() -> Self {
        Self::default()
    }
    
    /// Create aggressive config for high-volatility markets
    pub fn aggressive() -> Self {
        Self {
            enabled: true,
            priority_fee: PriorityFeeConfig {
                enabled: true,
                target_percentile: 75, // Pay more for faster inclusion
                sample_size: 150,
                min_fee_microlamports: 5_000,
                max_fee_microlamports: 100_000,
            },
            slippage: SlippageConfig {
                enabled: true,
                max_slippage_bps: 100, // 1.0% for volatile markets
                dynamic_adjustment: true,
                volatility_multiplier: 1.5,
            },
            jito: JitoConfig {
                enabled: true,
                tip_lamports: 5_000, // Higher tip for priority
                block_engine_url: "https://mainnet.block-engine.jito.wtf".to_string(),
                max_bundle_size: 5,
            },
        }
    }
    
    /// Create config for testing (MEV protection disabled)
    pub fn test_mode() -> Self {
        Self {
            enabled: false,
            priority_fee: PriorityFeeConfig {
                enabled: false,
                ..Default::default()
            },
            slippage: SlippageConfig {
                enabled: false,
                ..Default::default()
            },
            jito: JitoConfig {
                enabled: false,
                ..Default::default()
            },
        }
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if !self.enabled {
            log::warn!("⚠️  MEV Protection DISABLED - use only for testing!");
            return Ok(());
        }
        
        self.priority_fee.validate()?;
        self.slippage.validate()?;
        self.jito.validate()?;
        
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🛡️ MEV PROTECTION MANAGER
// ═══════════════════════════════════════════════════════════════════════════

/// Main MEV protection manager - coordinates all protection layers
pub struct MevProtectionManager {
    config: MevProtectionConfig,
    priority_fee_optimizer: PriorityFeeOptimizer,
    slippage_guardian: SlippageGuardian,
    jito_client: Option<JitoClient>,
}

impl MevProtectionManager {
    /// Create new MEV protection manager
    pub fn new(config: MevProtectionConfig) -> Result<Self> {
        config.validate()?;
        
        let priority_fee_optimizer = PriorityFeeOptimizer::new(config.priority_fee.clone())?;
        let slippage_guardian = SlippageGuardian::new(config.slippage.clone());
        
        let jito_client = if config.jito.enabled {
            Some(JitoClient::new(config.jito.clone())?)
        } else {
            None
        };
        
        log::info!("🛡️  MEV Protection Manager initialized");
        log::info!("   Priority Fee: {} (target: {}th percentile)", 
            if config.priority_fee.enabled { "ENABLED" } else { "DISABLED" },
            config.priority_fee.target_percentile
        );
        log::info!("   Slippage Guard: {} (max: {:.2}%)",
            if config.slippage.enabled { "ENABLED" } else { "DISABLED" },
            config.slippage.max_slippage_bps as f64 / 100.0
        );
        log::info!("   Jito Bundles: {} (tip: {} lamports)",
            if config.jito.enabled { "ENABLED" } else { "DISABLED" },
            config.jito.tip_lamports
        );
        
        Ok(Self {
            config,
            priority_fee_optimizer,
            slippage_guardian,
            jito_client,
        })
    }
    
    /// Get optimal priority fee for current network conditions
    pub async fn get_optimal_priority_fee(&self) -> Result<u64> {
        if !self.config.priority_fee.enabled {
            return Ok(0);
        }
        
        self.priority_fee_optimizer.get_optimal_fee().await
    }
    
    /// Validate trade slippage before execution
    pub fn validate_slippage(
        &self,
        expected_price: f64,
        actual_price: f64,
    ) -> Result<SlippageValidation> {
        if !self.config.slippage.enabled {
            return Ok(SlippageValidation {
                is_acceptable: true,
                slippage_bps: 0,
                max_slippage_bps: 0,
                message: "Slippage protection disabled".to_string(),
            });
        }
        
        self.slippage_guardian.validate(expected_price, actual_price)
    }
    
    /// Check if Jito bundles are enabled and available
    pub fn is_jito_enabled(&self) -> bool {
        self.jito_client.is_some()
    }
    
    /// Get reference to Jito client
    pub fn jito_client(&self) -> Option<&JitoClient> {
        self.jito_client.as_ref()
    }
    
    /// Get current configuration
    pub fn config(&self) -> &MevProtectionConfig {
        &self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ✅ TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = MevProtectionConfig::default();
        assert!(config.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_conservative_config() {
        let config = MevProtectionConfig::conservative();
        assert_eq!(config.priority_fee.target_percentile, 50);
        assert_eq!(config.slippage.max_slippage_bps, 50); // 0.5%
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_aggressive_config() {
        let config = MevProtectionConfig::aggressive();
        assert_eq!(config.priority_fee.target_percentile, 75);
        assert_eq!(config.slippage.max_slippage_bps, 100); // 1.0%
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_test_mode_disables_protection() {
        let config = MevProtectionConfig::test_mode();
        assert!(!config.enabled);
        assert!(!config.priority_fee.enabled);
        assert!(!config.slippage.enabled);
        assert!(!config.jito.enabled);
    }
}
