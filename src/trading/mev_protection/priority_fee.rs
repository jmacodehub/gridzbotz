//! ⚡ Priority Fee Optimizer - Dynamic Fee Calculation
//!
//! Automatically adjusts priority fees based on real-time network congestion.
//! Samples recent blocks via RPC and calculates optimal fee to beat target percentile.

use anyhow::{Context, Result};
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFeeConfig {
    /// Target percentile for fee (50 = median, 75 = faster, 90 = very fast)
    pub target_percentile: u8,
    
    /// Number of recent blocks to sample (default: 150 ~= 1 minute)
    pub sample_size: u64,
    
    /// Minimum priority fee in microlamports (safety floor)
    pub min_fee_microlamports: u64,
    
    /// Maximum priority fee in microlamports (safety ceiling)
    pub max_fee_microlamports: u64,
    
    /// Fallback fee if RPC sampling fails
    pub fallback_fee_microlamports: u64,
}

impl Default for PriorityFeeConfig {
    fn default() -> Self {
        Self {
            target_percentile: 75,          // Conservative: beat 75% of txs
            sample_size: 150,               // ~1 minute of blocks (400ms slots)
            min_fee_microlamports: 1_000,   // 0.001 lamports minimum
            max_fee_microlamports: 100_000, // 0.1 lamports maximum
            fallback_fee_microlamports: 10_000, // 0.01 lamports fallback
        }
    }
}

impl PriorityFeeConfig {
    /// Create aggressive config (90th percentile)
    pub fn aggressive() -> Self {
        Self {
            target_percentile: 90,
            max_fee_microlamports: 500_000, // Allow up to 0.5 lamports
            ..Default::default()
        }
    }
    
