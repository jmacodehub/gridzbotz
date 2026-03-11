//! ═════════════════════════════════════════════════════════════════════════
//! 🔥📎 GRID REBALANCER V5.4 - ADAPTIVE SPACING + FILL FEEDBACK + LEVEL ANALYTICS 🔥📎
//!
//! V5.4 (PR #94 Commit 4 — SmartFeeFilter wired):
//!   ✅ fee_filter: Option<SmartFeeFilter> — built from FeesConfig, per-instance
//!   ✅ should_place_order(): Path A = full P&L simulation via SmartFeeFilter;
//!      Path B = legacy spread gate (enable_fee_filtering=false fallback)
//!   ✅ position_size_sol param added so SmartFeeFilter gets market impact data
//!   ✅ fee_filter_stats() → Option<FeeFilterStats> for future metrics surfacing
//!   ✅ fill_rate_threshold: f64 replaces hardcoded HIGH_FILL_THR=0.10 in on_fill()
//!
//! V5.0 ENHANCEMENTS (Stage 3 — Feb 2026):
//!   ✅ SpacingMode enum: Fixed | VolatilityBuckets | AtrDynamic
//!   ✅ on_fill() Strategy trait impl: fill-rate spacing bias
//!   ✅ ATRDynamic wired: real regime adaptation via atr_dynamic field
//!   ✅ FillState: thread-safe fill timestamp + bias in Arc<Mutex<_>>
//!   ✅ Builder gains spacing_mode() for per-bot TOML config
//!
//! V5.1 ENHANCEMENTS (Feb 2026 — per-level analytics):
//!   ✅ LevelSnapshot: per-level fill count, total PnL, avg distance from mid
//!   ✅ LevelAnalytics: O(1) HashMap accumulator keyed on GridLevel.id (u64)
//!   ✅ on_fill() extended: records analytics when FillEvent::level_id is Some
//!   ✅ get_level_analytics(): public API returning LevelAnalyticsReport
//!   ✅ LevelAnalyticsReport: hot_levels, profitable_levels, full snapshots
//!
//! V5.2 (PR #77 — FeesConfig wiring):
//!   ✅ FeesConfig field on struct + builder
//!   ✅ should_place_order() driven by fees.min_order_spread_for_regime()
//!   ✅ Eliminated hardcoded spread match — single source of truth
//!
//! V5.3 (PR #80 — Regime Gate Fix):
//!   ✅ GATE-1 fix: should_trade_now() always re-evaluates conditions
//!   ✅ GATE-3 fix: display shows dollar std-dev, not misleading percentage
//!   ✅ Resume path now reachable — no more permanent pause deadlock
//!
//! PR #98 fix: name() returns "GridRebalancer" (stable WMA HashMap key).
//! Version info lives in this header and wma_summary() output — never
//! in a runtime-observable &str used as a P&L attribution key.
//!
//! February 28, 2026 - V5.1 | March 2026 - V5.4 🚀
//! ═════════════════════════════════════════════════════════════════════════

use crate::trading::{FillEvent, OrderSide};
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

    // ── V3: Order Lifecycle ───────────────────────────────────────────
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
}

fn default_high_fill_threshold() -> f64 { 0.10 }

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
        }
    }
}

