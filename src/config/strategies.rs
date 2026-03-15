//! Strategies configuration — consensus mode, grid/momentum/RSI/MACD sub-configs.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use super::{
    default_confidence, default_grid_seed_bypass, default_wma_confidence_threshold,
    default_extreme_oversold, default_extreme_overbought,
    default_strong_threshold, default_normal_threshold,
    default_macd_min_confidence, default_macd_histogram_threshold, default_macd_warmup_periods,
};

// ─────────────────────────────────────────────────────────────────────────────
// StrategiesConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StrategiesConfig {
    pub active: Vec<String>,
    pub consensus_mode: String,
    pub grid: GridStrategyConfig,
    #[serde(default)]
    pub momentum: MomentumStrategyConfig,
    #[serde(default)]
    pub mean_reversion: MeanReversionStrategyConfig,
    #[serde(default)]
    pub rsi: RsiStrategyConfig,
    #[serde(default)]
    pub momentum_macd: MomentumMACDStrategyConfig,
    #[serde(default)]
    pub enable_multi_timeframe: bool,
    #[serde(default)]
    pub require_timeframe_alignment: bool,
    #[serde(default = "default_wma_confidence_threshold")]
    pub wma_confidence_threshold: f64,
}

impl StrategiesConfig {
    pub fn validate(&self) -> Result<()> {
        let valid_modes = ["single", "weighted", "majority", "unanimous"];
        if !valid_modes.contains(&self.consensus_mode.as_str()) {
            bail!("Invalid consensus_mode '{}'. Must be one of: {:?}",
                  self.consensus_mode, valid_modes);
        }
        if self.active.is_empty() {
            bail!("At least one strategy must be active");
        }
        let mut total_weight = 0.0;
        if self.grid.enabled          { total_weight += self.grid.weight; }
        if self.momentum.enabled      { total_weight += self.momentum.weight; }
        if self.mean_reversion.enabled { total_weight += self.mean_reversion.weight; }
        if self.rsi.enabled           { total_weight += self.rsi.weight; }
        if self.momentum_macd.enabled  { total_weight += self.momentum_macd.weight; }
        if total_weight == 0.0 {
            bail!("No strategies are enabled");
        }
        if !(0.0_f64..=1.0_f64).contains(&self.wma_confidence_threshold) {
            bail!(
                "strategies.wma_confidence_threshold must be in [0.0, 1.0] (got {:.3})",
                self.wma_confidence_threshold
            );
        }
        Ok(())
    }
}

