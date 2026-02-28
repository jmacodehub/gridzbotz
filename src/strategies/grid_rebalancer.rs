//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥💎 GRID REBALANCER V5.0 - ADAPTIVE SPACING + FILL FEEDBACK 🔥💎
//!
//! V5.0 ENHANCEMENTS (Stage 3 — Feb 2026):
//!   ✅ SpacingMode enum: Fixed | VolatilityBuckets | AtrDynamic
//!   ✅ on_fill() Strategy trait impl: fill-rate spacing bias
//!   ✅ ATRDynamic wired: real regime adaptation via atr_dynamic field
//!   ✅ FillState: thread-safe fill timestamp + bias in Arc<Mutex<_>>
//!   ✅ Builder gains spacing_mode() for per-bot TOML config
//!
//! February 28, 2026 - V5.0 Stage 3 Complete!
//! ═══════════════════════════════════════════════════════════════════════════

use crate::trading::{FillEvent, OrderSide};
use crate::strategies::{Strategy, Signal, StrategyStats as BaseStrategyStats};
use crate::strategies::shared::analytics::atr_dynamic::{ATRDynamic, ATRConfig};
use async_trait::async_trait;
use anyhow::{Result, Context};
use log::{info, warn, debug, trace};
use std::collections::VecDeque;
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
// FILL STATE - Thread-safe fill feedback tracking
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
// CONFIGURATION - 100% Config-Driven
// ═══════════════════════════════════════════════════════════════════════════

/// Grid Rebalancer Configuration — all behavior controlled here, no hardcoded values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridRebalancerConfig {
    // ── Core Grid ─────────────────────────────────────────────────────────
    pub grid_spacing: f64,
    pub order_size: f64,
    pub min_usdc_balance: f64,
    pub min_sol_balance: f64,
    pub enabled: bool,

    // ── V2: Dynamic Spacing ───────────────────────────────────────────────
    pub enable_dynamic_spacing: bool,
    pub enable_fee_filtering: bool,
    pub volatility_window_seconds: u64,
    pub max_spacing: f64,
    pub min_spacing: f64,

    // ── V3: Market Regime Gate ────────────────────────────────────────────
    pub enable_regime_gate: bool,
    pub min_volatility_to_trade: f64,
    pub pause_in_very_low_vol: bool,

    // ── V3: Order Lifecycle ───────────────────────────────────────────────
    pub enable_order_lifecycle: bool,
    pub order_max_age_minutes: u64,
    pub order_refresh_interval_minutes: u64,
    pub min_orders_to_maintain: usize,

    // ── V5.0: Spacing Mode ────────────────────────────────────────────────
    /// Selects the spacing algorithm. Defaults to VolatilityBuckets.
    pub spacing_mode: SpacingMode,
}

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
                warn!("⚠️ min_volatility_to_trade {:.2}% may never trade", self.min_volatility_to_trade);
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
                    warn!("⚠️ Raising min_volatility to 0.3% for production safety");
                    self.min_volatility_to_trade = 0.3;
                }
            }
            _ => warn!("⚠️ Unknown environment '{}', using defaults", environment),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID REBALANCER - V5.0
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancer {
    config: GridRebalancerConfig,
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
}

impl GridRebalancer {
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        config.validate().context("GridRebalancer config validation failed")?;

        info!("═══════════════════════════════════════════════════════════");
        info!("🎯 Grid Rebalancer V5.0 Initializing...");
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

        info!("🛡️ REGIME GATE: {} | min_vol={:.3}%",
              if config.enable_regime_gate { "✅" } else { "❌ FREE" },
              config.min_volatility_to_trade * 100.0);
        info!("🧠 ADAPTIVE: fill-feedback bias ✅");
        info!("═══════════════════════════════════════════════════════════");

        // Build ATR only when the mode requires it
        let atr_dynamic = match &config.spacing_mode {
            SpacingMode::AtrDynamic { period, multiplier } => {
                let atr_cfg = ATRConfig {
                    atr_period: *period,
                    atr_multiplier: *multiplier,
                    min_spacing: config.min_spacing * 100.0, // ATRConfig uses %
                    max_spacing: config.max_spacing * 100.0,
                };
                Some(ATRDynamic::from_config(&atr_cfg))
            }
            _ => None,
        };

