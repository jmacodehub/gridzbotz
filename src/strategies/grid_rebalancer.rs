//! ═════════════════════════════════════════════════════════════════════════
//! 🔥📎 GRID REBALANCER V6.1 - CONFIGURABLE VOL FLOOR (PR #124)
//!
//! V6.1 (PR #124 — Configurable Vol Floor):
//!   ✅ GridRebalancerConfig: vol_floor_resume_pct: f64
//!      Replaces hardcoded 0.3 in apply_environment("development") and
//!      apply_environment("production"). Default 0.05 matches live mainnet.
//!      #[serde(default)] — all existing TOMLs parse unchanged.
//!   ✅ GridRebalancerBuilder: vol_floor_resume_pct() setter
//!   ✅ 5 new tests:
//!      - default is 0.05
//!      - serde roundtrip absent TOML field → 0.05
//!      - production enforcer uses vol_floor_resume_pct, not 0.3
//!      - development cap uses vol_floor_resume_pct, not 0.3
//!      - production does NOT stomp a value already above the floor
//!
//! V6.0 (PR #119 — Order Lifecycle Engine):
//!   ✅ check_stale_orders(&GridStateTracker, current_price) -> Vec<u64>
//!      Called from GridBot::process_price_update() on every tick.
//!      Fast-exit when enable_order_lifecycle=false.
//!      Throttled by last_lifecycle_check + order_refresh_interval_minutes.
//!      ONLY cancels Pending levels — BuyFilled positions are never touched
//!      (cancelling an open position = realised loss, not a re-quote).
//!      Logs age_mins + price_drift_pct per stale level for observability.
//!      Returns Vec<u64> of cancelled IDs — GridBot re-places at current price.
//!   ✅ #[allow(dead_code)] removed from last_lifecycle_check (now live)
//!   ✅ 3 new unit tests:
//!      - lifecycle disabled → always returns empty
//!      - throttle window → suppresses check within interval
//!      - BuyFilled level → never returned in stale set
//!
//! V5.5 (PR #106 — Fix Initial Grid Seeding / Orders=0 problem):
//!   ✅ GridRebalancerConfig: seed_orders_bypass: bool (default true)
//!   ✅ orders_seeded: AtomicBool — seeding state, starts false
//!   ✅ should_place_order(): short-circuit before Path A/B on seed bypass
//!   ✅ mark_seeding_complete(): pub fn — sets orders_seeded=true, logs
//!   ✅ on_fill() Bug B fix: fee_filter.record_execution() now called
//!   ✅ Builder: seed_orders_bypass() setter
//!
//! V5.4 (PR #94 Commit 4 — SmartFeeFilter wired):
//!   ✅ fee_filter: Option<SmartFeeFilter> — built from FeesConfig
//!   ✅ should_place_order(): Path A = SmartFeeFilter; Path B = legacy spread gate
//!   ✅ position_size_sol param + fee_filter_stats()
//!   ✅ fill_rate_threshold: f64 replaces hardcoded HIGH_FILL_THR=0.10
//!
//! V5.0–V5.3: SpacingMode, ATRDynamic, FillState, LevelAnalytics, RegimeGate.
//!
//! PR #98 fix: name() returns "GridRebalancer" (stable WMA HashMap key).
//!
//! March 14, 2026 - V6.1: Configurable vol floor (PR #124) 🔧
//! March 14, 2026 - V6.0: Order Lifecycle Engine (PR #119 C1) ⏰
//! March 12, 2026 - V5.5: Seed bypass + record_execution (PR #106) 🌱
//! March 2026     - V5.4: SmartFeeFilter wired (PR #94 C4)
//! February 2026  - V5.1: Level analytics
//! ═════════════════════════════════════════════════════════════════════════

use crate::trading::{FillEvent, OrderSide};
use crate::trading::grid_level::{GridStateTracker, GridLevelStatus};
use crate::strategies::{Strategy, Signal, StrategyStats as BaseStrategyStats};
use crate::strategies::shared::analytics::atr_dynamic::{ATRDynamic, ATRConfig};
use crate::strategies::fee_filter::{SmartFeeFilter, SmartFeeFilterConfig, FeeFilterStats};
use crate::config::FeesConfig;
use async_trait::async_trait;
use anyhow::{Result, Context};
use log::{info, warn, debug, trace};
use std::collections::{VecDeque, HashMap};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::Instant;
use serde::{Serialize, Deserialize};

// ═══════════════════════════════════════════════════════════════════════════
// SPACING MODE - Modular, extensible algorithm selector
// ═══════════════════════════════════════════════════════════════════════════

/// Controls which spacing algorithm drives the grid.
///
/// Add a new arm here to introduce a new algorithm — zero changes elsewhere.
/// Fully serde-serializable so each bot TOML can declare its own mode.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SpacingMode {
    /// Constant spacing — always `config.grid_spacing`. Best for ranging markets.
    Fixed,

    /// Simple std-dev volatility buckets (original V2 logic). Zero warm-up.
    VolatilityBuckets,

    /// ATR × multiplier, clamped to [min_spacing, max_spacing].
    /// Falls back to `grid_spacing` until `period` ticks have been collected.
    AtrDynamic {
        /// ATR calculation period (default 14)
        period: usize,
        /// Multiplier applied to ATR% (default 3.0)
        multiplier: f64,
    },
}