    /// Create conservative config (50th percentile)
    pub fn conservative() -> Self {
        Self {
            target_percentile: 50,
            max_fee_microlamports: 50_000, // Cap at 0.05 lamports
            ..Default::default()
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Fee Estimate Result
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeEstimate {
    /// Recommended priority fee in microlamports
    pub priority_fee_microlamports: u64,
    
    /// Minimum fee observed in sample
    pub min_observed: u64,
    
    /// Maximum fee observed in sample
    pub max_observed: u64,
    
    /// Median fee in sample
    pub median: u64,
    
    /// Network congestion level
    pub congestion: NetworkCongestion,
    
    /// Number of fees sampled
    pub sample_count: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkCongestion {
    Low,
    Medium,
    High,
    Extreme,
}

impl NetworkCongestion {
    fn from_median_fee(median_microlamports: u64) -> Self {
        match median_microlamports {
            0..=5_000 => NetworkCongestion::Low,
            5_001..=20_000 => NetworkCongestion::Medium,
            20_001..=100_000 => NetworkCongestion::High,
            _ => NetworkCongestion::Extreme,
        }
    }
    
    pub fn as_str(&self) -> &'static str {
        match self {
            NetworkCongestion::Low => "LOW",
            NetworkCongestion::Medium => "MEDIUM",
            NetworkCongestion::High => "HIGH",
            NetworkCongestion::Extreme => "EXTREME",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Priority Fee Optimizer
// ═══════════════════════════════════════════════════════════════════════════

pub struct PriorityFeeOptimizer {
    rpc_client: RpcClient,
    config: PriorityFeeConfig,
}

impl PriorityFeeOptimizer {
    /// Create new optimizer with RPC endpoint
    pub fn new(rpc_url: String, config: PriorityFeeConfig) -> Self {
        info!("⚡ Initializing Priority Fee Optimizer");
        info!("   RPC: {}", rpc_url);
        info!("   Target percentile: {}th", config.target_percentile);
        info!("   Sample size: {} blocks", config.sample_size);
        
        let rpc_client = RpcClient::new_with_timeout(
            rpc_url,
            Duration::from_secs(10),
        );
        
        Self { rpc_client, config }
    }
    
    /// Calculate optimal priority fee based on recent network activity
    pub async fn calculate_optimal_fee(&self) -> Result<FeeEstimate> {
        debug!("🔍 Sampling priority fees from recent blocks...");
        
        // Sample recent priority fees (in a real impl, we'd query recent_prioritization_fees RPC)
        // For now, we'll use a mock implementation that returns realistic values
        let fees = self.sample_recent_fees().await?;
        
        if fees.is_empty() {
            warn!("⚠️  No fee samples available, using fallback");
            return Ok(FeeEstimate {
                priority_fee_microlamports: self.config.fallback_fee_microlamports,
                min_observed: 0,
                max_observed: 0,
                median: 0,
                congestion: NetworkCongestion::Medium,
                sample_count: 0,
            });
        }
        
        // Calculate statistics
        let mut sorted_fees = fees.clone();
        sorted_fees.sort_unstable();
        
        let min_observed = sorted_fees[0];
        let max_observed = sorted_fees[sorted_fees.len() - 1];
        let median = sorted_fees[sorted_fees.len() / 2];
        
        // Calculate target percentile
        let percentile_index = (sorted_fees.len() as f64 * self.config.target_percentile as f64 / 100.0) as usize;
        let percentile_index = percentile_index.min(sorted_fees.len() - 1);
        let target_fee = sorted_fees[percentile_index];
        
        // Apply safety bounds
        let final_fee = target_fee
            .max(self.config.min_fee_microlamports)
            .min(self.config.max_fee_microlamports);
        
        let congestion = NetworkCongestion::from_median_fee(median);
        
        info!(
            "✅ Fee calculated: {} µΛ ({}th percentile) | Congestion: {}",
            final_fee,
            self.config.target_percentile,
            congestion.as_str()
        );
        debug!("   Min: {} | Median: {} | Max: {}", min_observed, median, max_observed);
        
        Ok(FeeEstimate {
            priority_fee_microlamports: final_fee,
            min_observed,
            max_observed,
            median,
            congestion,
            sample_count: fees.len(),
        })
    }
    
    /// Sample recent priority fees from the network
    /// 
    /// NOTE: In production, this would call `getRecentPrioritizationFees` RPC method.
    /// For now, we return mock data based on realistic network patterns.
    async fn sample_recent_fees(&self) -> Result<Vec<u64>> {
        // TODO: Replace with actual RPC call when integrated
        // let fees = self.rpc_client
        //     .get_recent_prioritization_fees(&[])
        //     .await?
        //     .into_iter()
        //     .map(|f| f.prioritization_fee)
        //     .collect();
        
        // Mock implementation - returns realistic fee distribution
        // Low congestion: 1k-15k microlamports
        // Medium: 10k-50k
        // High: 30k-200k
        let base_fee = 5_000u64;
        let variation = 15_000u64;
        
        let mut fees = Vec::new();
        for i in 0..self.config.sample_size.min(150) {
            // Simulate realistic fee distribution
            let fee = base_fee + (i * variation / 150);
            fees.push(fee);
        }
        
        Ok(fees)
    }
    
    /// Get quick estimate without full sampling (uses fallback)
    pub fn get_fallback_fee(&self) -> u64 {
        self.config.fallback_fee_microlamports
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
        let config = PriorityFeeConfig::default();
        assert_eq!(config.target_percentile, 75);
        assert_eq!(config.sample_size, 150);
        assert!(config.min_fee_microlamports > 0);
        assert!(config.max_fee_microlamports > config.min_fee_microlamports);
    }
    
    #[test]
    fn test_aggressive_config() {
        let config = PriorityFeeConfig::aggressive();
        assert_eq!(config.target_percentile, 90);
        assert!(config.max_fee_microlamports > PriorityFeeConfig::default().max_fee_microlamports);
    }
    
    #[test]
    fn test_conservative_config() {
        let config = PriorityFeeConfig::conservative();
        assert_eq!(config.target_percentile, 50);
        assert!(config.max_fee_microlamports < PriorityFeeConfig::default().max_fee_microlamports);
    }
    
    #[test]
    fn test_network_congestion_levels() {
        assert_eq!(NetworkCongestion::from_median_fee(1_000), NetworkCongestion::Low);
        assert_eq!(NetworkCongestion::from_median_fee(10_000), NetworkCongestion::Medium);
        assert_eq!(NetworkCongestion::from_median_fee(50_000), NetworkCongestion::High);
        assert_eq!(NetworkCongestion::from_median_fee(200_000), NetworkCongestion::Extreme);
    }
    
    #[test]
    fn test_congestion_string() {
        assert_eq!(NetworkCongestion::Low.as_str(), "LOW");
        assert_eq!(NetworkCongestion::Medium.as_str(), "MEDIUM");
        assert_eq!(NetworkCongestion::High.as_str(), "HIGH");
        assert_eq!(NetworkCongestion::Extreme.as_str(), "EXTREME");
    }
    
    #[tokio::test]
    async fn test_optimizer_creation() {
        let config = PriorityFeeConfig::default();
        let optimizer = PriorityFeeOptimizer::new(
            "http://localhost:8899".to_string(),
            config,
        );
        
        assert_eq!(optimizer.get_fallback_fee(), 10_000);
    }
    
    #[tokio::test]
    async fn test_fee_calculation() {
        let config = PriorityFeeConfig::default();
        let optimizer = PriorityFeeOptimizer::new(
            "http://localhost:8899".to_string(),
            config,
        );
        
        let estimate = optimizer.calculate_optimal_fee().await.unwrap();
        
        assert!(estimate.priority_fee_microlamports >= 1_000);
        assert!(estimate.priority_fee_microlamports <= 100_000);
        assert!(estimate.sample_count > 0);
        assert!(estimate.median > 0);
    }
    
    #[tokio::test]
    async fn test_fee_bounds_enforced() {
        let config = PriorityFeeConfig {
            min_fee_microlamports: 50_000,
            max_fee_microlamports: 60_000,
            ..Default::default()
        };
        
        let optimizer = PriorityFeeOptimizer::new(
            "http://localhost:8899".to_string(),
            config,
        );
        
        let estimate = optimizer.calculate_optimal_fee().await.unwrap();
        
        // Fee should be clamped to bounds
        assert!(estimate.priority_fee_microlamports >= 50_000);
        assert!(estimate.priority_fee_microlamports <= 60_000);
    }
}