        Ok(Self {
            current_spacing: Arc::new(tokio::sync::RwLock::new(config.grid_spacing)),
            config,
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

        // Feed ATR (no-op when mode is not AtrDynamic)
        let mut atr_guard = self.atr_dynamic.lock().await;
        if let Some(atr) = atr_guard.as_mut() {
            atr.update(price);
        }

        trace!("📊 Price: ${:.4} (history: {} pts)", price, history.len());
        Ok(())
    }

    // ── Regime Gate ─────────────────────────────────────────────────────────

    pub async fn should_trade_now(&self) -> bool {
        if !self.config.enable_regime_gate {
            return true;
        }
        if self.trading_paused.load(Ordering::Acquire) {
            return false;
        }
        let stats = self.grid_stats().await;
        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            self.trading_paused.store(true, Ordering::Release);
            *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
            return false;
        }
        if stats.volatility < self.config.min_volatility_to_trade {
            self.trading_paused.store(true, Ordering::Release);
            *self.pause_reason.write().await = format!(
                "Low volatility ({:.3}% < {:.3}%)",
                stats.volatility * 100.0, self.config.min_volatility_to_trade * 100.0
            );
            return false;
        }
        if self.trading_paused.load(Ordering::Acquire) {
            info!("✅ REGIME GATE: Resuming — {} / {:.3}%",
                  stats.market_regime, stats.volatility * 100.0);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }
        true
    }

    // ── Fee Filter ───────────────────────────────────────────────────────────

    pub async fn should_place_order(&self, side: OrderSide, price: f64, stats: &GridStats) -> bool {
        if !self.config.enable_fee_filtering {
            return true;
        }
        let current_price = match *self.current_price.read().await {
            Some(p) => p,
            None => return true,
        };
        let spread_pct = ((price - current_price).abs() / current_price) * 100.0;
        let min_spread = match stats.market_regime.as_str() {
            "VERY_LOW_VOL"  => 0.05,
            "LOW_VOL"       => 0.08,
            "MEDIUM_VOL"    => 0.10,
            "HIGH_VOL"      => 0.12,
            "VERY_HIGH_VOL" => 0.15,
            _               => 0.10,
        };
        if spread_pct < min_spread {
            debug!("🚫 FILTERED: {:?} @ ${:.4} (spread {:.3}% < min {:.2}%)",
                side, price, spread_pct, min_spread);
            self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }

    // ── Volatility (fallback for VolatilityBuckets) ──────────────────────────

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

    // ── Spacing Dispatch ─────────────────────────────────────────────────────

    async fn update_dynamic_spacing(&self) {
        if !self.config.enable_dynamic_spacing {
            return;
        }

        let base_spacing = match &self.config.spacing_mode {
            SpacingMode::Fixed => return, // constant — nothing to update

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
                        // calculate_spacing() returns % → convert to fraction
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

        // Apply fill-rate bias (shared by all modes)
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

    // ── Grid Stats ───────────────────────────────────────────────────────────

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
// BUILDER
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancerBuilder {
    config: GridRebalancerConfig,
}

impl GridRebalancerBuilder {
    pub fn new() -> Self { Self { config: GridRebalancerConfig::default() } }
    pub fn grid_spacing(mut self, s: f64) -> Self { self.config.grid_spacing = s; self }
    pub fn order_size(mut self, s: f64) -> Self { self.config.order_size = s; self }
    pub fn enable_regime_gate(mut self, e: bool) -> Self { self.config.enable_regime_gate = e; self }
    pub fn min_volatility(mut self, v: f64) -> Self { self.config.min_volatility_to_trade = v; self }
    pub fn environment(mut self, env: &str) -> Self { self.config.apply_environment(env); self }
    pub fn spacing_mode(mut self, mode: SpacingMode) -> Self { self.config.spacing_mode = mode; self }
    pub fn build(self) -> Result<GridRebalancer> { GridRebalancer::new(self.config) }
}

impl Default for GridRebalancerBuilder {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for GridRebalancer {
    fn name(&self) -> &str { "Grid Rebalancer V5.0" }

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

    // ── V5.0: Fill feedback loop ───────────────────────────────────────────
    /// Sync fn — uses try_lock (non-blocking). Never contended under normal load.
    fn on_fill(&mut self, fill: &FillEvent) {
        if let Ok(mut state) = self.fill_state.try_lock() {
            // Ring buffer — keep last 20 timestamps
            state.timestamps.push_back(fill.timestamp);
            if state.timestamps.len() > 20 {
                state.timestamps.pop_front();
            }

            // Fill rate over last 60 seconds
            let rate = state.fill_rate(60);
            const HIGH_FILL_THR: f64 = 0.10; // > 6 fills/min = hot market

            let old_bias = state.bias;
            if rate > HIGH_FILL_THR {
                state.bias = (state.bias + 0.0002).min(0.002);
            } else if rate < HIGH_FILL_THR * 0.3 {
                state.bias = (state.bias - 0.0001).max(-0.001);
            }

            if (state.bias - old_bias).abs() > 0.000001 {
                info!("🧠 Fill feedback: rate={:.3}/s bias {:+.4}% → {:+.4}%",
                    rate, old_bias * 100.0, state.bias * 100.0);
            }
            debug!("📨 on_fill: {:?} {} @ {:.4} | bias {:+.4}%",
                fill.side, fill.order_id, fill.fill_price, state.bias * 100.0);
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
        // 10 fills in quick succession pushes rate above threshold
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
}