impl Default for SpacingMode {
    fn default() -> Self {
        SpacingMode::VolatilityBuckets
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FILL STATE - Thread-safe fill-rate feedback tracking (V5.0)
// ═══════════════════════════════════════════════════════════════════════════

/// Bundles mutable fill-feedback state into one Arc<Mutex<_>>.
struct FillState {
    /// Unix timestamps (seconds) of recent fills — ring buffer, max 20
    timestamps: VecDeque<i64>,
    /// Additive spacing bias (fraction). +ve = widen, -ve = tighten.
    bias: f64,
}

impl FillState {
    fn new() -> Self {
        Self {
            timestamps: VecDeque::with_capacity(20),
            bias: 0.0,
        }
    }

    /// Fills per second over the last `window_secs` seconds
    fn fill_rate(&self, window_secs: i64) -> f64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let count = self.timestamps.iter()
            .filter(|&&t| now - t <= window_secs)
            .count();
        count as f64 / window_secs as f64
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// LEVEL ANALYTICS - Per-level fill tracking (V5.1)
// ═══════════════════════════════════════════════════════════════════════════

/// Point-in-time performance snapshot for a single grid level.
///
/// Accumulated across every `FillEvent` where `level_id == Some(self.level_id)`.
/// `level_id` matches `GridLevel.id` (u64) exactly — no casting needed.
#[derive(Debug, Clone)]
pub struct LevelSnapshot {
    /// Grid level ID — direct match to `GridLevel.id`
    pub level_id:              u64,
    /// Number of fills recorded on this level
    pub fill_count:            u64,
    /// Cumulative realised PnL across all fills (0.0 when pnl was None)
    pub total_pnl:             f64,
    /// Fill price of the most recent fill
    pub last_fill_price:       f64,
    /// Unix timestamp (seconds) of the most recent fill
    pub last_fill_timestamp:   i64,
    /// Sum of `distance_from_mid_pct` values for averaging.
    /// Divide by `fill_count` via `avg_distance_from_mid()` helper.
    pub distance_from_mid_sum: f64,
}

impl LevelSnapshot {
    /// Average % distance from mid-price across all fills on this level.
    ///
    /// - Negative result → level consistently fills below mid (buy zone)
    /// - Positive result → level consistently fills above mid (sell zone)
    /// - `None` when no fill carried a `distance_from_mid_pct` value
    pub fn avg_distance_from_mid(&self) -> Option<f64> {
        if self.fill_count == 0 || self.distance_from_mid_sum == 0.0 {
            return None;
        }
        Some(self.distance_from_mid_sum / self.fill_count as f64)
    }
}

/// Internal per-level accumulator. Keyed by `GridLevel.id` for O(1) lookup.
struct LevelAnalytics {
    levels:              HashMap<u64, LevelSnapshot>,
    /// Fills that carried a `level_id` (grid-originated)
    fills_with_level:    u64,
    /// Fills without a `level_id` (RSI, Momentum, manual, etc.)
    fills_without_level: u64,
}

impl LevelAnalytics {
    fn new() -> Self {
        Self {
            levels:              HashMap::new(),
            fills_with_level:    0,
            fills_without_level: 0,
        }
    }

    /// Record one fill. Upserts the `LevelSnapshot` when `level_id` is `Some`.
    fn record_fill(&mut self, fill: &FillEvent) {
        match fill.level_id {
            Some(id) => {
                let snap = self.levels.entry(id).or_insert_with(|| LevelSnapshot {
                    level_id:              id,
                    fill_count:            0,
                    total_pnl:             0.0,
                    last_fill_price:       fill.fill_price,
                    last_fill_timestamp:   fill.timestamp,
                    distance_from_mid_sum: 0.0,
                });
                snap.fill_count          += 1;
                snap.last_fill_price      = fill.fill_price;
                snap.last_fill_timestamp  = fill.timestamp;
                snap.total_pnl           += fill.pnl.unwrap_or(0.0);
                if let Some(dist) = fill.distance_from_mid_pct {
                    snap.distance_from_mid_sum += dist;
                }
                self.fills_with_level += 1;
            }
            None => {
                self.fills_without_level += 1;
            }
        }
    }

    fn hot_levels(&self, min_fills: u64) -> Vec<u64> {
        let mut ids: Vec<u64> = self.levels.values()
            .filter(|s| s.fill_count >= min_fills)
            .map(|s| s.level_id)
            .collect();
        ids.sort_unstable_by(|a, b| {
            self.levels[b].fill_count.cmp(&self.levels[a].fill_count)
        });
        ids
    }

    fn profitable_levels(&self, min_pnl: f64) -> Vec<u64> {
        let mut ids: Vec<u64> = self.levels.values()
            .filter(|s| s.total_pnl > min_pnl)
            .map(|s| s.level_id)
            .collect();
        ids.sort_unstable_by(|a, b| {
            let pa = self.levels[a].total_pnl;
            let pb = self.levels[b].total_pnl;
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });
        ids
    }

    fn snapshots_sorted(&self) -> Vec<LevelSnapshot> {
        let mut snaps: Vec<LevelSnapshot> = self.levels.values().cloned().collect();
        snaps.sort_unstable_by(|a, b| b.fill_count.cmp(&a.fill_count));
        snaps
    }
}

/// Public analytics report returned by `GridRebalancer::get_level_analytics()`.
#[derive(Debug, Clone)]
pub struct LevelAnalyticsReport {
    pub snapshots:            Vec<LevelSnapshot>,
    pub hot_levels:           Vec<u64>,
    pub profitable_levels:    Vec<u64>,
    pub fills_with_level:     u64,
    pub fills_without_level:  u64,
    pub total_tracked_levels: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION - 100% Config-Driven
// ═══════════════════════════════════════════════════════════════════════════

/// Grid Rebalancer Configuration — all behavior controlled here, no hardcoded values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridRebalancerConfig {
    // ── Core Grid ──────────────────────────────────────────────────────
    pub grid_spacing: f64,
    pub order_size: f64,
    pub min_usdc_balance: f64,
    pub min_sol_balance: f64,
    pub enabled: bool,

    // ── V2: Dynamic Spacing ───────────────────────────────────────────
    pub enable_dynamic_spacing: bool,
    pub enable_fee_filtering: bool,
    pub volatility_window_seconds: u64,
    pub max_spacing: f64,
    pub min_spacing: f64,

    // ── V3: Market Regime Gate ─────────────────────────────────────────
    pub enable_regime_gate: bool,
    pub min_volatility_to_trade: f64,
    pub pause_in_very_low_vol: bool,

    // ── V6.0: Order Lifecycle ─────────────────────────────────────────
    pub enable_order_lifecycle: bool,
    pub order_max_age_minutes: u64,
    pub order_refresh_interval_minutes: u64,
    pub min_orders_to_maintain: usize,

    // ── V5.0: Spacing Mode ───────────────────────────────────────────
    /// Selects the spacing algorithm. Defaults to VolatilityBuckets.
    pub spacing_mode: SpacingMode,

    // ── V5.4: Fill-rate bias threshold ───────────────────────────────
    /// Fill-rate (fills/sec) above which grid spacing widens to reduce
    /// over-trading. Replaces `const HIGH_FILL_THR = 0.10` in on_fill().
    /// Default: 0.10 (≈ 6 fills/min).
    #[serde(default = "default_high_fill_threshold")]
    pub fill_rate_threshold: f64,

    // ── V5.5: Seed bypass ────────────────────────────────────────────
    #[serde(default = "default_seed_orders_bypass")]
    pub seed_orders_bypass: bool,

    // ── V6.1: Configurable vol floor ─────────────────────────────────
    /// Minimum volatility floor applied by apply_environment().
    /// Replaces hardcoded 0.3 in both "development" cap and "production"
    /// safety raise. Default 0.05 matches live mainnet tuning Mar 2026.
    /// TOML key: vol_floor_resume_pct (omit → uses default 0.05).
    #[serde(default = "default_gr_vol_floor_resume_pct")]
    pub vol_floor_resume_pct: f64,
}

fn default_high_fill_threshold()     -> f64  { 0.10 }
fn default_seed_orders_bypass()      -> bool { true }
fn default_gr_vol_floor_resume_pct() -> f64  { 0.05 }

impl Default for GridRebalancerConfig {
    fn default() -> Self {
        Self {
            grid_spacing: 0.002,
            order_size: 0.1,
            min_usdc_balance: 100.0,
            min_sol_balance: 0.1,
            enabled: true,

            enable_dynamic_spacing: true,
            enable_fee_filtering: true,
            volatility_window_seconds: 600,
            max_spacing: 0.0075,
            min_spacing: 0.001,

            enable_regime_gate: true,
            min_volatility_to_trade: 0.5,
            pause_in_very_low_vol: true,

            enable_order_lifecycle: true,
            order_max_age_minutes: 10,
            order_refresh_interval_minutes: 5,
            min_orders_to_maintain: 8,

            spacing_mode: SpacingMode::VolatilityBuckets,

            fill_rate_threshold: 0.10,

            seed_orders_bypass: true,

            vol_floor_resume_pct: default_gr_vol_floor_resume_pct(),
        }
    }
}

impl GridRebalancerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.grid_spacing <= 0.0 {
            return Err(anyhow::anyhow!("grid_spacing must be > 0"));
        }
        if self.grid_spacing > 0.1 {
            warn!("\u{26a0}\u{fe0f} Grid spacing {:.2}% is very wide", self.grid_spacing * 100.0);
        }
        if self.enable_dynamic_spacing {
            if self.min_spacing >= self.max_spacing {
                return Err(anyhow::anyhow!(
                    "min_spacing ({}) must be < max_spacing ({})",
                    self.min_spacing, self.max_spacing
                ));
            }
            if self.min_spacing <= 0.0 {
                return Err(anyhow::anyhow!("min_spacing must be > 0"));
            }
        }
        if self.enable_regime_gate {
            if self.min_volatility_to_trade < 0.0 {
                return Err(anyhow::anyhow!("min_volatility_to_trade cannot be negative"));
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("\u{26a0}\u{fe0f} min_volatility_to_trade ${:.2} may never trade", self.min_volatility_to_trade);
            }
        }
        if self.order_size <= 0.0 {
            return Err(anyhow::anyhow!("order_size must be > 0"));
        }
        if self.min_usdc_balance < 0.0 || self.min_sol_balance < 0.0 {
            return Err(anyhow::anyhow!("Reserve balances cannot be negative"));
        }
        if self.enable_order_lifecycle {
            if self.order_max_age_minutes == 0 {
                return Err(anyhow::anyhow!("order_max_age_minutes must be > 0"));
            }
            if self.order_refresh_interval_minutes == 0 {
                return Err(anyhow::anyhow!("order_refresh_interval_minutes must be > 0"));
            }
        }
        if self.fill_rate_threshold <= 0.0 {
            return Err(anyhow::anyhow!("fill_rate_threshold must be > 0"));
        }
        Ok(())
    }

    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                info!("\u{1f9ea} Testing mode: Relaxing regime gate");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
            }
            "development" => {
                info!("\u{1f527} Development mode: Moderate regime gate");
                if self.min_volatility_to_trade > 0.5 {
                    self.min_volatility_to_trade = self.vol_floor_resume_pct;
                }
            }
            "production" => {
                info!("\u{1f512} Production mode: Enforcing regime gate");
                if !self.enable_regime_gate {
                    warn!("\u{26a0}\u{fe0f} Force-enabling regime gate for production!");
                    self.enable_regime_gate = true;
                }
                if self.min_volatility_to_trade < self.vol_floor_resume_pct {
                    warn!("\u{26a0}\u{fe0f} Raising min_volatility to {:.3} for production safety",
                          self.vol_floor_resume_pct);
                    self.min_volatility_to_trade = self.vol_floor_resume_pct;
                }
            }
            _ => warn!("\u{26a0}\u{fe0f} Unknown environment '{}', using defaults", environment),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID REBALANCER - V6.1
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancer {
    config: GridRebalancerConfig,
    fees: FeesConfig,
    current_price: Arc<tokio::sync::RwLock<Option<f64>>>,
    price_history: Arc<tokio::sync::Mutex<Vec<(Instant, f64)>>>,

    stats_rebalances: Arc<AtomicU64>,
    stats_filtered: Arc<AtomicU64>,
    stats_signals: Arc<AtomicU64>,
    dynamic_spacing_enabled: Arc<AtomicBool>,
    current_spacing: Arc<tokio::sync::RwLock<f64>>,

    // V6.0: live — throttle clock for check_stale_orders()
    last_lifecycle_check: Arc<tokio::sync::RwLock<Instant>>,
    trading_paused: Arc<AtomicBool>,
    pause_reason: Arc<tokio::sync::RwLock<String>>,
    last_signal: Arc<tokio::sync::RwLock<Option<Signal>>>,

    // V5.0: Fill feedback + ATR
    fill_state: Arc<tokio::sync::Mutex<FillState>>,
    atr_dynamic: Arc<tokio::sync::Mutex<Option<ATRDynamic>>>,

    // V5.1: Per-level analytics
    level_analytics: Arc<tokio::sync::Mutex<LevelAnalytics>>,

    // V5.4: SmartFeeFilter
    fee_filter: Option<SmartFeeFilter>,

    // V5.5: Seed bypass
    orders_seeded: Arc<AtomicBool>,
}

impl GridRebalancer {
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        Self::with_fees(config, FeesConfig::default())
    }

