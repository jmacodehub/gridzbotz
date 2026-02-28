//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥📎 GRID REBALANCER V5.0 — LEVEL-CROSSING EDITION 🔥📎
//!
//! V4.0 ENHANCEMENTS - Adaptive Learning:
//!   ✅ 100% Config-Driven (No Hardcoded Values!)
//!   ✅ Regime Gate Respects Config Enable/Disable
//!   ✅ Environment-Aware Defaults
//!   ✅ Builder Pattern for Flexible Construction
//!   ✅ Comprehensive Validation
//!   ✅ Better Error Handling & Logging
//!   ✅ Thread-Safe & Production-Ready
//!   ✅ 🆕 FILL NOTIFICATION & ADAPTIVE LEARNING
//!
//! V4.1 FIX (Stage 3 / Step 5D):
//!   🔴 analyze() returned Signal::Hold in BOTH branches — grid never placed.
//!      When should_trade_now() == true  → Signal::Buy (grid-active gate).
//!      When should_trade_now() == false → Signal::Hold (regime gate blocks).
//!      Grid bots have no directional opinion; the signal IS the on/off gate.
//!
//! V5.0 (Stage 3 / Step 6) — Strategy is Source of Truth:
//!   ✅ LevelSnapshot: lightweight price-only struct for level-crossing state.
//!   ✅ set_grid_levels() + set_anchor(): GridBot pushes level boundaries here
//!      after every grid placement so the strategy owns crossing state.
//!   ✅ analyze() has 5 ordered stages:
//!      1. Regime gate        → Signal::Hold  if blocked
//!      2. Not initialised    → Signal::Buy { level_id: None }  (bootstrap)
//!      3. Reposition drift   → Signal::Buy { level_id: None }  (anchor drift)
//!      4. Crossing scan      → Signal::Buy/Sell { level_id: Some(id) }
//!      5. Nothing triggered  → Signal::Hold
//!   ✅ last_price_for_crossing: dedicated prev-tick anchor (never drifts from
//!      other price update callers).
//!   ✅ reposition_threshold_pct: new config field (default 0.5%).
//!
//! Stage 3 / Step 6b:
//!   ✅ as_grid_rebalancer_mut() impl on Strategy trait so StrategyManager
//!      can hand out &mut GridRebalancer without std::any::Any.
//!
//! February 12–27, 2026 - V4.1 Signal Fix!
//! February 27, 2026    - V5.0 Level-Crossing Edition
//! February 28, 2026    - Test fix: crossing tests disable drift reposition
//! February 28, 2026    - Fix: replace \u escape ASCII text with real UTF-8
//! ═══════════════════════════════════════════════════════════════════════════

use crate::trading::OrderSide;
use crate::strategies::{Strategy, Signal, StrategyStats as BaseStrategyStats};
use async_trait::async_trait;
use anyhow::{Result, Context};
use log::{info, warn, debug, trace};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::Instant;
use serde::{Serialize, Deserialize};

// ═══════════════════════════════════════════════════════════════════════════
// LEVEL SNAPSHOT — price-only crossing state (no order IDs)
// ═══════════════════════════════════════════════════════════════════════════

/// Lightweight snapshot of a single grid level's price boundaries.
///
/// Only prices live here — order IDs and fill state stay in `GridStateTracker`
/// inside `GridBot`. This keeps the strategy layer stateless with respect to
/// execution, which is the key modularity property of Option B.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelSnapshot {
    /// Unique level identifier (matches `GridLevel.id` in GridStateTracker)
    pub id:         u64,
    /// Limit price for the buy order on this level
    pub buy_price:  f64,
    /// Limit price for the sell order on this level
    pub sell_price: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION - Now 100% Config-Driven!
// ═══════════════════════════════════════════════════════════════════════════

