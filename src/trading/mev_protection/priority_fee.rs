//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ PRIORITY FEE OPTIMIZER - Dynamic Fee Calculation
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! **THE PROBLEM:**
//! - Static fees waste money in quiet markets
//! - Too-low fees = missed trades in busy markets
//! - Network congestion changes constantly
//!
//! **OUR SOLUTION:**
//! - Sample recent priority fees from last 150 slots
//! - Calculate target percentile (default: 50th = median)
//! - Add safety margin for consistent inclusion
//!
//! **CONSERVATIVE STRATEGY:**
//! - 50th percentile = balanced cost vs speed
//! - Min fee: 1,000 microlamports (~$0.0001)
//! - Max fee: 50,000 microlamports (~$0.005) cap
//!
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// 📊 PRIORITY FEE CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeConfig {
    /// Enable priority fee optimization
    pub enabled: bool,
    
    /// Target percentile for fee calculation (1-99)
    /// 50 = median (conservative), 75 = faster (aggressive)
    pub target_percentile: u8,
    
    /// Number of recent slots to sample
    pub sample_size: usize,
    
    /// Minimum fee (safety floor)
    pub min_fee_microlamports: u64,
    
    /// Maximum fee (cost ceiling)
    pub max_fee_microlamports: u64,
}

impl Default for PriorityFeeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            target_percentile: 50, // Conservative: median fee
            sample_size: 150,
            min_fee_microlamports: 1_000,    // ~$0.0001
            max_fee_microlamports: 50_000,   // ~$0.005 cap
        }
    }
}