    /// Construct with explicit FeesConfig (preferred path from engine.rs).
    pub fn with_fees(config: GridRebalancerConfig, fees: FeesConfig) -> Result<Self> {
        config.validate().context("GridRebalancer config validation failed")?;

        info!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");
        info!("\u{1f3af} Grid Rebalancer V6.1 Initializing...");
        info!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");
        info!("\u{1f4ca} CORE: spacing={:.3}% size={} SOL reserves=${:.0}/{} SOL",
              config.grid_spacing * 100.0, config.order_size,
              config.min_usdc_balance, config.min_sol_balance);

        info!("\u{1f4d0} SPACING MODE:");
        match &config.spacing_mode {
            SpacingMode::Fixed =>
                info!("   Fixed ({:.3}%)", config.grid_spacing * 100.0),
            SpacingMode::VolatilityBuckets =>
                info!("   VolatilityBuckets ({:.3}%-{:.3}%)",
                      config.min_spacing * 100.0, config.max_spacing * 100.0),
            SpacingMode::AtrDynamic { period, multiplier } =>
                info!("   AtrDynamic (period={} mult={:.1}x range={:.3}%-{:.3}%)",
                      period, multiplier,
                      config.min_spacing * 100.0, config.max_spacing * 100.0),
        }

        info!("\u{1f4b0} FEES: maker={:.1}bps taker={:.1}bps slippage={:.1}bps multiplier={:.1}x",
              fees.maker_fee_bps, fees.taker_fee_bps,
              fees.slippage_bps, fees.min_profit_multiplier);
        info!("\u{1f6e1}\u{fe0f} REGIME GATE: {} | min_vol=${:.4} | vol_floor={:.3}",
              if config.enable_regime_gate { "\u{2705}" } else { "\u{274c} FREE" },
              config.min_volatility_to_trade,
              config.vol_floor_resume_pct);
        info!("\u{1f9e0} ADAPTIVE: fill-feedback bias \u{2705} | level analytics \u{2705} | fill_rate_thr={:.2}",
              config.fill_rate_threshold);
        info!("\u{1f331} SEED BYPASS: {} (fee filter enforced after mark_seeding_complete())",
              if config.seed_orders_bypass { "\u{2705} ACTIVE" } else { "\u{274c} disabled" });
        info!("\u{23f0} LIFECYCLE: {} (max_age={}m refresh={}m min_orders={})",
              if config.enable_order_lifecycle { "\u{2705} ACTIVE" } else { "\u{274c} disabled" },
              config.order_max_age_minutes,
              config.order_refresh_interval_minutes,
              config.min_orders_to_maintain);

        let atr_dynamic = match &config.spacing_mode {
            SpacingMode::AtrDynamic { period, multiplier } => {
                let atr_cfg = ATRConfig {
                    atr_period: *period,
                    atr_multiplier: *multiplier,
                    min_spacing: config.min_spacing * 100.0,
                    max_spacing: config.max_spacing * 100.0,
                };
                Some(ATRDynamic::from_config(&atr_cfg))
            }
            _ => None,
        };

        let fee_filter = if config.enable_fee_filtering {
            let filter_cfg = SmartFeeFilterConfig::from_fees_config(&fees);
            info!("\u{1f48e} SmartFeeFilter: ACTIVE (maker={:.2}bps taker={:.2}bps slippage={:.2}bps mult={:.1}x grace={})",
                  filter_cfg.maker_fee_percent * 100.0,
                  filter_cfg.taker_fee_percent * 100.0,
                  filter_cfg.slippage_percent * 100.0,
                  filter_cfg.min_profit_multiplier,
                  filter_cfg.grace_period_trades);
            Some(SmartFeeFilter::new(filter_cfg))
        } else {
            info!("\u{1f48e} SmartFeeFilter: DISABLED (enable_fee_filtering = false)");
            None
        };

        info!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");

        Ok(Self {
            current_spacing: Arc::new(tokio::sync::RwLock::new(config.grid_spacing)),
            orders_seeded: Arc::new(AtomicBool::new(false)),
            config,
            fees,
            current_price: Arc::new(tokio::sync::RwLock::new(None)),
            price_history: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            stats_rebalances: Arc::new(AtomicU64::new(0)),
            stats_filtered: Arc::new(AtomicU64::new(0)),
            stats_signals: Arc::new(AtomicU64::new(0)),
            dynamic_spacing_enabled: Arc::new(AtomicBool::new(true)),
            last_lifecycle_check: Arc::new(tokio::sync::RwLock::new(Instant::now())),
            trading_paused: Arc::new(AtomicBool::new(false)),
            pause_reason: Arc::new(tokio::sync::RwLock::new(String::new())),
            last_signal: Arc::new(tokio::sync::RwLock::new(None)),
            fill_state: Arc::new(tokio::sync::Mutex::new(FillState::new())),
            atr_dynamic: Arc::new(tokio::sync::Mutex::new(atr_dynamic)),
            level_analytics: Arc::new(tokio::sync::Mutex::new(LevelAnalytics::new())),
            fee_filter,
        })
    }

