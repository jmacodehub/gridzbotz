//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥💎 GRID REBALANCER V5.0 - PROJECT FLASH 🔥💎
//!
//! V5.0 ENHANCEMENTS - Modular Spacing + Fill Feedback:
//!   ✅ SpacingMode enum: Fixed | VolatilityBuckets | AtrDynamic { period, multiplier }
//!   ✅ ATRDynamic wired — real regime adaptation (tightens low ATR, widens high ATR)
//!   ✅ on_fill() Strategy trait impl — fill-rate drives spacing_bias
//!   ✅ All V4.0 features preserved (regime gate, fee filter, order lifecycle)
//!
//! February 28, 2026 - V5.0 ATR + Fill Feedback!
//! ═══════════════════════════════════════════════════════════════════════════

use crate::trading::OrderSide;
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
// SPACING MODE — modular algorithm selector
// ═══════════════════════════════════════════════════════════════════════════

/// Controls which spacing algorithm GridRebalancer uses.
///
/// One enum — zero conflicting booleans. Add future algorithms as new variants.
/// Fully serde-compatible; select per-bot via TOML:
///
/// ```toml
/// spacing_mode = "volatility_buckets"
/// spacing_mode = "fixed"
/// # or with inline params:
/// # spacing_mode = { atr_dynamic = { period = 14, multiplier = 3.0 } }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpacingMode {
    /// Always use config.grid_spacing — no runtime adaptation
    Fixed,
    /// Simple 5-bucket std-dev volatility (V2 legacy, default)
    VolatilityBuckets,
    /// ATR × multiplier, clamped to [min_spacing, max_spacing]
    /// Grid tightens in low-ATR regimes, widens in high-ATR regimes.
    AtrDynamic {
        period:     usize,
        multiplier: f64,
    },
}