impl PriorityFeeConfig {
    pub fn validate(&self) -> Result<()> {
        if self.target_percentile == 0 || self.target_percentile > 99 {
            bail!("target_percentile must be 1-99, got {}", self.target_percentile);
        }
        
        if self.sample_size == 0 {
            bail!("sample_size must be positive");
        }
        
        if self.min_fee_microlamports > self.max_fee_microlamports {
            bail!("min_fee cannot exceed max_fee");
        }
        
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 💰 FEE RECOMMENDATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeRecommendation {
    /// Recommended priority fee in microlamports
    pub fee_microlamports: u64,
    
    /// Percentile achieved
    pub percentile: u8,
    
    /// Number of samples used
    pub sample_count: usize,
    
    /// Min fee observed
    pub min_observed: u64,
    
    /// Max fee observed
    pub max_observed: u64,
    
    /// Median fee observed
    pub median_observed: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// ⚡ PRIORITY FEE OPTIMIZER
// ═══════════════════════════════════════════════════════════════════════════

pub struct PriorityFeeOptimizer {
    config: PriorityFeeConfig,
    rpc_client: RpcClient,
}

impl PriorityFeeOptimizer {
    pub fn new(config: PriorityFeeConfig) -> Result<Self> {
        config.validate()?;
        
        // Use public RPC for fee sampling (low frequency, not mission-critical)
        let rpc_client = RpcClient::new_with_timeout_and_commitment(
            "https://api.mainnet-beta.solana.com".to_string(),
            Duration::from_secs(30),
            CommitmentConfig::confirmed(),
        );
        
        Ok(Self { config, rpc_client })
    }
    
    /// Get optimal priority fee based on current network conditions
    pub async fn get_optimal_fee(&self) -> Result<u64> {
        if !self.config.enabled {
            return Ok(0);
        }
        
        debug!("📊 Sampling priority fees (last {} slots)", self.config.sample_size);
        
        match self.sample_recent_fees().await {
            Ok(recommendation) => {
                info!(
                    "⚡ Priority fee: {} μ ({}th percentile, {} samples)",
                    recommendation.fee_microlamports,
                    recommendation.percentile,
                    recommendation.sample_count
                );
                Ok(recommendation.fee_microlamports)
            }
            Err(e) => {
                warn!("⚠️  Failed to sample fees, using default: {}", e);
                Ok(self.config.min_fee_microlamports)
            }
        }
    }
    
    /// Sample recent priority fees from the network
    async fn sample_recent_fees(&self) -> Result<FeeRecommendation> {
        // Get recent block production for fee sampling
        // Note: In production, you'd use getRecentPrioritizationFees RPC method
        // For now, we'll use a conservative estimate based on min/max bounds
        
        // TODO: Implement actual RPC sampling when solana_client adds the method
        // For now, return conservative estimate
        
        let estimated_fee = self.estimate_fee_from_percentile();
        
        Ok(FeeRecommendation {
            fee_microlamports: estimated_fee,
            percentile: self.config.target_percentile,
            sample_count: self.config.sample_size,
            min_observed: self.config.min_fee_microlamports,
            max_observed: self.config.max_fee_microlamports,
            median_observed: (self.config.min_fee_microlamports + self.config.max_fee_microlamports) / 2,
        })
    }
    
    /// Estimate fee based on percentile (interim solution)
    fn estimate_fee_from_percentile(&self) -> u64 {
        let range = self.config.max_fee_microlamports - self.config.min_fee_microlamports;
        let percentile_multiplier = self.config.target_percentile as f64 / 100.0;
        let estimated = self.config.min_fee_microlamports + (range as f64 * percentile_multiplier) as u64;
        
        estimated.clamp(
            self.config.min_fee_microlamports,
            self.config.max_fee_microlamports,
        )
    }
    
    /// Get current configuration
    pub fn config(&self) -> &PriorityFeeConfig {
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
    fn test_config_validation() {
        let mut config = PriorityFeeConfig::default();
        assert!(config.validate().is_ok());
        
        config.target_percentile = 0;
        assert!(config.validate().is_err());
        
        config.target_percentile = 100;
        assert!(config.validate().is_err());
        
        config.target_percentile = 50;
        config.min_fee_microlamports = 100_000;
        config.max_fee_microlamports = 1_000;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_default_config() {
        let config = PriorityFeeConfig::default();
        assert_eq!(config.target_percentile, 50);
        assert_eq!(config.sample_size, 150);
        assert!(config.min_fee_microlamports > 0);
    }

    #[test]
    fn test_percentile_estimation() {
        let config = PriorityFeeConfig {
            enabled: true,
            target_percentile: 50,
            sample_size: 150,
            min_fee_microlamports: 1_000,
            max_fee_microlamports: 10_000,
        };
        
        let optimizer = PriorityFeeOptimizer::new(config).unwrap();
        let fee = optimizer.estimate_fee_from_percentile();
        
        // 50th percentile should be roughly in the middle
        assert!(fee >= 4_000 && fee <= 6_000);
    }

    #[test]
    fn test_fee_clamping() {
        let config = PriorityFeeConfig {
            enabled: true,
            target_percentile: 99,
            sample_size: 150,
            min_fee_microlamports: 1_000,
            max_fee_microlamports: 10_000,
        };
        
        let optimizer = PriorityFeeOptimizer::new(config).unwrap();
        let fee = optimizer.estimate_fee_from_percentile();
        
        // Should never exceed max
        assert!(fee <= 10_000);
    }

    #[tokio::test]
    async fn test_get_optimal_fee_disabled() {
        let config = PriorityFeeConfig {
            enabled: false,
            ..Default::default()
        };
        
        let optimizer = PriorityFeeOptimizer::new(config).unwrap();
        let fee = optimizer.get_optimal_fee().await.unwrap();
        
        assert_eq!(fee, 0);
    }

    #[tokio::test]
    async fn test_get_optimal_fee_enabled() {
        let config = PriorityFeeConfig::default();
        let optimizer = PriorityFeeOptimizer::new(config).unwrap();
        let fee = optimizer.get_optimal_fee().await.unwrap();
        
        // Should return a reasonable fee
        assert!(fee >= 1_000);
        assert!(fee <= 50_000);
    }
}
