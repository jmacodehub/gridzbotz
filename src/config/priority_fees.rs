//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ PRIORITY FEE CONFIG V1.0 — Dynamic Compute-Unit Priority Fees
//!
//! Controls how Solana priority fees are calculated for transactions.
//! Supports dynamic estimation via getRecentPrioritizationFees RPC call
//! with configurable percentile, multiplier, and safety bounds.
//!
//! When `enable_dynamic = false` (default), falls back to the static
//! `execution.priority_fee_microlamports` value.
//!
//! March 2026 — V1.0 ⚡
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULTS
// ═══════════════════════════════════════════════════════════════════════════

fn default_strategy() -> String { "percentile".to_string() }
fn default_percentile() -> u8 { 50 }
fn default_multiplier() -> f64 { 1.2 }
fn default_min_microlamports() -> u64 { 1_000 }
fn default_max_microlamports() -> u64 { 500_000 }
fn default_fallback_microlamports() -> u64 { 5_000 }
fn default_cache_ttl_secs() -> u64 { 10 }
fn default_sample_blocks() -> u64 { 150 }

// ═══════════════════════════════════════════════════════════════════════════
// PRIORITY FEE CONFIG
// ═══════════════════════════════════════════════════════════════════════════

/// Dynamic priority fee configuration for Solana transactions.
///
/// Controls compute-unit priority fee estimation using recent block data
/// from `getRecentPrioritizationFees` RPC endpoint.
///
/// ## TOML Usage
///
/// ```toml
/// [priority_fees]
/// enable_dynamic = true
/// strategy = "percentile"        # "percentile" | "fixed"
/// percentile = 50                # P50 = median (range: 25-99)
/// multiplier = 1.2               # 20% headroom above computed fee
/// min_microlamports = 1000       # floor — never go below
/// max_microlamports = 500000     # ceiling — cap runaway costs
/// fallback_microlamports = 5000  # used when RPC estimation fails
/// cache_ttl_secs = 10            # re-estimate every 10s
/// sample_blocks = 150            # sample last 150 blocks
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorityFeeConfig {
    /// Enable dynamic priority fee estimation.
    /// When false, uses `execution.priority_fee_microlamports` (static).
    #[serde(default)]
    pub enable_dynamic: bool,

    /// Fee estimation strategy:
    /// - `"percentile"`: Nth percentile of recent fees (recommended)
    /// - `"fixed"`: Always use `fallback_microlamports`
    #[serde(default = "default_strategy")]
    pub strategy: String,

    /// Percentile of recent fees to target (25–99).
    /// P50 = median (default), P75 = aggressive, P90 = very aggressive.
    #[serde(default = "default_percentile")]
    pub percentile: u8,

    /// Multiplier applied to computed fee for headroom.
    /// 1.0 = exact percentile, 1.2 = 20% above (default).
    #[serde(default = "default_multiplier")]
    pub multiplier: f64,

    /// Minimum priority fee floor (microlamports).
    /// Ensures transactions always carry some priority.
    #[serde(default = "default_min_microlamports")]
    pub min_microlamports: u64,

    /// Maximum priority fee ceiling (microlamports).
    /// Safety cap to prevent runaway costs during extreme congestion.
    #[serde(default = "default_max_microlamports")]
    pub max_microlamports: u64,

    /// Fallback fee when RPC estimation fails (microlamports).
    /// Default: 5000 (matches current static priority_fee_microlamports).
    #[serde(default = "default_fallback_microlamports")]
    pub fallback_microlamports: u64,

    /// How long to cache the computed priority fee (seconds).
    /// Avoids hammering the RPC on every transaction.
    #[serde(default = "default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,

    /// Number of recent blocks to sample for fee estimation.
    /// Solana RPC supports up to 150 blocks for getRecentPrioritizationFees.
    #[serde(default = "default_sample_blocks")]
    pub sample_blocks: u64,
}

