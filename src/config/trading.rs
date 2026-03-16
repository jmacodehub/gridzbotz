//! Trading configuration — grid params, rebalance, regime gate, order lifecycle.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail, Context};
use log::{info, warn};
use super::{
    default_true,
    default_reposition_threshold, default_volatility_window,
    default_rebalance_threshold, default_cooldown,
    default_max_orders, default_refresh_interval,
    default_profit_threshold, default_slippage,
    default_lower_bound, default_upper_bound,
    default_order_max_age, default_lifecycle_check, default_min_orders,
    default_signal_size_multiplier, default_optimizer_interval_cycles,
    default_max_grid_spacing_pct, default_min_grid_spacing_pct,
    default_vol_floor_resume_pct,
};
use super::execution::FeeFilterConfig;

// ─────────────────────────────────────────────────────────────────────────────
// TradingConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TradingConfig {
    pub grid_levels: u32,
    pub grid_spacing_percent: f64,
    pub min_order_size: f64,
    pub max_position_size: f64,
    pub min_usdc_reserve: f64,
    pub min_sol_reserve: f64,
    #[serde(default)]
    pub enable_dynamic_grid: bool,
    #[serde(default = "default_reposition_threshold")]
    pub reposition_threshold: f64,
    #[serde(default = "default_volatility_window")]
    pub volatility_window: u32,
    #[serde(default = "default_true")]
    pub enable_auto_rebalance: bool,
    #[serde(default = "default_true")]
    pub enable_smart_rebalance: bool,
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold_pct: f64,
    #[serde(default = "default_cooldown")]
    pub rebalance_cooldown_secs: u64,
    #[serde(default = "default_max_orders")]
    pub max_orders_per_side: u32,
    #[serde(default = "default_refresh_interval")]
    pub order_refresh_interval_secs: u64,
    #[serde(default)]
    pub enable_market_orders: bool,
    #[serde(default = "default_true")]
    pub enable_fee_optimization: bool,
    #[serde(default = "default_profit_threshold")]
    pub min_profit_threshold_pct: f64,
    #[serde(default = "default_slippage")]
    pub max_slippage_pct: f64,
    #[serde(default)]
    pub enable_price_bounds: bool,
    #[serde(default = "default_lower_bound")]
    pub lower_price_bound: f64,
    #[serde(default = "default_upper_bound")]
    pub upper_price_bound: f64,
    #[serde(default = "default_true")]
    pub enable_regime_gate: bool,
    #[serde(default)]
    pub min_volatility_to_trade: f64,
    #[serde(default = "default_true")]
    pub pause_in_very_low_vol: bool,
    #[serde(default = "default_vol_floor_resume_pct")]
    pub vol_floor_resume_pct: f64,
    #[serde(default = "default_true")]
    pub enable_order_lifecycle: bool,
    #[serde(default = "default_order_max_age")]
    pub order_max_age_minutes: u64,
    #[serde(default = "default_lifecycle_check")]
    pub order_refresh_interval_minutes: u64,
    #[serde(default = "default_min_orders")]
    pub min_orders_to_maintain: usize,
    #[serde(default)]
    pub enable_adaptive_spacing: bool,
    #[serde(default)]
    pub enable_smart_position_sizing: bool,
    #[serde(default = "default_signal_size_multiplier")]
    pub signal_size_multiplier: f64,
    #[serde(default = "default_optimizer_interval_cycles")]
    pub optimizer_interval_cycles: u64,
    #[serde(default)]
    pub fee_filter: FeeFilterConfig,
    #[serde(default = "default_max_grid_spacing_pct")]
    pub max_grid_spacing_pct: f64,
    #[serde(default = "default_min_grid_spacing_pct")]
    pub min_grid_spacing_pct: f64,
}

