//! Execution configuration — trade sizes, slippage, retries, Jito, fee filter.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use log::warn;
use super::{
    default_max_trade_sol, default_max_trade_size_usdc,
    default_priority_fee_microlamports, default_slippage_bps,
    default_confirm_timeout_secs, default_max_tx_retries,
    default_max_requote_attempts,
    default_true,
    default_min_fee_threshold_bps, default_max_fee_threshold_bps,
    default_fee_filter_window_secs,
};

// ─────────────────────────────────────────────────────────────────────────────
// FeeFilterConfig (V5.4 PR #94 Commit 3)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeeFilterConfig {
    #[serde(default = "default_true")]
    pub enable_smart_fee_filter: bool,
    #[serde(default = "default_min_fee_threshold_bps")]
    pub min_fee_threshold_bps: u32,
    #[serde(default = "default_max_fee_threshold_bps")]
    pub max_fee_threshold_bps: u32,
    #[serde(default = "default_fee_filter_window_secs")]
    pub fee_filter_window_secs: u64,
}

impl Default for FeeFilterConfig {
    fn default() -> Self {
        Self {
            enable_smart_fee_filter: true,
            min_fee_threshold_bps:   default_min_fee_threshold_bps(),
            max_fee_threshold_bps:   default_max_fee_threshold_bps(),
            fee_filter_window_secs:  default_fee_filter_window_secs(),
        }
    }
}

impl FeeFilterConfig {
    pub fn validate(&self) -> Result<()> {
        if self.enable_smart_fee_filter {
            if self.min_fee_threshold_bps == 0 {
                bail!("trading.fee_filter.min_fee_threshold_bps must be > 0");
            }
            if self.min_fee_threshold_bps >= self.max_fee_threshold_bps {
                bail!(
                    "trading.fee_filter.min_fee_threshold_bps ({}) must be < \
                     max_fee_threshold_bps ({})",
                    self.min_fee_threshold_bps, self.max_fee_threshold_bps
                );
            }
            if self.max_fee_threshold_bps > 500 {
                warn!(
                    "⚠️ fee_filter.max_fee_threshold_bps ({}) > 500 BPS (5%) — \
                     very permissive fee ceiling",
                    self.max_fee_threshold_bps
                );
            }
            if self.fee_filter_window_secs < 5 {
                warn!(
                    "⚠️ fee_filter.fee_filter_window_secs ({}) < 5s — \
                     fee average may be too noisy",
                    self.fee_filter_window_secs
                );
            }
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ExecutionConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionConfig {
    #[serde(default = "default_max_trade_sol")]
    pub max_trade_sol: f64,
    #[serde(default = "default_max_trade_size_usdc")]
    pub max_trade_size_usdc: f64,
    #[serde(default = "default_priority_fee_microlamports")]
    pub priority_fee_microlamports: u64,
    #[serde(default = "default_slippage_bps")]
    pub max_slippage_bps: u16,
    #[serde(default)]
    pub jito_tip_lamports: Option<u64>,
    #[serde(default)]
    pub rpc_fallback_urls: Option<Vec<String>>,
    #[serde(default = "default_confirm_timeout_secs")]
    pub confirmation_timeout_secs: u64,
    #[serde(default = "default_max_tx_retries")]
    pub max_retries: u8,
    #[serde(default = "default_max_requote_attempts")]
    pub max_requote_attempts: u8,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_trade_sol:               default_max_trade_sol(),
            max_trade_size_usdc:         default_max_trade_size_usdc(),
            priority_fee_microlamports:  default_priority_fee_microlamports(),
            max_slippage_bps:            default_slippage_bps(),
            jito_tip_lamports:           None,
            rpc_fallback_urls:           None,
            confirmation_timeout_secs:   default_confirm_timeout_secs(),
            max_retries:                 default_max_tx_retries(),
            max_requote_attempts:        default_max_requote_attempts(),
        }
    }
}

impl ExecutionConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_trade_sol <= 0.0 {
            bail!("execution.max_trade_sol must be positive");
        }
        if self.max_trade_sol > 100.0 {
            warn!("⚠️ execution.max_trade_sol ({:.2}) is very large — double-check capital allocation",
                  self.max_trade_sol);
        }
        if self.max_slippage_bps == 0 {
            bail!("execution.max_slippage_bps cannot be 0 — Jupiter requires > 0 BPS");
        }
        if self.max_slippage_bps > 500 {
            warn!("⚠️ execution.max_slippage_bps ({}) > 5% — very high slippage tolerance!",
                  self.max_slippage_bps);
        }
        if self.confirmation_timeout_secs == 0 {
            bail!("execution.confirmation_timeout_secs must be > 0");
        }
        if self.max_retries == 0 {
            warn!("⚠️ execution.max_retries = 0 — failed txs will NOT be retried");
        }
        if self.max_requote_attempts == 0 {
            bail!("execution.max_requote_attempts must be > 0");
        }
        Ok(())
    }

    pub fn slippage_pct(&self) -> f64 { self.max_slippage_bps as f64 / 100.0 }
    pub fn jito_enabled(&self) -> bool { self.jito_tip_lamports.is_some() }
}