impl Default for StrategiesConfig {
    fn default() -> Self {
        Self {
            active: vec!["grid".to_string()],
            consensus_mode: "single".to_string(),
            grid: GridStrategyConfig {
                enabled: true,
                weight: 1.0,
                min_confidence: 0.5,
                seed_orders_bypass: default_grid_seed_bypass(),
            },
            momentum: MomentumStrategyConfig::default(),
            mean_reversion: MeanReversionStrategyConfig::default(),
            rsi: RsiStrategyConfig::default(),
            momentum_macd: MomentumMACDStrategyConfig::default(),
            enable_multi_timeframe: false,
            require_timeframe_alignment: false,
            wma_confidence_threshold: default_wma_confidence_threshold(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Strategy sub-configs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GridStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_confidence")]
    pub min_confidence: f64,
    #[serde(default = "default_grid_seed_bypass")]
    pub seed_orders_bypass: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentumStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub lookback_period: usize,
    pub threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeanReversionStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub sma_period: usize,
    pub std_dev_multiplier: f64,
    #[serde(default = "default_strong_threshold")]
    pub strong_buy_threshold: f64,
    #[serde(default = "default_normal_threshold")]
    pub buy_threshold: f64,
    #[serde(default = "default_strong_threshold")]
    pub strong_sell_threshold: f64,
    #[serde(default = "default_normal_threshold")]
    pub sell_threshold: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RsiStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    pub min_confidence: f64,
    pub period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    #[serde(default = "default_extreme_oversold")]
    pub extreme_oversold: f64,
    #[serde(default = "default_extreme_overbought")]
    pub extreme_overbought: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentumMACDStrategyConfig {
    pub enabled: bool,
    pub weight: f64,
    #[serde(default = "default_macd_min_confidence")]
    pub min_confidence: f64,
    #[serde(default = "default_macd_histogram_threshold")]
    pub strong_histogram_threshold: f64,
    #[serde(default = "default_macd_warmup_periods")]
    pub min_warmup_periods: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Param structs (passed into strategy engines)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct RsiParams {
    pub rsi_period: usize,
    pub oversold_threshold: f64,
    pub overbought_threshold: f64,
    pub extreme_oversold: f64,
    pub extreme_overbought: f64,
}

#[derive(Debug, Clone)]
pub struct MeanReversionParams {
    pub mean_period: usize,
    pub strong_buy_threshold: f64,
    pub buy_threshold: f64,
    pub strong_sell_threshold: f64,
    pub sell_threshold: f64,
    pub min_confidence: f64,
}

#[derive(Debug, Clone)]
pub struct MomentumMACDParams {
    pub min_confidence: f64,
    pub strong_histogram_threshold: f64,
    pub min_warmup_periods: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// to_*_params() converters
// ─────────────────────────────────────────────────────────────────────────────

impl RsiStrategyConfig {
    pub fn to_rsi_params(&self) -> RsiParams {
        RsiParams {
            rsi_period:           self.period,
            oversold_threshold:   self.oversold_threshold,
            overbought_threshold: self.overbought_threshold,
            extreme_oversold:     self.extreme_oversold,
            extreme_overbought:   self.extreme_overbought,
        }
    }
}

impl MeanReversionStrategyConfig {
    pub fn to_mean_reversion_params(&self) -> MeanReversionParams {
        MeanReversionParams {
            mean_period:           self.sma_period,
            strong_buy_threshold:  self.strong_buy_threshold,
            buy_threshold:         self.buy_threshold,
            strong_sell_threshold: self.strong_sell_threshold,
            sell_threshold:        self.sell_threshold,
            min_confidence:        self.min_confidence,
        }
    }
}

impl MomentumMACDStrategyConfig {
    pub fn to_momentum_macd_params(&self) -> MomentumMACDParams {
        MomentumMACDParams {
            min_confidence:             self.min_confidence,
            strong_histogram_threshold: self.strong_histogram_threshold,
            min_warmup_periods:         self.min_warmup_periods,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Default impls
// ─────────────────────────────────────────────────────────────────────────────

impl Default for MomentumStrategyConfig {
    fn default() -> Self {
        Self { enabled: false, weight: 0.8, min_confidence: 0.6, lookback_period: 20, threshold: 0.02 }
    }
}

impl Default for MeanReversionStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false, weight: 0.7, min_confidence: 0.6, sma_period: 20, std_dev_multiplier: 2.0,
            strong_buy_threshold:  default_strong_threshold(),
            buy_threshold:         default_normal_threshold(),
            strong_sell_threshold: default_strong_threshold(),
            sell_threshold:        default_normal_threshold(),
        }
    }
}

impl Default for RsiStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false, weight: 0.9, min_confidence: 0.7, period: 14,
            oversold_threshold: 30.0, overbought_threshold: 70.0,
            extreme_oversold:   default_extreme_oversold(),
            extreme_overbought: default_extreme_overbought(),
        }
    }
}

impl Default for MomentumMACDStrategyConfig {
    fn default() -> Self {
        Self {
            enabled: false, weight: 0.8,
            min_confidence:             default_macd_min_confidence(),
            strong_histogram_threshold: default_macd_histogram_threshold(),
            min_warmup_periods:         default_macd_warmup_periods(),
        }
    }
}