impl TradingConfig {
    pub fn validate(&self) -> Result<()> {
        if self.grid_levels < 2 {
            bail!("grid_levels must be at least 2 (current: {})", self.grid_levels);
        }
        if self.grid_levels > 100 {
            warn!("⚠️ Very high grid_levels ({}) - may cause performance issues", self.grid_levels);
        }
        if self.grid_spacing_percent <= 0.0 {
            bail!("grid_spacing_percent must be positive (current: {})", self.grid_spacing_percent);
        }
        if self.grid_spacing_percent > 10.0 {
            warn!("⚠️ Very wide grid spacing ({:.2}%) - trades may be infrequent", self.grid_spacing_percent);
        }
        if self.grid_spacing_percent < 0.05 {
            warn!("⚠️ Very tight grid spacing ({:.2}%) - may not profit after fees", self.grid_spacing_percent);
        }
        if self.min_order_size <= 0.0 {
            bail!("min_order_size must be positive");
        }
        if self.max_position_size <= self.min_order_size {
            bail!("max_position_size must be > min_order_size");
        }
        if self.min_usdc_reserve < 0.0 {
            bail!("min_usdc_reserve cannot be negative");
        }
        if self.min_sol_reserve < 0.0 {
            bail!("min_sol_reserve cannot be negative");
        }
        if self.enable_regime_gate {
            if self.min_volatility_to_trade < 0.0 {
                bail!("min_volatility_to_trade cannot be negative");
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("⚠️ Very high min_volatility_to_trade ({:.2}%) - bot may rarely trade!",
                      self.min_volatility_to_trade);
            }
        }
        if self.enable_order_lifecycle {
            if self.order_max_age_minutes == 0 {
                bail!("order_max_age_minutes must be > 0");
            }
            if self.order_refresh_interval_minutes == 0 {
                bail!("order_refresh_interval_minutes must be > 0");
            }
            if self.order_refresh_interval_minutes > self.order_max_age_minutes {
                warn!("⚠️ refresh_interval > max_age - orders will never trigger refresh");
            }
            if self.min_orders_to_maintain < 2 {
                bail!("min_orders_to_maintain must be at least 2");
            }
        }
        if self.enable_price_bounds {
            if self.lower_price_bound >= self.upper_price_bound {
                bail!("lower_price_bound must be < upper_price_bound");
            }
            if self.lower_price_bound <= 0.0 {
                bail!("lower_price_bound must be positive");
            }
        }
        if self.optimizer_interval_cycles == 0 {
            bail!("trading.optimizer_interval_cycles must be > 0 (got 0)");
        }
        if self.enable_smart_position_sizing
            && !(0.5_f64..=3.0_f64).contains(&self.signal_size_multiplier)
        {
            bail!(
                "trading.signal_size_multiplier must be in [0.5, 3.0] when \
                 enable_smart_position_sizing=true (got {:.3})",
                self.signal_size_multiplier
            );
        }
        self.fee_filter.validate().context("trading.fee_filter validation failed")?;
        if self.enable_dynamic_grid {
            if self.min_grid_spacing_pct <= 0.0 {
                bail!("trading.min_grid_spacing_pct must be positive");
            }
            if self.min_grid_spacing_pct >= self.max_grid_spacing_pct {
                bail!(
                    "trading.min_grid_spacing_pct ({:.5}) must be < max_grid_spacing_pct ({:.5})",
                    self.min_grid_spacing_pct, self.max_grid_spacing_pct
                );
            }
        }
        Ok(())
    }

    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                info!("🧪 Testing environment: Relaxing safety constraints");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
                self.enable_price_bounds = false;
            }
            "development" => {
                info!("🔧 Development environment: Moderate safety");
                if self.min_volatility_to_trade > 0.5 {
                    info!("   Lowering min_volatility from {:.2}% to 0.3%",
                          self.min_volatility_to_trade);
                    self.min_volatility_to_trade = 0.3;
                }
            }
            "production" => {
                info!("🔒 Production environment: Enforcing safety");
                if !self.enable_regime_gate {
                    warn!("⚠️ Force-enabling regime gate for production!");
                    self.enable_regime_gate = true;
                }
                if self.min_volatility_to_trade < self.vol_floor_resume_pct {
                    warn!("⚠️ Raising min_volatility to {:.2}% for production safety",
                          self.vol_floor_resume_pct);
                    self.min_volatility_to_trade = self.vol_floor_resume_pct;
                }
                if !self.enable_order_lifecycle {
                    warn!("⚠️ Force-enabling order lifecycle for production!");
                    self.enable_order_lifecycle = true;
                }
            }
            _ => { warn!("⚠️ Unknown environment '{}' - using config as-is", environment); }
        }
    }
}