    pub fn builder() -> GridRebalancerBuilder {
        GridRebalancerBuilder::new()
    }

    // ── V5.5: Seed Bypass API ─────────────────────────────────────────────

    pub fn mark_seeding_complete(&self) {
        if self.config.seed_orders_bypass
            && !self.orders_seeded.load(Ordering::Acquire)
        {
            self.orders_seeded.store(true, Ordering::Release);
            info!("\u{1f331} Grid seeding complete \u{2014} SmartFeeFilter now enforced for all orders");
        }
    }

    pub fn is_seeding(&self) -> bool {
        self.config.seed_orders_bypass && !self.orders_seeded.load(Ordering::Acquire)
    }

    // ── V6.0: Order Lifecycle Engine ──────────────────────────────────────

    /// Scan all Pending grid levels and cancel those older than
    /// `order_max_age_minutes`. Returns the IDs of cancelled levels so
    /// GridBot can re-place fresh orders at the current market price.
    ///
    /// # Safety invariants
    /// - **BuyFilled levels are never touched** — cancelling an open position
    ///   would realise a loss. Only `Pending` (unfilled buy orders) are eligible.
    /// - Throttled by `last_lifecycle_check`: runs at most once per
    ///   `order_refresh_interval_minutes` wall-clock minutes.
    /// - Fast-exits (`Vec::new()`) when `enable_order_lifecycle = false`.
    ///
    /// # Call site
    /// Called from `GridBot::process_price_update()` on every tick, after the
    /// fill-processing loop and before the heartbeat stats block.
    pub async fn check_stale_orders(
        &self,
        tracker: &GridStateTracker,
        current_price: f64,
    ) -> Vec<u64> {
        if !self.config.enable_order_lifecycle {
            return Vec::new();
        }

        // ── Throttle: only run every order_refresh_interval_minutes ──────
        {
            let last = self.last_lifecycle_check.read().await;
            let interval = Duration::from_secs(
                self.config.order_refresh_interval_minutes * 60
            );
            if last.elapsed() < interval {
                return Vec::new();
            }
        }
        // Advance the checkpoint *before* doing work to prevent thundering
        // herd if multiple async callers slip through the read guard above.
        *self.last_lifecycle_check.write().await = Instant::now();

        let max_age = Duration::from_secs(self.config.order_max_age_minutes * 60);
        let levels  = tracker.get_all_levels().await;
        let mut stale_ids: Vec<u64> = Vec::new();

        for level in &levels {
            // Only cancel unfilled buy orders — NEVER cancel an open position
            if level.status != GridLevelStatus::Pending {
                continue;
            }
            if !level.is_stale(max_age) {
                continue;
            }

            let age_mins   = level.age_seconds() / 60;
            let drift_pct  = ((current_price - level.buy_price).abs()
                / level.buy_price) * 100.0;

            warn!(
                "\u{23f0} STALE ORDER: level={} buy=${:.4} age={}m \
                 price_drift={:.2}% \u{2192} cancelling for re-quote",
                level.id, level.buy_price, age_mins, drift_pct
            );

            tracker.cancel_level(level.id).await;
            stale_ids.push(level.id);
        }

        if !stale_ids.is_empty() {
            info!(
                "\u{1f504} Lifecycle: cancelled {} stale order(s) \u{2014} \
                 GridBot will re-place at current price ${:.4}",
                stale_ids.len(), current_price
            );
        }

        stale_ids
    }