impl Default for SpacingMode {
    fn default() -> Self { SpacingMode::VolatilityBuckets }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION — 100% config-driven, no hardcoded values
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridRebalancerConfig {
    // ── Core Grid ──────────────────────────────────────────────────────────
    /// Base grid spacing as a fraction (e.g. 0.002 = 0.2%)
    pub grid_spacing:       f64,
    pub order_size:         f64,
    pub min_usdc_balance:   f64,
    pub min_sol_balance:    f64,
    pub enabled:            bool,

    // ── Spacing Algorithm ──────────────────────────────────────────────────
    /// Which spacing algorithm to use. Replaces enable_dynamic_spacing.
    pub spacing_mode:       SpacingMode,
    /// Upper bound for any spacing mode (fraction)
    pub max_spacing:        f64,
    /// Lower bound for any spacing mode (fraction)
    pub min_spacing:        f64,

    // ── Legacy fields (kept for backwards compat with existing configs) ────
    pub enable_dynamic_spacing:    bool,
    pub enable_fee_filtering:      bool,
    pub volatility_window_seconds: u64,

    // ── Regime Gate ────────────────────────────────────────────────────────
    pub enable_regime_gate:        bool,
    pub min_volatility_to_trade:   f64,
    pub pause_in_very_low_vol:     bool,

    // ── Order Lifecycle ────────────────────────────────────────────────────
    pub enable_order_lifecycle:           bool,
    pub order_max_age_minutes:            u64,
    pub order_refresh_interval_minutes:   u64,
    pub min_orders_to_maintain:           usize,
}

impl Default for GridRebalancerConfig {
    fn default() -> Self {
        Self {
            grid_spacing:     0.002,
            order_size:       0.1,
            min_usdc_balance: 100.0,
            min_sol_balance:  0.1,
            enabled:          true,

            spacing_mode:              SpacingMode::VolatilityBuckets,
            max_spacing:               0.0075,
            min_spacing:               0.001,
            enable_dynamic_spacing:    true,
            enable_fee_filtering:      true,
            volatility_window_seconds: 600,

            enable_regime_gate:       true,
            min_volatility_to_trade:  0.5,
            pause_in_very_low_vol:    true,

            enable_order_lifecycle:          true,
            order_max_age_minutes:           10,
            order_refresh_interval_minutes:  5,
            min_orders_to_maintain:          8,
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
        if self.min_spacing >= self.max_spacing {
            return Err(anyhow::anyhow!(
                "min_spacing ({}) must be < max_spacing ({})",
                self.min_spacing, self.max_spacing
            ));
        }
        if self.min_spacing <= 0.0 {
            return Err(anyhow::anyhow!("min_spacing must be > 0"));
        }
        if self.enable_regime_gate {
            if self.min_volatility_to_trade < 0.0 {
                return Err(anyhow::anyhow!("min_volatility_to_trade cannot be negative"));
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("⚠️ min_volatility_to_trade {:.2}% is very high — may never trade!",
                      self.min_volatility_to_trade);
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
                info!("🧪 Testing mode: Relaxing regime gate for demos");
                self.enable_regime_gate      = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol   = false;
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
// GRID REBALANCER — thread-safe, production-ready
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancer {
    config:         GridRebalancerConfig,
    current_price:  Arc<tokio::sync::RwLock<Option<f64>>>,
    price_history:  Arc<tokio::sync::Mutex<Vec<(Instant, f64)>>>,

    // Stats
    stats_rebalances:        Arc<AtomicU64>,
    stats_filtered:          Arc<AtomicU64>,
    stats_signals:           Arc<AtomicU64>,
    dynamic_spacing_enabled: Arc<AtomicBool>,
    current_spacing:         Arc<tokio::sync::RwLock<f64>>,

    // Order lifecycle
    #[allow(dead_code)]
    last_lifecycle_check: Arc<tokio::sync::RwLock<Instant>>,
    trading_paused:       Arc<AtomicBool>,
    pause_reason:         Arc<tokio::sync::RwLock<String>>,

    last_signal: Arc<tokio::sync::RwLock<Option<Signal>>>,

    // ── V5.0: ATR + Fill Feedback ─────────────────────────────────────────
    /// ATR-based spacing calculator; Some(..) when SpacingMode::AtrDynamic
    atr_dynamic:      Arc<tokio::sync::Mutex<Option<ATRDynamic>>>,
    /// Adaptive spacing bias driven by fill rate (+ve widens, -ve tightens)
    spacing_bias:     Arc<tokio::sync::RwLock<f64>>,
    /// Ring buffer of the last 20 fill timestamps (Unix seconds)
    fill_timestamps:  Arc<tokio::sync::Mutex<VecDeque<i64>>>,
}

impl GridRebalancer {
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        config.validate().context("GridRebalancer config validation failed")?;

        // Build ATR calculator when the mode requires it
        let atr: Option<ATRDynamic> = match &config.spacing_mode {
            SpacingMode::AtrDynamic { period, multiplier } => {
                let atr_cfg = ATRConfig {
                    atr_period:     *period,
                    atr_multiplier: *multiplier,
                    // ATRConfig uses %; GridRebalancerConfig stores fractions
                    min_spacing:    config.min_spacing * 100.0,
                    max_spacing:    config.max_spacing * 100.0,
                };
                info!("📐 SpacingMode::AtrDynamic  period={}  mult={}×  [{:.3}%–{:.3}%]",
                      period, multiplier,
                      config.min_spacing * 100.0, config.max_spacing * 100.0);
                Some(ATRDynamic::from_config(&atr_cfg))
            }
            SpacingMode::VolatilityBuckets => {
                info!("📐 SpacingMode::VolatilityBuckets (legacy std-dev)");
                None
            }
            SpacingMode::Fixed => {
                info!("📐 SpacingMode::Fixed @ {:.3}%", config.grid_spacing * 100.0);
                None
            }
        };

        info!("═══════════════════════════════════════════════════════════");
        info!("🎯 Grid Rebalancer V5.0 Initializing...");
        info!("═══════════════════════════════════════════════════════════");
        info!("📊 CORE:  spacing={:.3}%  size={} SOL  reserves=${:.0}/{} SOL",
              config.grid_spacing * 100.0, config.order_size,
              config.min_usdc_balance, config.min_sol_balance);
        info!("🛡️  REGIME GATE: {}  min_vol={:.3}%",
              if config.enable_regime_gate { "ON" } else { "OFF (TRADING FREELY!)" },
              config.min_volatility_to_trade * 100.0);
        info!("🔄 ORDER LIFECYCLE: {}  max_age={}m  refresh={}m",
              if config.enable_order_lifecycle { "ON" } else { "OFF" },
              config.order_max_age_minutes, config.order_refresh_interval_minutes);
        info!("🧠 ADAPTIVE: fill-rate bias ✅  ATR {}",
              if atr.is_some() { "✅ ACTIVE" } else { "❌ (not selected)" });
        info!("═══════════════════════════════════════════════════════════");

        Ok(Self {
            current_spacing: Arc::new(tokio::sync::RwLock::new(config.grid_spacing)),
            config,
            current_price:    Arc::new(tokio::sync::RwLock::new(None)),
            price_history:    Arc::new(tokio::sync::Mutex::new(Vec::new())),
            stats_rebalances: Arc::new(AtomicU64::new(0)),
            stats_filtered:   Arc::new(AtomicU64::new(0)),
            stats_signals:    Arc::new(AtomicU64::new(0)),
            dynamic_spacing_enabled: Arc::new(AtomicBool::new(true)),
            last_lifecycle_check:    Arc::new(tokio::sync::RwLock::new(Instant::now())),
            trading_paused:  Arc::new(AtomicBool::new(false)),
            pause_reason:    Arc::new(tokio::sync::RwLock::new(String::new())),
            last_signal:     Arc::new(tokio::sync::RwLock::new(None)),
            atr_dynamic:     Arc::new(tokio::sync::Mutex::new(atr)),
            spacing_bias:    Arc::new(tokio::sync::RwLock::new(0.0)),
            fill_timestamps: Arc::new(tokio::sync::Mutex::new(VecDeque::with_capacity(20))),
        })
    }

    pub fn builder() -> GridRebalancerBuilder {
        GridRebalancerBuilder::new()
    }

    /// Update current price, maintain history window, and feed ATR.
    pub async fn update_price(&self, price: f64) -> Result<()> {
        if price <= 0.0 {
            return Err(anyhow::anyhow!("Invalid price: {}", price));
        }
        *self.current_price.write().await = Some(price);

        // Maintain price history window
        {
            let mut history = self.price_history.lock().await;
            history.push((Instant::now(), price));
            let cutoff = Instant::now()
                - tokio::time::Duration::from_secs(self.config.volatility_window_seconds);
            history.retain(|(t, _)| *t > cutoff);
            trace!("📊 Price updated: ${:.4}  history={} pts", price, history.len());
        }

        // Feed ATR every tick — no-op when atr_dynamic is None
        {
            let mut atr = self.atr_dynamic.lock().await;
            if let Some(a) = atr.as_mut() {
                a.update(price);
            }
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // V4.0 LEGACY: on_fill_notification (kept for existing callers)
    // ═══════════════════════════════════════════════════════════════════

    pub async fn on_fill_notification(
        &self,
        order_id:  &str,
        side:      OrderSide,
        fill_price: f64,
        fill_size:  f64,
        pnl:        Option<f64>,
    ) {
        debug!("📨 Fill notification: {:?} {} @ ${:.4} (size: {:.4})",
               side, order_id, fill_price, fill_size);
        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);
        if let Some(profit) = pnl {
            if profit > 0.0 {
                info!("💰 Profitable {:?} fill: +${:.2}", side, profit);
            }
        }
        let stats = self.grid_stats().await;
        trace!("📊 Grid efficiency post-fill: {:.2}%", stats.efficiency_percent);
    }

    // ═══════════════════════════════════════════════════════════════════
    // REGIME GATE
    // ═══════════════════════════════════════════════════════════════════

    pub async fn should_trade_now(&self) -> bool {
        if !self.config.enable_regime_gate {
            trace!("⚡ Regime gate DISABLED — trading freely");
            return true;
        }
        if self.trading_paused.load(Ordering::Acquire) {
            let reason = self.pause_reason.read().await;
            trace!("⏸️  Trading paused: {}", reason);
            return false;
        }
        let stats = self.grid_stats().await;
        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            if !self.trading_paused.load(Ordering::Acquire) {
                warn!("🚫 REGIME GATE: Pausing — VERY_LOW_VOL detected");
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
            }
            return false;
        }
        if stats.volatility < self.config.min_volatility_to_trade {
            if !self.trading_paused.load(Ordering::Acquire) {
                warn!("🚫 REGIME GATE: Pausing — vol {:.3}% < min {:.3}%",
                      stats.volatility * 100.0, self.config.min_volatility_to_trade * 100.0);
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = format!(
                    "Low volatility ({:.3}% < {:.3}%)",
                    stats.volatility * 100.0, self.config.min_volatility_to_trade * 100.0
                );
            }
            return false;
        }
        if self.trading_paused.load(Ordering::Acquire) {
            info!("✅ REGIME GATE: Resuming!  regime={}  vol={:.3}%",
                  stats.market_regime, stats.volatility * 100.0);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }
        true
    }

    // ═══════════════════════════════════════════════════════════════════
    // FEE FILTER
    // ═══════════════════════════════════════════════════════════════════

    pub async fn should_place_order(
        &self, side: OrderSide, price: f64, stats: &GridStats,
    ) -> bool {
        if !self.config.enable_fee_filtering { return true; }
        let current_price = match *self.current_price.read().await {
            Some(p) => p,
            None    => return true,
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
            debug!("🚫 FILTERED: {:?} @ ${:.4}  spread {:.3}% < min {:.2}%",
                side, price, spread_pct, min_spread);
            self.stats_filtered.fetch_add(1, Ordering::Relaxed);
            return false;
        }
        true
    }

    // ═══════════════════════════════════════════════════════════════════
    // VOLATILITY (legacy — used by VolatilityBuckets mode)
    // ═══════════════════════════════════════════════════════════════════

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

    pub async fn grid_stats(&self) -> GridStats {
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);
        let filtered   = self.stats_filtered.load(Ordering::Relaxed);
        let efficiency = if rebalances + filtered > 0 {
            (rebalances as f64 / (rebalances + filtered) as f64) * 100.0
        } else { 100.0 };
        let volatility = self.calculate_volatility().await;
        let market_regime = if      volatility < 0.5 { "VERY_LOW_VOL" }
                            else if volatility < 1.0 { "LOW_VOL" }
                            else if volatility < 2.0 { "MEDIUM_VOL" }
                            else if volatility < 3.0 { "HIGH_VOL" }
                            else                     { "VERY_HIGH_VOL" };
        let current_spacing = *self.current_spacing.read().await;
        let trading_paused  = self.trading_paused.load(Ordering::Acquire);
        let pause_reason = if trading_paused {
            self.pause_reason.read().await.clone()
        } else { String::new() };
        GridStats {
            total_rebalances:        rebalances,
            rebalances_filtered:     filtered,
            efficiency_percent:      efficiency,
            dynamic_spacing_enabled: self.dynamic_spacing_enabled.load(Ordering::Relaxed),
            current_spacing_percent: current_spacing * 100.0,
            volatility,
            market_regime: market_regime.to_string(),
            trading_paused,
            pause_reason,
        }
    }

    // ═══════════════════════════════════════════════════════════════════
    // V5.0: SPACING — driven by SpacingMode enum
    // ═══════════════════════════════════════════════════════════════════

    async fn update_dynamic_spacing(&self) {
        let base: f64 = match &self.config.spacing_mode {
            // Fixed: nothing to compute
            SpacingMode::Fixed => return,

            // Legacy 5-bucket std-dev
            SpacingMode::VolatilityBuckets => {
                let vol = self.calculate_volatility().await;
                if      vol < 0.5 { self.config.min_spacing }
                else if vol > 2.0 { self.config.max_spacing }
                else              { self.config.grid_spacing }
            }

            // ATR-based — real regime adaptation
            SpacingMode::AtrDynamic { .. } => {
                let atr_guard   = self.atr_dynamic.lock().await;
                let price_guard = self.current_price.read().await;
                match (atr_guard.as_ref(), *price_guard) {
                    (Some(atr), Some(price)) if atr.ready() => {
                        match atr.calculate_spacing(price) {
                            // ATRDynamic returns %; convert back to fraction
                            Some(pct) => {
                                debug!("📐 ATR spacing: {:.4}%", pct);
                                pct / 100.0
                            }
                            None => self.config.grid_spacing,
                        }
                    }
                    _ => {
                        trace!("📐 ATR warming up — using base spacing");
                        self.config.grid_spacing
                    }
                }
            }
        };

        // Apply fill-rate bias on top of the computed base, then clamp
        let bias   = *self.spacing_bias.read().await;
        let biased = (base + bias)
            .max(self.config.min_spacing)
            .min(self.config.max_spacing);

        let mut current = self.current_spacing.write().await;
        if (*current - biased).abs() > 0.00001 {
            debug!("📊 Spacing: {:.4}% → {:.4}%  bias={:+.4}%",
                   *current * 100.0, biased * 100.0, bias * 100.0);
            *current = biased;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancerBuilder { config: GridRebalancerConfig }

impl GridRebalancerBuilder {
    pub fn new() -> Self { Self { config: GridRebalancerConfig::default() } }

    pub fn grid_spacing(mut self, v: f64)       -> Self { self.config.grid_spacing = v; self }
    pub fn order_size(mut self, v: f64)          -> Self { self.config.order_size  = v; self }
    pub fn enable_regime_gate(mut self, v: bool) -> Self { self.config.enable_regime_gate = v; self }
    pub fn min_volatility(mut self, v: f64)      -> Self { self.config.min_volatility_to_trade = v; self }
    pub fn spacing_mode(mut self, m: SpacingMode)-> Self { self.config.spacing_mode = m; self }
    pub fn environment(mut self, env: &str)      -> Self { self.config.apply_environment(env); self }
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
        let stats        = self.grid_stats().await;

        let signal = if !should_trade {
            Signal::Hold { reason: Some(format!("Trading paused — {}", stats.pause_reason)) }
        } else {
            Signal::Hold { reason: Some(format!("Grid active — {} regime  spacing={:.3}%",
                stats.market_regime, stats.current_spacing_percent)) }
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
        info!("🔄 Resetting GridRebalancer V5.0 stats");
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

    // ── V5.0: Fill feedback loop ─────────────────────────────────────────
    //
    // Called sync by StrategyManager::notify_fill().
    // Uses try_lock() throughout — never blocks the trading hot path.
    // Worst case on contention: one timestamp or one bias write is skipped.
    // The ring buffer and bias still converge over subsequent fills.
    fn on_fill(&mut self, fill: &crate::trading::FillEvent) {
        // 1. Record fill timestamp into ring buffer
        if let Ok(mut ts) = self.fill_timestamps.try_lock() {
            ts.push_back(fill.timestamp);
            if ts.len() > 20 { ts.pop_front(); }
        }

        // 2. Compute fills/sec over last 60 s using fill.timestamp as 'now'
        let fill_rate: f64 = {
            let now = fill.timestamp;
            match self.fill_timestamps.try_lock() {
                Ok(ts) => ts.iter().filter(|&&t| now - t <= 60).count() as f64 / 60.0,
                Err(_) => return, // contention — skip this update
            }
        };

        // 3. Adjust spacing bias
        //    HIGH (>6/min): widen  — protect edge, reduce churn
        //    LOW  (<2/min): tighten — hunt for fills in quiet market
        const HIGH: f64 = 0.10;   // fills/sec ≈ 6/min
        const LOW:  f64 = 0.033;  // fills/sec ≈ 2/min

        let old_bias = match self.spacing_bias.try_read() {
            Ok(b)  => *b,
            Err(_) => return,
        };
        let new_bias = if      fill_rate > HIGH { (old_bias + 0.0002).min( 0.002) }
                       else if fill_rate < LOW  { (old_bias - 0.0001).max(-0.001) }
                       else                     { old_bias };

        if (new_bias - old_bias).abs() > 0.000001 {
            if let Ok(mut b) = self.spacing_bias.try_write() {
                *b = new_bias;
                debug!("🧠 Fill feedback: rate={:.4}/s  bias {:+.4}% → {:+.4}%",
                       fill_rate, old_bias * 100.0, new_bias * 100.0);
            }
        }

        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);
        debug!("📨 on_fill: {:?} {} @ {:.4}", fill.side, fill.order_id, fill.fill_price);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID STATS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridStats {
    pub total_rebalances:        u64,
    pub rebalances_filtered:     u64,
    pub efficiency_percent:      f64,
    pub dynamic_spacing_enabled: bool,
    pub current_spacing_percent: f64,
    pub volatility:              f64,
    pub market_regime:           String,
    pub trading_paused:          bool,
    pub pause_reason:            String,
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
        let mut c2 = GridRebalancerConfig::default();
        c2.min_spacing = 0.2;
        c2.max_spacing = 0.1;
        assert!(c2.validate().is_err());
    }

    #[test]
    fn test_spacing_mode_default() {
        let config = GridRebalancerConfig::default();
        assert!(matches!(config.spacing_mode, SpacingMode::VolatilityBuckets));
    }

    #[test]
    fn test_environment_overrides() {
        let mut config = GridRebalancerConfig::default();
        config.apply_environment("testing");
        assert!(!config.enable_regime_gate);
        assert_eq!(config.min_volatility_to_trade, 0.0);
        config.apply_environment("production");
        assert!(config.enable_regime_gate);
        assert!(config.min_volatility_to_trade >= 0.3);
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
        config.spacing_mode       = SpacingMode::AtrDynamic { period: 3, multiplier: 2.0 };
        config.enable_regime_gate = false;
        let r = GridRebalancer::new(config).unwrap();
        // Need `period` updates to fill the ATR window
        for p in [100.0_f64, 101.0, 102.0, 103.0] {
            r.update_price(p).await.unwrap();
        }
        let ready = r.atr_dynamic.lock().await
            .as_ref().map(|a| a.ready()).unwrap_or(false);
        assert!(ready, "ATR should be ready after period+1 updates");
    }

    #[test]
    fn test_builder_spacing_mode() {
        let r = GridRebalancer::builder()
            .spacing_mode(SpacingMode::AtrDynamic { period: 14, multiplier: 3.0 })
            .enable_regime_gate(false)
            .build();
        assert!(r.is_ok());
    }

    #[test]
    fn test_builder_fixed_mode() {
        let r = GridRebalancer::builder()
            .spacing_mode(SpacingMode::Fixed)
            .enable_regime_gate(false)
            .build();
        assert!(r.is_ok());
    }

    #[tokio::test]
    async fn test_builder_pattern_legacy() {
        let r = GridRebalancer::builder()
            .grid_spacing(0.15)
            .enable_regime_gate(false)
            .environment("testing")
            .build();
        assert!(r.is_ok());
    }

    #[test]
    fn test_on_fill_high_rate_widens_bias() {
        let config = GridRebalancerConfig::default();
        let mut r = GridRebalancer::new(config).unwrap();
        // 10 fills with the same timestamp → rate = 10/60 ≈ 0.167/s (> HIGH 0.10)
        let now: i64 = 1_700_000_000;
        for i in 0..10_u64 {
            let fill = FillEvent::new(
                format!("ORDER-{:03}", i),
                OrderSide::Buy,
                142.50, 0.1, 0.0025,
                Some(0.05),
                now,
            );
            r.on_fill(&fill);
        }
        let bias = tokio::runtime::Runtime::new().unwrap()
            .block_on(async { *r.spacing_bias.read().await });
        assert!(bias > 0.0, "High fill rate should widen bias (got {})", bias);
    }

    #[test]
    fn test_on_fill_no_pnl_does_not_panic() {
        let config = GridRebalancerConfig::default();
        let mut r = GridRebalancer::new(config).unwrap();
        let fill = FillEvent::new("TEST", OrderSide::Sell, 100.0, 0.1, 0.001, None, 0);
        r.on_fill(&fill); // must not panic
    }

    #[tokio::test]
    async fn test_fill_notification_legacy() {
        let config = GridRebalancerConfig::default();
        let r = GridRebalancer::new(config).unwrap();
        r.update_price(100.0).await.unwrap();
        r.on_fill_notification("test_buy", OrderSide::Buy, 99.5, 0.1, Some(0.05)).await;
        let stats = r.grid_stats().await;
        assert_eq!(stats.total_rebalances, 1);
    }
}