impl Default for TradingConfig {
    fn default() -> Self {
        Self {
            grid_levels:                    35,
            grid_spacing_percent:           0.15,
            min_order_size:                 0.1,
            max_position_size:              100.0,
            min_usdc_reserve:               300.0,
            min_sol_reserve:                2.0,
            enable_dynamic_grid:            false,
            reposition_threshold:           default_reposition_threshold(),
            volatility_window:              default_volatility_window(),
            enable_auto_rebalance:          true,
            enable_smart_rebalance:         true,
            rebalance_threshold_pct:        default_rebalance_threshold(),
            rebalance_cooldown_secs:        default_cooldown(),
            max_orders_per_side:            default_max_orders(),
            order_refresh_interval_secs:    default_refresh_interval(),
            enable_market_orders:           false,
            enable_fee_optimization:        true,
            min_profit_threshold_pct:       default_profit_threshold(),
            max_slippage_pct:               default_slippage(),
            enable_price_bounds:            false,
            lower_price_bound:              default_lower_bound(),
            upper_price_bound:              default_upper_bound(),
            enable_regime_gate:             false,
            min_volatility_to_trade:        0.0,
            pause_in_very_low_vol:          false,
            vol_floor_resume_pct:           default_vol_floor_resume_pct(),
            enable_order_lifecycle:         true,
            order_max_age_minutes:          default_order_max_age(),
            order_refresh_interval_minutes: default_lifecycle_check(),
            min_orders_to_maintain:         default_min_orders(),
            enable_adaptive_spacing:        false,
            enable_smart_position_sizing:   false,
            signal_size_multiplier:         default_signal_size_multiplier(),
            optimizer_interval_cycles:      default_optimizer_interval_cycles(),
            fee_filter:                     FeeFilterConfig::default(),
            max_grid_spacing_pct:           default_max_grid_spacing_pct(),
            min_grid_spacing_pct:           default_min_grid_spacing_pct(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RegimeGateConfig (Analytics Module Bridge)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RegimeGateConfig {
    pub enable_regime_gate: bool,
    pub volatility_threshold_bps: f64,
    pub trend_threshold: f64,
    pub min_volatility_to_trade_bps: f64,
    pub pause_in_very_low_vol: bool,
}

impl From<&TradingConfig> for RegimeGateConfig {
    fn from(trading: &TradingConfig) -> Self {
        let volatility_bps = trading.min_volatility_to_trade * 100.0;
        info!("🔧 Converting TradingConfig → RegimeGateConfig:");
        info!("   Min volatility: {:.2}% → {} BPS", trading.min_volatility_to_trade, volatility_bps);
        info!("   Regime gate: {}", if trading.enable_regime_gate { "ENABLED" } else { "DISABLED" });
        Self {
            enable_regime_gate:          trading.enable_regime_gate,
            volatility_threshold_bps:    volatility_bps,
            trend_threshold:             3.0,
            min_volatility_to_trade_bps: volatility_bps,
            pause_in_very_low_vol:       trading.pause_in_very_low_vol,
        }
    }
}

impl Default for RegimeGateConfig {
    fn default() -> Self {
        Self {
            enable_regime_gate:          true,
            volatility_threshold_bps:    2.0,
            trend_threshold:             3.0,
            min_volatility_to_trade_bps: 3.0,
            pause_in_very_low_vol:       true,
        }
    }
}