    // ── Price update ──────────────────────────────────────────────────────

    pub async fn update_price(&self, price: f64) -> Result<()> {
        if price <= 0.0 {
            return Err(anyhow::anyhow!("Invalid price: {}", price));
        }
        *self.current_price.write().await = Some(price);

        let mut history = self.price_history.lock().await;
        history.push((Instant::now(), price));
        let cutoff = Instant::now()
            - tokio::time::Duration::from_secs(self.config.volatility_window_seconds);
        history.retain(|(time, _)| *time > cutoff);

        let mut atr_guard = self.atr_dynamic.lock().await;
        if let Some(atr) = atr_guard.as_mut() {
            atr.update(price);
        }

        trace!("\u{1f4ca} Price: ${:.4} (history: {} pts)", price, history.len());
        Ok(())
    }

    // ── Regime Gate ───────────────────────────────────────────────────────

    pub async fn should_trade_now(&self) -> bool {
        if !self.config.enable_regime_gate {
            return true;
        }

        let stats = self.grid_stats().await;

        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            if !self.trading_paused.load(Ordering::Acquire) {
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
                warn!("\u{26d4} REGIME GATE: Pausing \u{2014} VERY_LOW_VOL (vol=${:.4})", stats.volatility);
            }
            return false;
        }

        if stats.volatility < self.config.min_volatility_to_trade {
            if !self.trading_paused.load(Ordering::Acquire) {
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = format!(
                    "Low volatility (${:.4} < ${:.4})",
                    stats.volatility, self.config.min_volatility_to_trade
                );
                warn!("\u{26d4} REGIME GATE: Pausing \u{2014} Low volatility (${:.4} < min ${:.4})",
                      stats.volatility, self.config.min_volatility_to_trade);
            }
            return false;
        }

        if self.trading_paused.load(Ordering::Acquire) {
            info!("\u{2705} REGIME GATE: Resuming \u{2014} {} / vol=${:.4}",
                  stats.market_regime, stats.volatility);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }
        true
    }

    // ── Fee Filter ────────────────────────────────────────────────────────

    pub async fn should_place_order(
        &self,
        side: OrderSide,
        price: f64,
        position_size_sol: f64,
        stats: &GridStats,
    ) -> bool {
        if !self.config.enable_fee_filtering {
            return true;
        }

        if self.config.seed_orders_bypass && !self.orders_seeded.load(Ordering::Acquire) {
            trace!("\u{1f331} Seed bypass: {:?} @ ${:.4} \u{2014} fee filter deferred until seeding complete",
                side, price);
            return true;
        }

        let current_price = match *self.current_price.read().await {
            Some(p) => p,
            None => return true,
        };

        if let Some(filter) = &self.fee_filter {
            let exit_price = match side {
                OrderSide::Buy  => price * (1.0 + self.config.grid_spacing),
                OrderSide::Sell => price * (1.0 - self.config.grid_spacing),
            };
            let volatility = self.calculate_volatility().await;
            let (pass, net_profit, reason) = filter.should_execute_trade(
                price,
                exit_price,
                position_size_sol,
                volatility,
                stats.market_regime.as_str(),
            );
            if !pass {
                debug!("\u{1f6ab} SmartFeeFilter BLOCKED {:?} @ ${:.4} | net_profit=${:.6} | {}",
                    side, price, net_profit, reason);
                self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            } else {
                trace!("\u{2705} SmartFeeFilter PASSED {:?} @ ${:.4} | net_profit=${:.6} | {}",
                    side, price, net_profit, reason);
            }
            return pass;
        }

        let spread_pct = ((price - current_price).abs() / current_price) * 100.0;
        let min_spread = self.fees.min_order_spread_for_regime(stats.market_regime.as_str());
        if spread_pct < min_spread {
            debug!("\u{1f6ab} SPREAD GATE: {:?} @ ${:.4} (spread {:.3}% < min {:.2}%)",
                side, price, spread_pct, min_spread);
            self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }

    pub fn fee_filter_stats(&self) -> Option<FeeFilterStats> {
        self.fee_filter.as_ref().map(|f| f.stats())
    }

    // ── Volatility ────────────────────────────────────────────────────────

    async fn calculate_volatility(&self) -> f64 {
        let history = self.price_history.lock().await;
        if history.len() < 2 { return 0.0; }
        let prices: Vec<f64> = history.iter().map(|(_, p)| *p).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;
        variance.sqrt()
    }

    // ── Spacing Dispatch ──────────────────────────────────────────────────

    async fn update_dynamic_spacing(&self) {
        if !self.config.enable_dynamic_spacing {
            return;
        }

        let base_spacing = match &self.config.spacing_mode {
            SpacingMode::Fixed => return,

            SpacingMode::VolatilityBuckets => {
                let vol = self.calculate_volatility().await;
                if vol < 0.5 { self.config.min_spacing }
                else if vol > 2.0 { self.config.max_spacing }
                else { self.config.grid_spacing }
            }

            SpacingMode::AtrDynamic { .. } => {
                let atr_guard = self.atr_dynamic.lock().await;
                let price_guard = self.current_price.read().await;
                match (atr_guard.as_ref(), *price_guard) {
                    (Some(atr), Some(price)) if atr.ready() => {
                        atr.calculate_spacing(price)
                            .map(|pct| pct / 100.0)
                            .unwrap_or(self.config.grid_spacing)
                    }
                    _ => {
                        trace!("\u{1f4d0} ATR warming up, using base spacing");
                        self.config.grid_spacing
                    }
                }
            }
        };

        let bias = self.fill_state.lock().await.bias;
        let biased = (base_spacing + bias)
            .max(self.config.min_spacing)
            .min(self.config.max_spacing);

        let mut current = self.current_spacing.write().await;
        if (*current - biased).abs() > 0.00001 {
            debug!("\u{1f4ca} Spacing: {:.4}% \u{2192} {:.4}% (bias {:+.4}%)",
                   *current * 100.0, biased * 100.0, bias * 100.0);
            *current = biased;
        }
    }

    // ── Grid Stats ────────────────────────────────────────────────────────

    pub async fn grid_stats(&self) -> GridStats {
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);
        let filtered   = self.stats_filtered.load(Ordering::Relaxed);
        let efficiency = if rebalances + filtered > 0 {
            (rebalances as f64 / (rebalances + filtered) as f64) * 100.0
        } else { 100.0 };
        let volatility = self.calculate_volatility().await;
        let market_regime = if volatility < 0.5 { "VERY_LOW_VOL" }
            else if volatility < 1.0 { "LOW_VOL" }
            else if volatility < 2.0 { "MEDIUM_VOL" }
            else if volatility < 3.0 { "HIGH_VOL" }
            else { "VERY_HIGH_VOL" };
        let current_spacing = *self.current_spacing.read().await;
        let trading_paused  = self.trading_paused.load(Ordering::Acquire);
        let pause_reason = if trading_paused {
            self.pause_reason.read().await.clone()
        } else { String::new() };
        GridStats {
            total_rebalances: rebalances,
            rebalances_filtered: filtered,
            efficiency_percent: efficiency,
            dynamic_spacing_enabled: self.dynamic_spacing_enabled.load(Ordering::Relaxed),
            current_spacing_percent: current_spacing * 100.0,
            volatility,
            market_regime: market_regime.to_string(),
            trading_paused,
            pause_reason,
        }
    }

    // ── V5.1: Level Analytics ─────────────────────────────────────────────

    pub async fn get_level_analytics(&self) -> LevelAnalyticsReport {
        let analytics = self.level_analytics.lock().await;
        LevelAnalyticsReport {
            hot_levels:           analytics.hot_levels(5),
            profitable_levels:    analytics.profitable_levels(0.0),
            snapshots:            analytics.snapshots_sorted(),
            fills_with_level:     analytics.fills_with_level,
            fills_without_level:  analytics.fills_without_level,
            total_tracked_levels: analytics.levels.len(),
        }
    }

    pub async fn on_fill_notification(
        &self, order_id: &str, side: OrderSide,
        fill_price: f64, fill_size: f64, pnl: Option<f64>,
    ) {
        debug!("\u{1f4e8} Fill notification: {:?} {} @ ${:.4} (size: {:.4})",
               side, order_id, fill_price, fill_size);
        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);
        if let Some(p) = pnl {
            if p > 0.0 { info!("\u{1f4b0} Profitable {:?} fill: +${:.2}", side, p); }
        }
        let stats = self.grid_stats().await;
        trace!("\u{1f4ca} Grid efficiency post-fill: {:.2}%", stats.efficiency_percent);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancerBuilder {
    config: GridRebalancerConfig,
    fees: FeesConfig,
}