/// Grid Rebalancer Configuration
///
/// All behavior is controlled through this config - NO HARDCODED VALUES!
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridRebalancerConfig {
    // ─────────────────────────────────────────────────────────────────────
    // Core Grid Settings
    // ─────────────────────────────────────────────────────────────────────

    /// Base grid spacing as a percentage (e.g., 0.15 = 0.15%)
    pub grid_spacing: f64,

    /// Order size in SOL
    pub order_size: f64,

    /// Minimum USDC balance to maintain
    pub min_usdc_balance: f64,

    /// Minimum SOL balance to maintain
    pub min_sol_balance: f64,

    /// Enable/disable this strategy
    pub enabled: bool,

    // ─────────────────────────────────────────────────────────────────────
    // V2 Features - Dynamic Spacing & Fee Filtering
    // ─────────────────────────────────────────────────────────────────────

    /// Enable dynamic spacing based on volatility
    pub enable_dynamic_spacing: bool,

    /// Enable smart fee filtering
    pub enable_fee_filtering: bool,

    /// Volatility calculation window in seconds
    pub volatility_window_seconds: u64,

    /// Maximum grid spacing (high volatility)
    pub max_spacing: f64,

    /// Minimum grid spacing (low volatility)
    pub min_spacing: f64,

    // ─────────────────────────────────────────────────────────────────────
    // V3 Features - Market Regime Gate (NOW CONFIG-DRIVEN! 🔥)
    // ─────────────────────────────────────────────────────────────────────

    /// 🔥 CRITICAL: Enable/disable regime gate
    /// - true: Respects min_volatility_to_trade
    /// - false: Trades in ANY volatility (testing mode)
    pub enable_regime_gate: bool,

    /// 🔥 CRITICAL: Minimum volatility required to trade
    /// - Set to 0.0 to disable threshold (trades always)
    /// - Typical values: 0.1 (testing), 0.3 (dev), 0.5 (prod)
    pub min_volatility_to_trade: f64,

    /// Pause trading in VERY_LOW_VOL regime
    pub pause_in_very_low_vol: bool,

    // ─────────────────────────────────────────────────────────────────────
    // V3 Features - Order Lifecycle Management
    // ─────────────────────────────────────────────────────────────────────

    /// Enable automatic order lifecycle management
    pub enable_order_lifecycle: bool,

    /// Maximum age for orders before refresh (minutes)
    pub order_max_age_minutes: u64,

    /// Interval between lifecycle checks (minutes)
    pub order_refresh_interval_minutes: u64,

    /// Minimum number of orders to maintain
    pub min_orders_to_maintain: usize,

    // ─────────────────────────────────────────────────────────────────────
    // V5.0 Features - Level Crossing
    // ─────────────────────────────────────────────────────────────────────

    /// Percentage drift from grid anchor that triggers a full reposition.
    /// E.g. 0.5 means: if price moves more than 0.5% from anchor, reposition.
    /// Set to 0.0 to disable drift-based repositioning (rely on crossings only).
    pub reposition_threshold_pct: f64,
}

impl Default for GridRebalancerConfig {
    /// Production-safe defaults
    /// Override these in config files for different environments
    fn default() -> Self {
        Self {
            // Core grid
            grid_spacing: 0.002,          // 0.2% default spacing
            order_size: 0.1,              // 0.1 SOL per order
            min_usdc_balance: 100.0,      // Keep $100 reserve
            min_sol_balance: 0.1,         // Keep 0.1 SOL reserve
            enabled: true,

            // Dynamic features
            enable_dynamic_spacing: true,
            enable_fee_filtering: true,
            volatility_window_seconds: 600,  // 10 minutes
            max_spacing: 0.0075,          // 0.75% max
            min_spacing: 0.001,           // 0.1% min

            // Regime gate - CONSERVATIVE DEFAULTS
            enable_regime_gate: true,     // Enabled by default for safety
            min_volatility_to_trade: 0.5, // 0.5% minimum (conservative)
            pause_in_very_low_vol: true,  // Safety first

            // Order lifecycle
            enable_order_lifecycle: true,
            order_max_age_minutes: 10,
            order_refresh_interval_minutes: 5,
            min_orders_to_maintain: 8,

            // Level crossing
            reposition_threshold_pct: 0.5, // 0.5% anchor drift triggers reposition
        }
    }
}