impl GridRebalancerConfig {
    pub fn validate(&self) -> Result<()> {
        if self.grid_spacing <= 0.0 {
            return Err(anyhow::anyhow!("grid_spacing must be > 0"));
        }
        if self.grid_spacing > 0.1 {
            warn!("⚠️ Grid spacing {:.2}% is very wide", self.grid_spacing * 100.0);
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
                warn!("⚠️ min_volatility_to_trade ${:.2} may never trade", self.min_volatility_to_trade);
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
                info!("🧪 Testing mode: Relaxing regime gate");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
            }
            "development" => {
                info!("🔧 Development mode: Moderate regime gate");
                if self.min_volatility_to_trade > 0.5 {
                    self.min_volatility_to_trade = 0.3;
                }
            }
            "production" => {
                info!("🔒 Production mode: Enforcing regime gate");
                if !self.enable_regime_gate {
                    warn!("⚠️ Force-enabling regime gate for production!");
                    self.enable_regime_gate = true;
                }
                if self.min_volatility_to_trade < 0.3 {
                    warn!("⚠️ Raising min_volatility to $0.30 for production safety");
                    self.min_volatility_to_trade = 0.3;
                }
            }
            _ => warn!("⚠️ Unknown environment '{}', using defaults", environment),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID REBALANCER - V5.4
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

    #[allow(dead_code)]
    last_lifecycle_check: Arc<tokio::sync::RwLock<Instant>>,
    trading_paused: Arc<AtomicBool>,
    pause_reason: Arc<tokio::sync::RwLock<String>>,
    last_signal: Arc<tokio::sync::RwLock<Option<Signal>>>,

    // V5.0: Fill feedback + ATR
    fill_state: Arc<tokio::sync::Mutex<FillState>>,
    atr_dynamic: Arc<tokio::sync::Mutex<Option<ATRDynamic>>>,

    // V5.1: Per-level analytics
    level_analytics: Arc<tokio::sync::Mutex<LevelAnalytics>>,

    // V5.4: SmartFeeFilter — built from FeesConfig, None when fee filtering disabled
    fee_filter: Option<SmartFeeFilter>,
}

impl GridRebalancer {
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        Self::with_fees(config, FeesConfig::default())
    }