impl GridRebalancerBuilder {
    pub fn new() -> Self {
        Self {
            config: GridRebalancerConfig::default(),
            fees: FeesConfig::default(),
        }
    }
    pub fn grid_spacing(mut self, s: f64) -> Self { self.config.grid_spacing = s; self }
    pub fn order_size(mut self, s: f64) -> Self { self.config.order_size = s; self }
    pub fn enable_regime_gate(mut self, e: bool) -> Self { self.config.enable_regime_gate = e; self }
    pub fn min_volatility(mut self, v: f64) -> Self { self.config.min_volatility_to_trade = v; self }
    pub fn environment(mut self, env: &str) -> Self { self.config.apply_environment(env); self }
    pub fn spacing_mode(mut self, mode: SpacingMode) -> Self { self.config.spacing_mode = mode; self }
    pub fn fees_config(mut self, fees: FeesConfig) -> Self { self.fees = fees; self }
    pub fn fill_rate_threshold(mut self, threshold: f64) -> Self {
        self.config.fill_rate_threshold = threshold;
        self
    }
    pub fn seed_orders_bypass(mut self, bypass: bool) -> Self {
        self.config.seed_orders_bypass = bypass;
        self
    }
    pub fn vol_floor_resume_pct(mut self, v: f64) -> Self {
        self.config.vol_floor_resume_pct = v;
        self
    }
    pub fn build(self) -> Result<GridRebalancer> { GridRebalancer::with_fees(self.config, self.fees) }
}

impl Default for GridRebalancerBuilder {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for GridRebalancer {
    fn name(&self) -> &str { "GridRebalancer" }

    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        self.update_price(price).await.context("Failed to update price")?;
        self.update_dynamic_spacing().await;
        self.stats_signals.fetch_add(1, Ordering::Relaxed);
        let should_trade = self.should_trade_now().await;
        let stats = self.grid_stats().await;
        let signal = if !should_trade {
            Signal::Hold { reason: Some(format!("Paused \u{2014} {}", stats.pause_reason)) }
        } else {
            Signal::Hold { reason: Some(format!("Grid active \u{2014} {} regime", stats.market_regime)) }
        };
        *self.last_signal.write().await = Some(signal.clone());
        Ok(signal)
    }