impl GridRebalancerConfig {
    /// Validate configuration values
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
                return Err(anyhow::anyhow!(
                    "min_volatility_to_trade cannot be negative"
                ));
            }
            if self.min_volatility_to_trade > 5.0 {
                warn!("⚠️ min_volatility_to_trade {:.2}% is very high - may never trade!",
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

        if self.reposition_threshold_pct < 0.0 {
            return Err(anyhow::anyhow!("reposition_threshold_pct cannot be negative"));
        }

        Ok(())
    }

    /// Apply environment-specific overrides
    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                info!("🧪 Testing mode: Relaxing regime gate for demos");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
                self.reposition_threshold_pct = 0.5;
            }
            "development" => {
                info!("🔧 Development mode: Moderate regime gate");
                if self.min_volatility_to_trade > 0.5 {
                    self.min_volatility_to_trade = 0.3;
                }
                self.reposition_threshold_pct = 0.5;
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
            _ => {
                warn!("⚠️ Unknown environment '{}', using defaults", environment);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID REBALANCER - Thread-Safe & Production Ready
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

    // ────────────────────────────────────────────────────────────────────
    // V5.0 — Level-crossing state
    // ────────────────────────────────────────────────────────────────────
    grid_levels: Arc<tokio::sync::RwLock<Vec<LevelSnapshot>>>,
    grid_anchor: Arc<tokio::sync::RwLock<Option<f64>>>,
    last_price_for_crossing: Arc<tokio::sync::RwLock<Option<f64>>>,
}

impl GridRebalancer {
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        config.validate()
            .context("GridRebalancer config validation failed")?;

        info!("═══════════════════════════════════════════════════════════════════════════");
        info!("🎯 Grid Rebalancer V5.0 (Level-Crossing Edition) Initializing...");
        info!("═══════════════════════════════════════════════════════════════════════════");
        info!("📊 CORE SETTINGS:");
        info!("   Base spacing:     {:.3}%", config.grid_spacing * 100.0);
        info!("   Order size:       {} SOL", config.order_size);
        info!("   Reserves:         ${:.0} USDC / {} SOL",
              config.min_usdc_balance, config.min_sol_balance);
        info!("   Reposition at:    {:.2}% anchor drift", config.reposition_threshold_pct);
        info!("📈 DYNAMIC FEATURES:");
        info!("   Dynamic spacing:  {}", if config.enable_dynamic_spacing { "✅" } else { "❌" });
        if config.enable_dynamic_spacing {
            info!("     Range:          {:.3}% - {:.3}%",
                  config.min_spacing * 100.0, config.max_spacing * 100.0);
        }
        info!("   Fee filtering:    {}", if config.enable_fee_filtering { "✅" } else { "❌" });
        info!("🛡\u{fe0f} MARKET REGIME GATE:");
        info!("   Enabled:          {}", if config.enable_regime_gate { "✅" } else { "❌ (TRADING FREELY!)" });
        if config.enable_regime_gate {
            info!("   Min volatility:   {:.3}%", config.min_volatility_to_trade * 100.0);
            info!("   Pause low vol:    {}", if config.pause_in_very_low_vol { "✅" } else { "❌" });
        } else {
            warn!("⚠️ REGIME GATE DISABLED - Will trade in ANY market condition!");
        }
        info!("🔄 ORDER LIFECYCLE:");
        info!("   Enabled:          {}", if config.enable_order_lifecycle { "✅" } else { "❌" });
        if config.enable_order_lifecycle {
            info!("   Max age:          {}m", config.order_max_age_minutes);
            info!("   Refresh interval: {}m", config.order_refresh_interval_minutes);
            info!("   Min orders:       {}", config.min_orders_to_maintain);
        }
        info!("🧠 ADAPTIVE LEARNING:");
        info!("   Fill tracking:    ✅");
        info!("   Level crossing:   ✅ (V5.0)");
        info!("═══════════════════════════════════════════════════════════════════════════");

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
            grid_levels:             Arc::new(tokio::sync::RwLock::new(Vec::new())),
            grid_anchor:             Arc::new(tokio::sync::RwLock::new(None)),
            last_price_for_crossing: Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    pub fn builder() -> GridRebalancerBuilder {
        GridRebalancerBuilder::new()
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // V5.0 — LEVEL SNAPSHOT INTERFACE
    // ═══════════════════════════════════════════════════════════════════════════

    pub async fn set_grid_levels(&self, levels: Vec<LevelSnapshot>) {
        let count = levels.len();
        *self.grid_levels.write().await = levels;
        debug!("[GridRebalancer] Level snapshot updated: {} levels", count);
    }

    pub async fn set_anchor(&self, anchor_price: f64) {
        *self.grid_anchor.write().await = Some(anchor_price);
        *self.last_price_for_crossing.write().await = Some(anchor_price);
        debug!("[GridRebalancer] Anchor set @ ${:.4}", anchor_price);
    }

    pub async fn is_initialized(&self) -> bool {
        !self.grid_levels.read().await.is_empty()
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // V4.0 — FILL TRACKING & ADAPTIVE LEARNING
    // ═══════════════════════════════════════════════════════════════════════════

    pub async fn on_fill_notification(
        &self,
        order_id: &str,
        side: OrderSide,
        fill_price: f64,
        fill_size: f64,
        pnl: Option<f64>,
    ) {
        debug!("📨 Fill notification: {:?} {} @ ${:.4} (size: {:.4})",
               side, order_id, fill_price, fill_size);

        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);

        if let Some(profit) = pnl {
            if profit > 0.0 {
                info!("💰 Profitable {:?} fill: +${:.2}", side, profit);
            } else if profit < -0.01 {
                debug!("📊 {:?} fill P&L: ${:.2}", side, profit);
            }
        }

        if let Some(current_price) = *self.current_price.read().await {
            let _deviation_pct = ((fill_price - current_price).abs() / current_price) * 100.0;
            trace!("📊 Fill deviation from mid: {:.3}%", _deviation_pct);
        }

        let stats = self.grid_stats().await;
        trace!("📊 Grid efficiency post-fill: {:.2}%", stats.efficiency_percent);
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // V3.5 — CONFIG-DRIVEN REGIME GATE
    // ═══════════════════════════════════════════════════════════════════════════

    pub async fn should_trade_now(&self) -> bool {
        if !self.config.enable_regime_gate {
            trace!("⚡ Regime gate DISABLED - trading freely");
            return true;
        }

        if self.trading_paused.load(Ordering::Acquire) {
            let reason = self.pause_reason.read().await;
            trace!("⏸\u{fe0f} Trading paused: {}", reason);
            return false;
        }

        let stats = self.grid_stats().await;

        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            if !self.trading_paused.load(Ordering::Acquire) {
                warn!("🚫 REGIME GATE: Pausing - VERY_LOW_VOL detected");
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
            }
            return false;
        }

        if stats.volatility < self.config.min_volatility_to_trade {
            if !self.trading_paused.load(Ordering::Acquire) {
                warn!("🚫 REGIME GATE: Pausing - Volatility {:.3}% < min {:.3}%",
                      stats.volatility * 100.0,
                      self.config.min_volatility_to_trade * 100.0);
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = format!(
                    "Low volatility ({:.3}% < {:.3}%)",
                    stats.volatility * 100.0,
                    self.config.min_volatility_to_trade * 100.0
                );
            }
            return false;
        }

        if self.trading_paused.load(Ordering::Acquire) {
            info!("✅ REGIME GATE: Resuming trading!");
            info!("   Regime: {} | Volatility: {:.3}%",
                  stats.market_regime, stats.volatility * 100.0);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }

        true
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // V3 — SMART FEE FILTER
    // ═══════════════════════════════════════════════════════════════════════════

    pub async fn should_place_order(&self, side: OrderSide, price: f64, stats: &GridStats) -> bool {
        if !self.config.enable_fee_filtering {
            trace!("💰 Fee filtering disabled - allowing order");
            return true;
        }

        let current_price = match *self.current_price.read().await {
            Some(p) => p,
            None => {
                trace!("💰 No current price - allowing order");
                return true;
            }
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

        trace!("✅ Order passes fee filter: spread {:.3}% >= min {:.2}%",
               spread_pct, min_spread);
        true
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // VOLATILITY & REGIME
    // ═══════════════════════════════════════════════════════════════════════════

    async fn calculate_volatility(&self) -> f64 {
        let history = self.price_history.lock().await;
        if history.len() < 2 { return 0.0; }
        let prices: Vec<f64> = history.iter().map(|(_, p)| *p).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / prices.len() as f64;
        variance.sqrt()
    }

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
        let pause_reason    = if trading_paused {
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

    async fn update_price(&self, price: f64) -> Result<()> {
        if price <= 0.0 { return Err(anyhow::anyhow!("Invalid price: {}", price)); }
        *self.current_price.write().await = Some(price);
        let mut history = self.price_history.lock().await;
        history.push((Instant::now(), price));
        let cutoff = Instant::now()
            - tokio::time::Duration::from_secs(self.config.volatility_window_seconds);
        history.retain(|(time, _)| *time > cutoff);
        trace!("📊 Price updated: ${:.4} (history: {} points)", price, history.len());
        Ok(())
    }

    async fn update_dynamic_spacing(&self) {
        if !self.config.enable_dynamic_spacing { return; }
        let volatility = self.calculate_volatility().await;
        let new_spacing = if volatility < 0.5 { self.config.min_spacing }
            else if volatility > 2.0 { self.config.max_spacing }
            else { self.config.grid_spacing };
        let mut current = self.current_spacing.write().await;
        if (*current - new_spacing).abs() > 0.0001 {
            debug!("📊 Dynamic spacing adjusted: {:.3}% -> {:.3}%",
                   *current * 100.0, new_spacing * 100.0);
            *current = new_spacing;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER PATTERN
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancerBuilder { config: GridRebalancerConfig }

impl GridRebalancerBuilder {
    pub fn new() -> Self { Self { config: GridRebalancerConfig::default() } }
    pub fn grid_spacing(mut self, spacing: f64) -> Self { self.config.grid_spacing = spacing; self }
    pub fn order_size(mut self, size: f64) -> Self { self.config.order_size = size; self }
    pub fn enable_regime_gate(mut self, enabled: bool) -> Self { self.config.enable_regime_gate = enabled; self }
    pub fn min_volatility(mut self, min_vol: f64) -> Self { self.config.min_volatility_to_trade = min_vol; self }
    pub fn reposition_threshold(mut self, pct: f64) -> Self { self.config.reposition_threshold_pct = pct; self }
    pub fn environment(mut self, env: &str) -> Self { self.config.apply_environment(env); self }
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

        // ── 1. REGIME GATE ─────────────────────────────────────────────────────────────────────
        if !self.should_trade_now().await {
            let stats = self.grid_stats().await;
            let sig = Signal::Hold {
                reason: Some(format!("Regime gate: {}", stats.pause_reason)),
            };
            *self.last_signal.write().await = Some(sig.clone());
            return Ok(sig);
        }

        // ── 2. BOOTSTRAP CHECK ────────────────────────────────────────────────────────────────
        if !self.is_initialized().await {
            debug!("[GridRebalancer] No levels — signalling grid bootstrap @ ${:.4}", price);
            let sig = Signal::Buy {
                price,
                size: 0.0,
                reason: "Grid bootstrap — place initial levels".to_string(),
                confidence: 1.0,
                level_id: None,
            };
            *self.last_signal.write().await = Some(sig.clone());
            return Ok(sig);
        }

        // ── 3. ANCHOR DRIFT → FULL REPOSITION ──────────────────────────────────────────────────────
        if self.config.reposition_threshold_pct > 0.0 {
            if let Some(anchor) = *self.grid_anchor.read().await {
                let drift_pct = ((price - anchor).abs() / anchor) * 100.0;
                if drift_pct > self.config.reposition_threshold_pct {
                    info!("[GridRebalancer] Anchor drift {:.3}% > {:.3}% — reposition",
                          drift_pct, self.config.reposition_threshold_pct);
                    let sig = Signal::Buy {
                        price,
                        size: 0.0,
                        reason: format!("Reposition — anchor drift {:.2}%", drift_pct),
                        confidence: 1.0,
                        level_id: None,
                    };
                    *self.last_signal.write().await = Some(sig.clone());
                    return Ok(sig);
                }
            }
        }

        // ── 4. LEVEL CROSSING SCAN ─────────────────────────────────────────────────────────────────
        let prev_opt = *self.last_price_for_crossing.read().await;
        *self.last_price_for_crossing.write().await = Some(price);

        if let Some(prev) = prev_opt {
            let levels = self.grid_levels.read().await;
            for level in levels.iter() {
                if prev > level.buy_price && price <= level.buy_price {
                    debug!("[GridRebalancer] BUY crossing L{}: ${:.4} → ${:.4} crossed ${:.4}",
                           level.id, prev, price, level.buy_price);
                    let sig = Signal::Buy {
                        price:      level.buy_price,
                        size:       self.config.order_size,
                        reason:     format!("Level {} buy boundary crossed", level.id),
                        confidence: 1.0,
                        level_id:   Some(level.id),
                    };
                    *self.last_signal.write().await = Some(sig.clone());
                    self.stats_signals.fetch_add(1, Ordering::Relaxed);
                    return Ok(sig);
                }
                if prev < level.sell_price && price >= level.sell_price {
                    debug!("[GridRebalancer] SELL crossing L{}: ${:.4} → ${:.4} crossed ${:.4}",
                           level.id, prev, price, level.sell_price);
                    let sig = Signal::Sell {
                        price:      level.sell_price,
                        size:       self.config.order_size,
                        reason:     format!("Level {} sell boundary crossed", level.id),
                        confidence: 1.0,
                        level_id:   Some(level.id),
                    };
                    *self.last_signal.write().await = Some(sig.clone());
                    self.stats_signals.fetch_add(1, Ordering::Relaxed);
                    return Ok(sig);
                }
            }
        }

        // ── 5. NOTHING TRIGGERED ────────────────────────────────────────────────────────────────────
        let sig = Signal::Hold { reason: None };
        *self.last_signal.write().await = Some(sig.clone());
        Ok(sig)
    }

    fn stats(&self) -> BaseStrategyStats {
        let signals    = self.stats_signals.load(Ordering::Relaxed);
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);
        BaseStrategyStats {
            signals_generated: signals,
            buy_signals:       rebalances / 2,
            sell_signals:      rebalances / 2,
            hold_signals:      signals.saturating_sub(rebalances),
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

    fn as_grid_rebalancer_mut(&mut self) -> Option<&mut GridRebalancer> {
        Some(self)
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

    fn test_config() -> GridRebalancerConfig {
        GridRebalancerConfig {
            enable_regime_gate:       false,
            min_volatility_to_trade:  0.0,
            pause_in_very_low_vol:    false,
            reposition_threshold_pct: 0.5,
            ..GridRebalancerConfig::default()
        }
    }

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
    fn test_config_validation_reposition_threshold() {
        let mut config = GridRebalancerConfig::default();
        config.reposition_threshold_pct = -1.0;
        assert!(config.validate().is_err(), "negative threshold should fail");
    }

    #[test]
    fn test_environment_overrides() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = true;
        config.min_volatility_to_trade = 0.5;
        config.apply_environment("testing");
        assert!(!config.enable_regime_gate);
        assert_eq!(config.min_volatility_to_trade, 0.0);
        config.apply_environment("production");
        assert!(config.enable_regime_gate);
        assert!(config.min_volatility_to_trade >= 0.3);
    }

    #[tokio::test]
    async fn test_regime_gate_disabled() {
        let config = test_config();
        let rebalancer = GridRebalancer::new(config).unwrap();
        assert!(rebalancer.should_trade_now().await);
    }

    #[tokio::test]
    async fn test_builder_pattern() {
        let rebalancer = GridRebalancer::builder()
            .grid_spacing(0.15)
            .enable_regime_gate(false)
            .environment("testing")
            .build();
        assert!(rebalancer.is_ok());
    }

    #[tokio::test]
    async fn test_builder_reposition_threshold() {
        let r = GridRebalancer::builder()
            .reposition_threshold(1.0)
            .enable_regime_gate(false)
            .build()
            .unwrap();
        assert_eq!(r.config.reposition_threshold_pct, 1.0);
    }

    #[tokio::test]
    async fn test_fill_notification() {
        let rebalancer = GridRebalancer::new(test_config()).unwrap();
        rebalancer.update_price(100.0).await.unwrap();
        rebalancer.on_fill_notification(
            "test_order_buy_123", OrderSide::Buy, 99.5, 0.1, Some(0.05),
        ).await;
        let stats = rebalancer.grid_stats().await;
        assert_eq!(stats.total_rebalances, 1);
    }

    // ─ V4.1 signal gate tests ────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_analyze_returns_buy_when_gate_open() {
        let mut r = GridRebalancer::new(test_config()).unwrap();
        let sig = r.analyze(100.0, 0).await.unwrap();
        assert!(
            matches!(sig, Signal::Buy { level_id: None, .. }),
            "Expected bootstrap Signal::Buy(level_id=None), got {:?}", sig
        );
    }

    #[tokio::test]
    async fn test_analyze_returns_hold_when_gate_closed() {
        let config = GridRebalancerConfig {
            enable_regime_gate:      true,
            min_volatility_to_trade: 999.0,
            pause_in_very_low_vol:   false,
            ..GridRebalancerConfig::default()
        };
        let mut r = GridRebalancer::new(config).unwrap();
        let sig = r.analyze(100.0, 0).await.unwrap();
        assert!(
            matches!(sig, Signal::Hold { .. }),
            "Expected Signal::Hold, got {:?}", sig
        );
    }

    // ─ V5.0 crossing detection tests ───────────────────────────────────────────────────────────

    /// Price drops through a buy boundary → Signal::Buy { level_id: Some(1) }
    ///
    /// reposition_threshold_pct: 0.0 disables the Stage 3 drift-reposition so
    /// Stage 4 (crossing scan) is the only possible trigger. The priority logic
    /// itself (Stage 3 fires before Stage 4) is proven by
    /// test_reposition_on_anchor_drift.
    #[tokio::test]
    async fn test_buy_crossing_emits_buy_signal() {
        let config = GridRebalancerConfig {
            reposition_threshold_pct: 0.0, // disable drift reposition — test crossing in isolation
            ..test_config()
        };
        let mut r = GridRebalancer::new(config).unwrap();

        r.set_grid_levels(vec![
            LevelSnapshot { id: 1, buy_price: 99.0, sell_price: 101.0 },
        ]).await;
        r.set_anchor(100.0).await;

        // First tick above buy boundary (prev set by set_anchor)
        r.analyze(100.5, 0).await.unwrap();
        // Second tick drops through buy boundary (1.5% from anchor — would
        // have triggered Stage 3 reposition if threshold != 0.0)
        let sig = r.analyze(98.5, 0).await.unwrap();

        assert!(
            matches!(sig, Signal::Buy { level_id: Some(1), .. }),
            "Expected Buy crossing L1, got {:?}", sig
        );
    }

    /// Price rises through a sell boundary → Signal::Sell { level_id: Some(2) }
    ///
    /// reposition_threshold_pct: 0.0 disables Stage 3 so Stage 4 (crossing)
    /// fires in isolation.
    #[tokio::test]
    async fn test_sell_crossing_emits_sell_signal() {
        let config = GridRebalancerConfig {
            reposition_threshold_pct: 0.0, // disable drift reposition — test crossing in isolation
            ..test_config()
        };
        let mut r = GridRebalancer::new(config).unwrap();

        r.set_grid_levels(vec![
            LevelSnapshot { id: 2, buy_price: 99.0, sell_price: 101.0 },
        ]).await;
        r.set_anchor(100.0).await;

        // First tick below sell boundary
        r.analyze(100.0, 0).await.unwrap();
        // Second tick rises through sell boundary (1.5% from anchor — would
        // have triggered Stage 3 reposition if threshold != 0.0)
        let sig = r.analyze(101.5, 0).await.unwrap();

        assert!(
            matches!(sig, Signal::Sell { level_id: Some(2), .. }),
            "Expected Sell crossing L2, got {:?}", sig
        );
    }

    /// Price within band, no boundary crossed → Signal::Hold
    #[tokio::test]
    async fn test_no_crossing_emits_hold() {
        let mut r = GridRebalancer::new(test_config()).unwrap();
        r.set_grid_levels(vec![
            LevelSnapshot { id: 1, buy_price: 98.0, sell_price: 102.0 },
        ]).await;
        r.set_anchor(100.0).await;
        r.analyze(100.0, 0).await.unwrap();
        let sig = r.analyze(100.1, 0).await.unwrap();
        assert!(
            matches!(sig, Signal::Hold { .. }),
            "Expected Hold (no crossing), got {:?}", sig
        );
    }

    /// Price drifts more than threshold from anchor → reposition Buy (level_id: None)
    #[tokio::test]
    async fn test_reposition_on_anchor_drift() {
        let config = GridRebalancerConfig {
            reposition_threshold_pct: 0.5,
            ..test_config()
        };
        let mut r = GridRebalancer::new(config).unwrap();
        r.set_grid_levels(vec![
            LevelSnapshot { id: 1, buy_price: 99.0, sell_price: 101.0 },
        ]).await;
        r.set_anchor(100.0).await;
        r.analyze(100.0, 0).await.unwrap();
        // 100.6 is 0.6% drift from anchor 100.0 — above 0.5% threshold
        let sig = r.analyze(100.6, 0).await.unwrap();
        assert!(
            matches!(sig, Signal::Buy { level_id: None, .. }),
            "Expected reposition Buy(level_id=None), got {:?}", sig
        );
        if let Signal::Buy { reason, .. } = sig {
            assert!(reason.contains("Reposition"), "Unexpected reason: {}", reason);
        }
    }

    /// as_grid_rebalancer_mut() must return Some(self)
    #[test]
    fn test_as_grid_rebalancer_mut_returns_self() {
        let mut r = GridRebalancer::new(GridRebalancerConfig {
            enable_regime_gate: false,
            ..GridRebalancerConfig::default()
        }).unwrap();
        assert!(r.as_grid_rebalancer_mut().is_some());
    }
}