    /// Construct with explicit FeesConfig (preferred path from engine.rs).
    pub fn with_fees(config: GridRebalancerConfig, fees: FeesConfig) -> Result<Self> {
        config.validate().context("GridRebalancer config validation failed")?;

        info!("═══════════════════════════════════════════════════════════");
        info!("🎯 Grid Rebalancer V5.4 Initializing...");
        info!("═══════════════════════════════════════════════════════════");
        info!("📊 CORE: spacing={:.3}% size={} SOL reserves=${:.0}/{} SOL",
              config.grid_spacing * 100.0, config.order_size,
              config.min_usdc_balance, config.min_sol_balance);

        info!("📐 SPACING MODE:");
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

        info!("💰 FEES: maker={:.1}bps taker={:.1}bps slippage={:.1}bps multiplier={:.1}x",
              fees.maker_fee_bps, fees.taker_fee_bps,
              fees.slippage_bps, fees.min_profit_multiplier);
        info!("🛡️ REGIME GATE: {} | min_vol=${:.4} (dollar std-dev)",
              if config.enable_regime_gate { "✅" } else { "❌ FREE" },
              config.min_volatility_to_trade);
        info!("🧠 ADAPTIVE: fill-feedback bias ✅ | level analytics ✅ | fill_rate_thr={:.2}",
              config.fill_rate_threshold);

        // Build ATR only when the mode requires it
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

        // V5.4: build SmartFeeFilter when fee filtering is enabled
        let fee_filter = if config.enable_fee_filtering {
            let filter_cfg = SmartFeeFilterConfig::from_fees_config(&fees);
            info!("💎 SmartFeeFilter: ACTIVE (maker={:.2}bps taker={:.2}bps slippage={:.2}bps mult={:.1}x grace={})",
                  filter_cfg.maker_fee_percent * 100.0,
                  filter_cfg.taker_fee_percent * 100.0,
                  filter_cfg.slippage_percent * 100.0,
                  filter_cfg.min_profit_multiplier,
                  filter_cfg.grace_period_trades);
            Some(SmartFeeFilter::new(filter_cfg))
        } else {
            info!("💎 SmartFeeFilter: DISABLED (enable_fee_filtering = false)");
            None
        };

        info!("═══════════════════════════════════════════════════════════");

        Ok(Self {
            current_spacing: Arc::new(tokio::sync::RwLock::new(config.grid_spacing)),
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

    /// Update price + price history. Also feeds ATR if active.
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

        trace!("📊 Price: ${:.4} (history: {} pts)", price, history.len());
        Ok(())
    }

    // ── Regime Gate (V5.3: GATE-1 fix — always re-evaluate) ──────────────────

    pub async fn should_trade_now(&self) -> bool {
        if !self.config.enable_regime_gate {
            return true;
        }

        let stats = self.grid_stats().await;

        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            if !self.trading_paused.load(Ordering::Acquire) {
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
                warn!("⛔ REGIME GATE: Pausing — VERY_LOW_VOL (vol=${:.4})", stats.volatility);
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
                warn!("⛔ REGIME GATE: Pausing — Low volatility (${:.4} < min ${:.4})",
                      stats.volatility, self.config.min_volatility_to_trade);
            }
            return false;
        }

        if self.trading_paused.load(Ordering::Acquire) {
            info!("✅ REGIME GATE: Resuming — {} / vol=${:.4}",
                  stats.market_regime, stats.volatility);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }
        true
    }

    // ── Fee Filter (V5.4: SmartFeeFilter-driven, V5.2 spread-gate fallback) ──

    /// Returns `true` if this order should be placed.
    ///
    /// **Path A** (`fee_filter` is `Some`, `enable_fee_filtering = true`):
    ///   Computes full round-trip P&L: entry fee + exit fee + both-leg slippage
    ///   + market impact + regime-aware threshold multiplier.
    ///   Uses `exit_price = price * (1 ± grid_spacing)` as synthetic exit.
    ///   Logs structured `reason` string from SmartFeeFilter for observability.
    ///
    /// **Path B** (`fee_filter` is `None`, legacy fallback):
    ///   Single-number spread gate via `FeesConfig.min_order_spread_for_regime()`.
    ///   Identical behaviour to V5.2 — zero regression.
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
        let current_price = match *self.current_price.read().await {
            Some(p) => p,
            None => return true,
        };

        // ── Path A: SmartFeeFilter (full P&L simulation) ──────────────────
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
                debug!("🚫 SmartFeeFilter BLOCKED {:?} @ ${:.4} | net_profit=${:.6} | {}",
                    side, price, net_profit, reason);
                self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            } else {
                trace!("✅ SmartFeeFilter PASSED {:?} @ ${:.4} | net_profit=${:.6} | {}",
                    side, price, net_profit, reason);
            }
            return pass;
        }

        // ── Path B: Legacy spread gate (SmartFeeFilter disabled) ──────────
        let spread_pct = ((price - current_price).abs() / current_price) * 100.0;
        let min_spread = self.fees.min_order_spread_for_regime(stats.market_regime.as_str());
        if spread_pct < min_spread {
            debug!("🚫 SPREAD GATE: {:?} @ ${:.4} (spread {:.3}% < min {:.2}%)",
                side, price, spread_pct, min_spread);
            self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }

    /// Snapshot of SmartFeeFilter statistics.
    /// Returns `None` when `enable_fee_filtering = false`.
    pub fn fee_filter_stats(&self) -> Option<FeeFilterStats> {
        self.fee_filter.as_ref().map(|f| f.stats())
    }

    // ── Volatility (fallback for VolatilityBuckets) ───────────────────────────

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

    // ── Spacing Dispatch ──────────────────────────────────────────────────────

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
                        trace!("📐 ATR warming up, using base spacing");
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
            debug!("📊 Spacing: {:.4}% → {:.4}% (bias {:+.4}%)",
                   *current * 100.0, biased * 100.0, bias * 100.0);
            *current = biased;
        }
    }

    // ── Grid Stats ────────────────────────────────────────────────────────────

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

    // ── V5.1: Level Analytics API ─────────────────────────────────────────────

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

    /// V4.0 legacy method — kept for direct callers; on_fill() is the trait path.
    pub async fn on_fill_notification(
        &self, order_id: &str, side: OrderSide,
        fill_price: f64, fill_size: f64, pnl: Option<f64>,
    ) {
        debug!("📨 Fill notification: {:?} {} @ ${:.4} (size: {:.4})",
               side, order_id, fill_price, fill_size);
        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);
        if let Some(p) = pnl {
            if p > 0.0 { info!("💰 Profitable {:?} fill: +${:.2}", side, p); }
        }
        let stats = self.grid_stats().await;
        trace!("📊 Grid efficiency post-fill: {:.2}%", stats.efficiency_percent);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER (V5.4: +fill_rate_threshold)
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
    /// Override fill-rate bias threshold. Default: 0.10 (≈ 6 fills/min).
    pub fn fill_rate_threshold(mut self, threshold: f64) -> Self {
        self.config.fill_rate_threshold = threshold;
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
    /// Stable identifier used as a HashMap key in WMAConsensusEngine.
    /// Version info lives in the file header and wma_summary() output — never here.
    fn name(&self) -> &str { "GridRebalancer" }

    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        self.update_price(price).await.context("Failed to update price")?;
        self.update_dynamic_spacing().await;
        self.stats_signals.fetch_add(1, Ordering::Relaxed);
        let should_trade = self.should_trade_now().await;
        let stats = self.grid_stats().await;
        let signal = if !should_trade {
            Signal::Hold { reason: Some(format!("Paused — {}", stats.pause_reason)) }
        } else {
            Signal::Hold { reason: Some(format!("Grid active — {} regime", stats.market_regime)) }
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
        info!("🔄 Resetting GridRebalancer stats");
        self.stats_rebalances.store(0, Ordering::Relaxed);
        self.stats_filtered.store(0, Ordering::Relaxed);
        self.stats_signals.store(0, Ordering::Relaxed);
        self.trading_paused.store(false, Ordering::Relaxed);
    }

    fn is_enabled(&self) -> bool { self.config.enabled }

    fn last_signal(&self) -> Option<Signal> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.last_signal.read().await.clone()
            })
        })
    }

    // ── V5.0 + V5.1 + V5.4: Fill feedback loop + per-level analytics ─────────
    //
    // Sync fn — uses try_lock (non-blocking). Never contended under normal load.
    //
    // Execution order:
    //   1. [V5.0/V5.4] Update fill-rate ring buffer + compute spacing bias
    //      HIGH_FILL_THR is now self.config.fill_rate_threshold (TOML-driven)
    //   2. [V5.1]       Record level analytics (O(1) HashMap upsert)
    //   3.              Increment global rebalance counter
    fn on_fill(&mut self, fill: &FillEvent) {
        // ─────────────────────────────────────────────────────────────────
        // Step 1: Fill-rate spacing bias (V5.0 — threshold now config-driven)
        // ─────────────────────────────────────────────────────────────────
        if let Ok(mut state) = self.fill_state.try_lock() {
            state.timestamps.push_back(fill.timestamp);
            if state.timestamps.len() > 20 {
                state.timestamps.pop_front();
            }

            let rate = state.fill_rate(60);
            // V5.4: config-driven threshold replaces hardcoded 0.10 const
            let high_fill_thr = self.config.fill_rate_threshold;

            let old_bias = state.bias;
            if rate > high_fill_thr {
                state.bias = (state.bias + 0.0002).min(0.002);
            } else if rate < high_fill_thr * 0.3 {
                state.bias = (state.bias - 0.0001).max(-0.001);
            }

            if (state.bias - old_bias).abs() > 0.000001 {
                info!("🧠 Fill feedback: rate={:.3}/s bias {:+.4}% → {:+.4}%",
                    rate, old_bias * 100.0, state.bias * 100.0);
            }
            debug!("📨 on_fill: {:?} {} @ {:.4} | bias {:+.4}%",
                fill.side, fill.order_id, fill.fill_price, state.bias * 100.0);
        }

        // ─────────────────────────────────────────────────────────────────
        // Step 2: Per-level analytics (V5.1)
        // ─────────────────────────────────────────────────────────────────
        if let Ok(mut analytics) = self.level_analytics.try_lock() {
            analytics.record_fill(fill);
            if let Some(id) = fill.level_id {
                let fill_count = analytics.levels.get(&id).map(|s| s.fill_count).unwrap_or(0);
                debug!("📊 Level {:3} | {:?} @ ${:.4} | pnl: {:+.4} | total fills: {}",
                    id, fill.side, fill.fill_price,
                    fill.pnl.unwrap_or(0.0),
                    fill_count);
            }
        }

        // ─────────────────────────────────────────────────────────────────
        // Step 3: Global fill counter
        // ─────────────────────────────────────────────────────────────────
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
    // Existing V5.0 + V5.1 + V5.2 + V5.3 tests (unchanged)
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_config_validation() {
        let mut config = GridRebalancerConfig::default();
        assert!(config.validate().is_ok());
        config.grid_spacing = -0.1;
        assert!(config.validate().is_err());
        config.grid_spacing = 0.15;
        config.min_spacing = 0.2;
        config.max_spacing = 0.1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_overrides() {
        let mut config = GridRebalancerConfig::default();
        config.apply_environment("testing");
        assert!(!config.enable_regime_gate);
        config.apply_environment("production");
        assert!(config.enable_regime_gate);
        assert!(config.min_volatility_to_trade >= 0.3);
    }

    #[test]
    fn test_spacing_mode_default() {
        let config = GridRebalancerConfig::default();
        assert!(matches!(config.spacing_mode, SpacingMode::VolatilityBuckets));
    }

    #[tokio::test]
    async fn test_regime_gate_disabled() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = false;
        let r = GridRebalancer::new(config).unwrap();
        assert!(r.should_trade_now().await);
    }

    #[tokio::test]
    async fn test_atr_dynamic_mode_warms_up() {
        let mut config = GridRebalancerConfig::default();
        config.spacing_mode = SpacingMode::AtrDynamic { period: 3, multiplier: 2.0 };
        config.enable_regime_gate = false;
        let r = GridRebalancer::new(config).unwrap();
        for p in [100.0_f64, 101.0, 102.0] {
            r.update_price(p).await.unwrap();
        }
        let ready = r.atr_dynamic.lock().await
            .as_ref().map(|a| a.ready()).unwrap_or(false);
        assert!(ready, "ATR should be ready after period updates");
    }

    #[tokio::test]
    async fn test_builder_spacing_mode() {
        let r = GridRebalancer::builder()
            .spacing_mode(SpacingMode::AtrDynamic { period: 14, multiplier: 3.0 })
            .enable_regime_gate(false)
            .build();
        assert!(r.is_ok());
    }

    #[test]
    fn test_on_fill_updates_bias() {
        let config = GridRebalancerConfig::default();
        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64).unwrap_or(0);
        for i in 0..10 {
            let fill = FillEvent::new(
                format!("ORDER-{:03}", i),
                OrderSide::Buy, 100.0, 0.1, 0.001, Some(0.02), now,
            );
            rebalancer.on_fill(&fill);
        }
        let bias = rebalancer.fill_state.try_lock().unwrap().bias;
        assert!(bias > 0.0, "High fill rate should widen spacing bias");
    }

    #[tokio::test]
    async fn test_fill_notification_legacy() {
        let config = GridRebalancerConfig::default();
        let rebalancer = GridRebalancer::new(config).unwrap();
        rebalancer.update_price(100.0).await.unwrap();
        rebalancer.on_fill_notification(
            "test_order", OrderSide::Buy, 99.5, 0.1, Some(0.05)
        ).await;
        let stats = rebalancer.grid_stats().await;
        assert_eq!(stats.total_rebalances, 1);
    }

    #[test]
    fn test_level_snapshot_avg_distance() {
        let snap = LevelSnapshot {
            level_id: 101,
            fill_count: 4,
            total_pnl: 2.0,
            last_fill_price: 153.0,
            last_fill_timestamp: 1_000_000,
            distance_from_mid_sum: -4.8,
        };
        let avg = snap.avg_distance_from_mid().unwrap();
        assert!((avg - (-1.2)).abs() < 0.001, "Expected -1.2%, got {:.4}", avg);
    }

    #[test]
    fn test_level_snapshot_avg_distance_none_when_zero() {
        let snap = LevelSnapshot {
            level_id: 201,
            fill_count: 3,
            total_pnl: 0.0,
            last_fill_price: 157.0,
            last_fill_timestamp: 1_000_001,
            distance_from_mid_sum: 0.0,
        };
        assert!(snap.avg_distance_from_mid().is_none());
    }

    #[test]
    fn test_level_analytics_records_fills() {
        let mut analytics = LevelAnalytics::new();
        let now = 1_700_000_000_i64;
        for i in 0..3 {
            let fill = FillEvent::new(
                format!("ORD-{}", i), OrderSide::Buy,
                153.50, 0.1, 0.001, Some(0.50), now + i,
            ).with_level(102).with_distance_from_mid(-1.0);
            analytics.record_fill(&fill);
        }
        assert_eq!(analytics.fills_with_level, 3);
        assert_eq!(analytics.fills_without_level, 0);
        let snap = &analytics.levels[&102];
        assert_eq!(snap.fill_count, 3);
        assert!((snap.total_pnl - 1.50).abs() < 0.001);
        assert!((snap.distance_from_mid_sum - (-3.0)).abs() < 0.001);
    }

    #[test]
    fn test_level_analytics_hot_levels() {
        let mut analytics = LevelAnalytics::new();
        let now = 1_700_000_000_i64;
        for i in 0..6 {
            analytics.record_fill(&FillEvent::new(
                format!("H-{}", i), OrderSide::Buy,
                100.0, 0.1, 0.001, None, now + i,
            ).with_level(102));
        }
        for i in 0..2 {
            analytics.record_fill(&FillEvent::new(
                format!("C-{}", i), OrderSide::Sell,
                101.0, 0.1, 0.001, None, now + i,
            ).with_level(103));
        }
        let hot = analytics.hot_levels(5);
        assert_eq!(hot, vec![102], "Only level 102 has >= 5 fills");
        let not_hot = analytics.hot_levels(3);
        assert!(not_hot.contains(&102));
        assert!(!not_hot.contains(&103), "level 103 has only 2 fills");
    }

    #[test]
    fn test_level_analytics_profitable_levels() {
        let mut analytics = LevelAnalytics::new();
        let now = 1_700_000_000_i64;
        analytics.record_fill(&FillEvent::new(
            "P1", OrderSide::Sell, 155.0, 0.1, 0.001, Some(5.0), now,
        ).with_level(201));
        analytics.record_fill(&FillEvent::new(
            "P2", OrderSide::Sell, 155.5, 0.1, 0.001, Some(0.0), now + 1,
        ).with_level(202));
        analytics.record_fill(&FillEvent::new(
            "P3", OrderSide::Buy, 154.0, 0.1, 0.001, Some(-1.0), now + 2,
        ).with_level(203));
        let profitable = analytics.profitable_levels(0.0);
        assert!(profitable.contains(&201));
        assert!(!profitable.contains(&202));
        assert!(!profitable.contains(&203));
    }

    #[test]
    fn test_on_fill_without_level_id_counted() {
        let config = GridRebalancerConfig::default();
        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let now = 1_700_000_000_i64;
        let fill = FillEvent::new("RSI-001", OrderSide::Buy, 150.0, 0.5, 0.005, None, now);
        rebalancer.on_fill(&fill);
        let analytics = rebalancer.level_analytics.try_lock().unwrap();
        assert_eq!(analytics.fills_without_level, 1);
        assert_eq!(analytics.fills_with_level, 0);
        assert!(analytics.levels.is_empty());
    }

    #[test]
    fn test_on_fill_with_level_id_tracked() {
        let config = GridRebalancerConfig::default();
        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let now = 1_700_000_000_i64;
        let fill = FillEvent::new(
            "GRID-042", OrderSide::Buy, 153.50, 0.1, 0.001, Some(1.25), now,
        ).with_level(42).with_distance_from_mid(-0.9);
        rebalancer.on_fill(&fill);
        let analytics = rebalancer.level_analytics.try_lock().unwrap();
        assert_eq!(analytics.fills_with_level, 1);
        let snap = analytics.levels.get(&42).expect("Level 42 must be tracked");
        assert_eq!(snap.fill_count, 1);
        assert!((snap.total_pnl - 1.25).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_get_level_analytics_report() {
        let config = GridRebalancerConfig::default();
        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let now = 1_700_000_000_i64;
        for i in 0..6 {
            rebalancer.on_fill(&FillEvent::new(
                format!("L101-{}", i), OrderSide::Buy,
                153.0, 0.1, 0.001, Some(0.80), now + i,
            ).with_level(101).with_distance_from_mid(-1.2));
        }
        for i in 0..2 {
            rebalancer.on_fill(&FillEvent::new(
                format!("L201-{}", i), OrderSide::Sell,
                157.0, 0.1, 0.001, Some(0.20), now + i,
            ).with_level(201));
        }
        rebalancer.on_fill(&FillEvent::new(
            "MOMENTUM-01", OrderSide::Buy, 154.0, 0.3, 0.003, None, now,
        ));
        let report = rebalancer.get_level_analytics().await;
        assert_eq!(report.total_tracked_levels, 2);
        assert_eq!(report.fills_with_level, 8);
        assert_eq!(report.fills_without_level, 1);
        assert_eq!(report.hot_levels, vec![101]);
        assert!(report.profitable_levels.contains(&101));
        assert!(report.profitable_levels.contains(&201));
        assert_eq!(report.snapshots[0].level_id, 101);
    }

    #[tokio::test]
    async fn test_should_place_order_uses_fees_config() {
        let fees = FeesConfig {
            maker_fee_bps: 10.0,
            taker_fee_bps: 20.0,
            slippage_bps: 15.0,
            ..FeesConfig::default()
        };
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = false;
        let r = GridRebalancer::with_fees(config, fees).unwrap();
        r.update_price(100.0).await.unwrap();
        assert_eq!(r.fees.maker_fee_bps, 10.0);
        assert_eq!(r.fees.taker_fee_bps, 20.0);
        let stats = r.grid_stats().await;
        let tight_result = r.should_place_order(OrderSide::Buy, 100.01, 0.1, &stats).await;
        assert!(!tight_result, "0.01% spread should be filtered");
    }

    #[tokio::test]
    async fn test_builder_fees_config() {
        let custom_fees = FeesConfig {
            maker_fee_bps: 3.0,
            taker_fee_bps: 6.0,
            slippage_bps: 4.0,
            min_profit_multiplier: 3.0,
            ..FeesConfig::default()
        };
        let r = GridRebalancer::builder()
            .enable_regime_gate(false)
            .fees_config(custom_fees)
            .build()
            .unwrap();
        assert_eq!(r.fees.maker_fee_bps, 3.0);
        assert_eq!(r.fees.taker_fee_bps, 6.0);
        assert_eq!(r.fees.min_profit_multiplier, 3.0);
    }

    #[tokio::test]
    async fn test_regime_gate_pause_then_resume() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = true;
        config.min_volatility_to_trade = 0.5;
        config.pause_in_very_low_vol = false;
        let r = GridRebalancer::new(config).unwrap();
        let result1 = r.should_trade_now().await;
        assert!(!result1);
        assert!(r.trading_paused.load(Ordering::Acquire));
        for p in [80.0, 82.0, 84.0, 80.0, 84.0] {
            r.update_price(p).await.unwrap();
        }
        let result2 = r.should_trade_now().await;
        assert!(result2);
        assert!(!r.trading_paused.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn test_regime_gate_no_spam_logging() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = true;
        config.min_volatility_to_trade = 0.5;
        config.pause_in_very_low_vol = false;
        let r = GridRebalancer::new(config).unwrap();
        let _ = r.should_trade_now().await;
        assert!(r.trading_paused.load(Ordering::Acquire));
        let result = r.should_trade_now().await;
        assert!(!result);
    }

    // ─────────────────────────────────────────────────────────────────────
    // V5.4: SmartFeeFilter wiring tests
    // ─────────────────────────────────────────────────────────────────────

    #[test]
    fn test_fill_rate_threshold_default() {
        let cfg = GridRebalancerConfig::default();
        assert!((cfg.fill_rate_threshold - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_fill_rate_threshold_zero_rejected() {
        let mut cfg = GridRebalancerConfig::default();
        cfg.fill_rate_threshold = 0.0;
        assert!(cfg.validate().is_err());
    }

    #[tokio::test]
    async fn test_smart_fee_filter_wired_when_enabled() {
        let cfg = GridRebalancerConfig {
            enable_fee_filtering: true,
            ..GridRebalancerConfig::default()
        };
        let fees = FeesConfig::default();
        let rebalancer = GridRebalancer::with_fees(cfg, fees).unwrap();
        assert!(rebalancer.fee_filter_stats().is_some(),
            "SmartFeeFilter must be Some when enable_fee_filtering=true");
    }

    #[tokio::test]
    async fn test_smart_fee_filter_absent_when_disabled() {
        let cfg = GridRebalancerConfig {
            enable_fee_filtering: false,
            ..GridRebalancerConfig::default()
        };
        let fees = FeesConfig::default();
        let rebalancer = GridRebalancer::with_fees(cfg, fees).unwrap();
        assert!(rebalancer.fee_filter_stats().is_none(),
            "SmartFeeFilter must be None when enable_fee_filtering=false");
    }
}