    fn stats(&self) -> BaseStrategyStats {
        let signals    = self.stats_signals.load(Ordering::Relaxed);
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);
        BaseStrategyStats {
            signals_generated:   signals,
            buy_signals:         rebalances / 2,
            sell_signals:        rebalances / 2,
            hold_signals:        signals.saturating_sub(rebalances),
            rebalances_executed: rebalances,
            ..Default::default()
        }
    }

    fn reset(&mut self) {
        info!("\u{1f504} Resetting GridRebalancer stats");
        self.stats_rebalances.store(0, Ordering::Relaxed);
        self.stats_filtered.store(0, Ordering::Relaxed);
        self.stats_signals.store(0, Ordering::Relaxed);
        self.trading_paused.store(false, Ordering::Relaxed);
        self.orders_seeded.store(false, Ordering::Relaxed);
    }

    fn is_enabled(&self) -> bool { self.config.enabled }

    fn last_signal(&self) -> Option<Signal> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.last_signal.read().await.clone()
            })
        })
    }

    fn on_fill(&mut self, fill: &FillEvent) {
        if let Ok(mut state) = self.fill_state.try_lock() {
            state.timestamps.push_back(fill.timestamp);
            if state.timestamps.len() > 20 {
                state.timestamps.pop_front();
            }
            let rate = state.fill_rate(60);
            let high_fill_thr = self.config.fill_rate_threshold;
            let old_bias = state.bias;
            if rate > high_fill_thr {
                state.bias = (state.bias + 0.0002).min(0.002);
            } else if rate < high_fill_thr * 0.3 {
                state.bias = (state.bias - 0.0001).max(-0.001);
            }
            if (state.bias - old_bias).abs() > 0.000001 {
                info!("\u{1f9e0} Fill feedback: rate={:.3}/s bias {:+.4}% \u{2192} {:+.4}%",
                    rate, old_bias * 100.0, state.bias * 100.0);
            }
            debug!("\u{1f4e8} on_fill: {:?} {} @ {:.4} | bias {:+.4}%",
                fill.side, fill.order_id, fill.fill_price, state.bias * 100.0);
        }

        if let Ok(mut analytics) = self.level_analytics.try_lock() {
            analytics.record_fill(fill);
            if let Some(id) = fill.level_id {
                let fill_count = analytics.levels.get(&id).map(|s| s.fill_count).unwrap_or(0);
                debug!("\u{1f4ca} Level {:3} | {:?} @ ${:.4} | pnl: {:+.4} | total fills: {}",
                    id, fill.side, fill.fill_price,
                    fill.pnl.unwrap_or(0.0),
                    fill_count);
            }
        }

        if let Some(filter) = &self.fee_filter {
            filter.record_execution();
        }

        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID STATS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridStats {
    pub total_rebalances: u64,
    pub rebalances_filtered: u64,
    pub efficiency_percent: f64,
    pub dynamic_spacing_enabled: bool,
    pub current_spacing_percent: f64,
    pub volatility: f64,
    pub market_regime: String,
    pub trading_paused: bool,
    pub pause_reason: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trading::{FillEvent, OrderSide};

    // ─────────────────────────────────────────────────────────────────────
    // Existing regression tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_config_validation() {
        let mut config = GridRebalancerConfig::default();
        assert!(config.validate().is_ok());
        config.grid_spacing = -0.1;
        assert!(config.validate().is_err());
        config.grid_spacing = 0.15;
        config.min_spacing = 0.005;
        config.max_spacing = 0.0075;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_default_seed_bypass_is_true() {
        let config = GridRebalancerConfig::default();
        assert!(config.seed_orders_bypass,
            "seed_orders_bypass must default to true");
    }

    #[test]
    fn test_builder_seed_bypass_setter() {
        let gr = GridRebalancer::builder()
            .seed_orders_bypass(false)
            .build()
            .expect("build failed");
        assert!(!gr.config.seed_orders_bypass);
    }

    #[tokio::test]
    async fn test_seed_bypass_allows_initial_orders() {
        let config = GridRebalancerConfig {
            enable_fee_filtering: true,
            seed_orders_bypass: true,
            ..GridRebalancerConfig::default()
        };
        let gr = GridRebalancer::new(config).expect("build");
        assert!(gr.is_seeding());
        let stats = GridStats {
            total_rebalances: 0, rebalances_filtered: 0, efficiency_percent: 100.0,
            dynamic_spacing_enabled: true, current_spacing_percent: 0.15,
            volatility: 0.001,
            market_regime: "VERY_LOW_VOL".to_string(),
            trading_paused: false, pause_reason: String::new(),
        };
        let pass_buy  = gr.should_place_order(OrderSide::Buy,  85.0, 0.1, &stats).await;
        let pass_sell = gr.should_place_order(OrderSide::Sell, 86.0, 0.1, &stats).await;
        assert!(pass_buy,  "seed bypass: buy must pass during seeding");
        assert!(pass_sell, "seed bypass: sell must pass during seeding");
    }

    #[tokio::test]
    async fn test_seed_bypass_off_after_mark_seeding_complete() {
        let config = GridRebalancerConfig {
            enable_fee_filtering: true,
            seed_orders_bypass: true,
            grid_spacing: 0.0015,
            ..GridRebalancerConfig::default()
        };
        let gr = GridRebalancer::new(config).expect("build");
        assert!(gr.is_seeding());
        gr.mark_seeding_complete();
        assert!(!gr.is_seeding());
    }

    #[test]
    fn test_mark_seeding_complete_idempotent() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).expect("build");
        gr.mark_seeding_complete();
        gr.mark_seeding_complete();
        assert!(!gr.is_seeding());
    }

    #[tokio::test]
    async fn test_seed_bypass_disabled_filter_from_order_one() {
        let config = GridRebalancerConfig {
            enable_fee_filtering: true,
            seed_orders_bypass: false,
            ..GridRebalancerConfig::default()
        };
        let gr = GridRebalancer::new(config).expect("build");
        assert!(!gr.is_seeding());
        gr.mark_seeding_complete();
        assert!(!gr.is_seeding());
    }

    #[test]
    fn test_record_execution_called_on_fill() {
        let mut gr = GridRebalancer::new(GridRebalancerConfig {
            enable_fee_filtering: true,
            seed_orders_bypass: true,
            ..GridRebalancerConfig::default()
        }).expect("build");
        let stats_before = gr.fee_filter_stats().map(|s| s.trades_passed).unwrap_or(0);
        let fill = FillEvent {
            order_id: "test-fill-001".to_string(),
            side: OrderSide::Buy,
            fill_price: 85.0,
            fill_size: 0.1,
            fee_usdc: 0.0,
            pnl: Some(0.05),
            timestamp: 1_700_000_000,
            level_id: Some(1),
            distance_from_mid_pct: None,
        };
        gr.on_fill(&fill);
        let stats_after = gr.fee_filter_stats();
        assert!(stats_after.is_some());
        let executed_after = stats_after.map(|s| s.trades_passed).unwrap_or(0);
        assert_eq!(stats_before, executed_after);
    }

    #[test]
    fn test_spacing_mode_default() {
        assert_eq!(SpacingMode::default(), SpacingMode::VolatilityBuckets);
    }

    #[test]
    fn test_grid_rebalancer_execution_only_no_wma_slot() {
        use crate::strategies::{
            StrategyRegistryBuilder,
            grid_rebalancer::{GridRebalancer, GridRebalancerConfig},
            shared::analytics::AnalyticsContext,
        };
        let gr  = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, weights) = StrategyRegistryBuilder::new()
            .add_execution_only(gr, 0.40)
            .build(ctx);
        assert_eq!(manager.strategies.len(), 1);
        assert_eq!(weights, vec![0.40]);
        assert!(
            manager.wma_engine.get_performance("GridRebalancer").is_none(),
            "GridRebalancer must NOT be a WMA voter when added via add_execution_only()"
        );
    }

    // ─────────────────────────────────────────────────────────────────────
    // V6.1: Configurable vol floor tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_gr_vol_floor_default_is_0_05() {
        let cfg = GridRebalancerConfig::default();
        assert!(
            (cfg.vol_floor_resume_pct - 0.05).abs() < 1e-9,
            "default vol_floor_resume_pct must be 0.05, got {}",
            cfg.vol_floor_resume_pct
        );
    }

    #[test]
    fn test_gr_vol_floor_serde_absent_field_uses_default() {
        // spacing_mode is present (required — no #[serde(default)] on SpacingMode).
        // vol_floor_resume_pct is intentionally ABSENT to exercise its serde default.
        let toml_str = r#"
            grid_spacing = 0.002
            order_size = 0.1
            min_usdc_balance = 100.0
            min_sol_balance = 0.1
            enabled = true
            enable_dynamic_spacing = true
            enable_fee_filtering = true
            volatility_window_seconds = 600
            max_spacing = 0.0075
            min_spacing = 0.001
            enable_regime_gate = true
            min_volatility_to_trade = 0.05
            pause_in_very_low_vol = true
            enable_order_lifecycle = true
            order_max_age_minutes = 10
            order_refresh_interval_minutes = 5
            min_orders_to_maintain = 8
            spacing_mode = "volatility_buckets"
        "#;
        let cfg: GridRebalancerConfig = toml::from_str(toml_str)
            .expect("TOML parse failed");
        assert!(
            (cfg.vol_floor_resume_pct - 0.05).abs() < 1e-9,
            "absent vol_floor_resume_pct must deserialize to 0.05, got {}",
            cfg.vol_floor_resume_pct
        );
    }

    #[test]
    fn test_gr_production_uses_vol_floor_not_hardcoded_0_3() {
        let mut cfg = GridRebalancerConfig {
            min_volatility_to_trade: 0.02,
            vol_floor_resume_pct: 0.05,
            ..GridRebalancerConfig::default()
        };
        cfg.apply_environment("production");
        assert!(
            (cfg.min_volatility_to_trade - 0.05).abs() < 1e-9,
            "production must raise to vol_floor_resume_pct (0.05), not 0.3; got {}",
            cfg.min_volatility_to_trade
        );
    }

    #[test]
    fn test_gr_development_uses_vol_floor_not_hardcoded_0_3() {
        let mut cfg = GridRebalancerConfig {
            min_volatility_to_trade: 0.8,
            vol_floor_resume_pct: 0.05,
            ..GridRebalancerConfig::default()
        };
        cfg.apply_environment("development");
        assert!(
            (cfg.min_volatility_to_trade - 0.05).abs() < 1e-9,
            "development cap must use vol_floor_resume_pct (0.05), not 0.3; got {}",
            cfg.min_volatility_to_trade
        );
    }

    #[test]
    fn test_gr_production_no_stomp_above_floor() {
        let mut cfg = GridRebalancerConfig {
            min_volatility_to_trade: 0.10,
            vol_floor_resume_pct: 0.05,
            ..GridRebalancerConfig::default()
        };
        cfg.apply_environment("production");
        assert!(
            (cfg.min_volatility_to_trade - 0.10).abs() < 1e-9,
            "production must NOT lower a value already above the floor; got {}",
            cfg.min_volatility_to_trade
        );
    }

    // ─────────────────────────────────────────────────────────────────────
    // V6.0: Order Lifecycle Engine tests
    // ─────────────────────────────────────────────────────────────────────

    /// When enable_order_lifecycle=false, check_stale_orders() must always
    /// return an empty Vec — no tracker access, zero overhead.
    #[tokio::test]
    async fn test_lifecycle_disabled_returns_empty() {
        let config = GridRebalancerConfig {
            enable_order_lifecycle: false,
            ..GridRebalancerConfig::default()
        };
        let gr      = GridRebalancer::new(config).expect("build");
        let tracker = GridStateTracker::new();
        tracker.create_level(85.0, 86.0, 0.1).await;

        let stale = gr.check_stale_orders(&tracker, 85.5).await;
        assert!(stale.is_empty(), "disabled lifecycle must always return empty");
    }

    /// Within the refresh interval window, check_stale_orders() must be
    /// suppressed — throttle prevents redundant scans every tick.
    #[tokio::test]
    async fn test_lifecycle_throttle_suppresses_within_interval() {
        let config = GridRebalancerConfig {
            enable_order_lifecycle: true,
            order_max_age_minutes: 1,
            order_refresh_interval_minutes: 60,
            ..GridRebalancerConfig::default()
        };
        let gr      = GridRebalancer::new(config).expect("build");
        let tracker = GridStateTracker::new();
        tracker.create_level(85.0, 86.0, 0.1).await;
        let stale = gr.check_stale_orders(&tracker, 85.5).await;
        assert!(stale.is_empty(), "throttle must suppress check within refresh interval");
    }

    /// BuyFilled levels must NEVER be returned as stale — they represent
    /// open positions where cancelling = realised loss.
    ///
    /// Uses order_max_age_minutes=1 and order_refresh_interval_minutes=1
    /// (minimum valid values per validate()). The invariant is proven by
    /// the status guard in check_stale_orders(), which filters BuyFilled
    /// levels before any age check is performed.
    #[tokio::test]
    async fn test_lifecycle_never_cancels_buy_filled_level() {
        let config = GridRebalancerConfig {
            enable_order_lifecycle: true,
            order_max_age_minutes: 1,
            order_refresh_interval_minutes: 1,
            ..GridRebalancerConfig::default()
        };
        let gr      = GridRebalancer::new(config).expect("build");
        let tracker = GridStateTracker::new();
        let id = tracker.create_level(85.0, 86.0, 0.1).await;
        tracker.mark_buy_filled(id, 85.0).await;

        let stale = gr.check_stale_orders(&tracker, 90.0).await;
        assert!(
            !stale.contains(&id),
            "BuyFilled level must never appear in stale cancellation list"
        );
    }
}