impl Default for PriorityFeeConfig {
    fn default() -> Self {
        Self {
            enable_dynamic: false,
            strategy: default_strategy(),
            percentile: default_percentile(),
            multiplier: default_multiplier(),
            min_microlamports: default_min_microlamports(),
            max_microlamports: default_max_microlamports(),
            fallback_microlamports: default_fallback_microlamports(),
            cache_ttl_secs: default_cache_ttl_secs(),
            sample_blocks: default_sample_blocks(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

impl PriorityFeeConfig {
    /// Validate priority fee configuration at startup.
    /// Only validates dynamic-specific fields when `enable_dynamic = true`.
    pub fn validate(&self) -> Result<()> {
        if !self.enable_dynamic {
            return Ok(());
        }

        match self.strategy.as_str() {
            "percentile" | "fixed" => {}
            other => bail!(
                "priority_fees.strategy must be 'percentile' or 'fixed', got '{}'", other
            ),
        }

        if self.percentile < 25 || self.percentile > 99 {
            bail!("priority_fees.percentile must be 25-99, got {}", self.percentile);
        }

        if self.multiplier < 0.1 || self.multiplier > 10.0 {
            bail!("priority_fees.multiplier must be 0.1-10.0, got {}", self.multiplier);
        }

        if self.min_microlamports >= self.max_microlamports {
            bail!(
                "priority_fees.min_microlamports ({}) must be < max_microlamports ({})",
                self.min_microlamports, self.max_microlamports
            );
        }

        if self.cache_ttl_secs == 0 {
            bail!("priority_fees.cache_ttl_secs must be > 0");
        }

        if self.sample_blocks == 0 || self.sample_blocks > 300 {
            bail!("priority_fees.sample_blocks must be 1-300, got {}", self.sample_blocks);
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let cfg = PriorityFeeConfig::default();
        assert!(!cfg.enable_dynamic);
        assert_eq!(cfg.strategy, "percentile");
        assert_eq!(cfg.percentile, 50);
        assert!((cfg.multiplier - 1.2).abs() < f64::EPSILON);
        assert_eq!(cfg.min_microlamports, 1_000);
        assert_eq!(cfg.max_microlamports, 500_000);
        assert_eq!(cfg.fallback_microlamports, 5_000);
        assert_eq!(cfg.cache_ttl_secs, 10);
        assert_eq!(cfg.sample_blocks, 150);
    }

    #[test]
    fn test_validate_disabled_skips_checks() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.percentile = 0; // invalid but should pass when disabled
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_valid_dynamic() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_validate_bad_strategy() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        cfg.strategy = "yolo".to_string();
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("strategy"));
    }

    #[test]
    fn test_validate_percentile_too_low() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        cfg.percentile = 10;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("percentile"));
    }

    #[test]
    fn test_validate_percentile_too_high() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        cfg.percentile = 100;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("percentile"));
    }

    #[test]
    fn test_validate_min_exceeds_max() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        cfg.min_microlamports = 600_000;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("min_microlamports"));
    }

    #[test]
    fn test_validate_zero_cache_ttl() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;
        cfg.cache_ttl_secs = 0;
        let err = cfg.validate().unwrap_err();
        assert!(err.to_string().contains("cache_ttl_secs"));
    }

    #[test]
    fn test_validate_sample_blocks_bounds() {
        let mut cfg = PriorityFeeConfig::default();
        cfg.enable_dynamic = true;

        cfg.sample_blocks = 0;
        assert!(cfg.validate().is_err());

        cfg.sample_blocks = 301;
        assert!(cfg.validate().is_err());

        cfg.sample_blocks = 150;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_serde_round_trip() {
        let original = PriorityFeeConfig::default();
        let toml_str = toml::to_string(&original).expect("serialize");
        let restored: PriorityFeeConfig = toml::from_str(&toml_str).expect("deserialize");
        assert_eq!(original.percentile, restored.percentile);
        assert_eq!(original.max_microlamports, restored.max_microlamports);
    }

    #[test]
    fn test_serde_empty_uses_defaults() {
        let cfg: PriorityFeeConfig = toml::from_str("").expect("empty should use defaults");
        assert!(!cfg.enable_dynamic);
        assert_eq!(cfg.strategy, "percentile");
        assert_eq!(cfg.fallback_microlamports, 5_000);
    }

    #[test]
    fn test_serde_partial_override() {
        let toml_str = r#"
enable_dynamic = true
percentile = 75
"#;
        let cfg: PriorityFeeConfig = toml::from_str(toml_str).expect("partial override");
        assert!(cfg.enable_dynamic);
        assert_eq!(cfg.percentile, 75);
        assert_eq!(cfg.multiplier, 1.2); // default preserved
    }
}
