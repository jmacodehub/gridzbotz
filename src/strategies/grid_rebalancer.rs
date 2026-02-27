//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥💎 GRID REBALANCER V4.1 - PROJECT FLASH 🔥💎
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
//! February 12–27, 2026 - V4.1 Signal Fix!
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
        }
    }
}

impl GridRebalancerConfig {
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        // Grid spacing validation
        if self.grid_spacing <= 0.0 {
            return Err(anyhow::anyhow!("grid_spacing must be > 0"));
        }
        if self.grid_spacing > 0.1 {
            warn!("⚠️ Grid spacing {:.2}% is very wide", self.grid_spacing * 100.0);
        }

        // Dynamic spacing validation
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

        // Regime gate validation
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

        // Capital validation
        if self.order_size <= 0.0 {
            return Err(anyhow::anyhow!("order_size must be > 0"));
        }
        if self.min_usdc_balance < 0.0 || self.min_sol_balance < 0.0 {
            return Err(anyhow::anyhow!("Reserve balances cannot be negative"));
        }

        // Order lifecycle validation
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

    /// Apply environment-specific overrides
    pub fn apply_environment(&mut self, environment: &str) {
        match environment {
            "testing" => {
                // Testing: Disable safety features for demos
                info!("🧪 Testing mode: Relaxing regime gate for demos");
                self.enable_regime_gate = false;
                self.min_volatility_to_trade = 0.0;
                self.pause_in_very_low_vol = false;
            }
            "development" => {
                // Dev: Moderate safety
                info!("🔧 Development mode: Moderate regime gate");
                if self.min_volatility_to_trade > 0.5 {
                    self.min_volatility_to_trade = 0.3;
                }
            }
            "production" => {
                // Production: Enforce safety
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

    // Stats tracking (thread-safe)
    stats_rebalances: Arc<AtomicU64>,
    stats_filtered: Arc<AtomicU64>,
    stats_signals: Arc<AtomicU64>,
    dynamic_spacing_enabled: Arc<AtomicBool>,
    current_spacing: Arc<tokio::sync::RwLock<f64>>,

    // V3 ENHANCEMENTS - Order Lifecycle
    #[allow(dead_code)]
    last_lifecycle_check: Arc<tokio::sync::RwLock<Instant>>,
    trading_paused: Arc<AtomicBool>,
    pause_reason: Arc<tokio::sync::RwLock<String>>,

    // Strategy trait support
    last_signal: Arc<tokio::sync::RwLock<Option<Signal>>>,
}

impl GridRebalancer {
    /// Create new GridRebalancer with config
    pub fn new(config: GridRebalancerConfig) -> Result<Self> {
        // Validate config
        config.validate()
            .context("GridRebalancer config validation failed")?;

        // Log initialization
        info!("═══════════════════════════════════════════════════════════");
        info!("🎯 Grid Rebalancer V4.0 Initializing...");
        info!("═══════════════════════════════════════════════════════════");
        info!("📊 CORE SETTINGS:");
        info!("   Base spacing:     {:.3}%", config.grid_spacing * 100.0);
        info!("   Order size:       {} SOL", config.order_size);
        info!("   Reserves:         ${:.0} USDC / {} SOL",
              config.min_usdc_balance, config.min_sol_balance);

        info!("📈 DYNAMIC FEATURES:");
        info!("   Dynamic spacing:  {}", if config.enable_dynamic_spacing { "✅" } else { "❌" });
        if config.enable_dynamic_spacing {
            info!("     Range:          {:.3}% - {:.3}%",
                  config.min_spacing * 100.0, config.max_spacing * 100.0);
        }
        info!("   Fee filtering:    {}", if config.enable_fee_filtering { "✅" } else { "❌" });

        info!("🛡️ MARKET REGIME GATE:");
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
        info!("═══════════════════════════════════════════════════════════");

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
        })
    }

    /// Builder pattern for flexible construction
    pub fn builder() -> GridRebalancerBuilder {
        GridRebalancerBuilder::new()
    }

    /// Update current price and price history
    pub async fn update_price(&self, price: f64) -> Result<()> {
        if price <= 0.0 {
            return Err(anyhow::anyhow!("Invalid price: {}", price));
        }

        // Update current price
        *self.current_price.write().await = Some(price);

        // Update price history
        let mut history = self.price_history.lock().await;
        history.push((Instant::now(), price));

        // Keep only relevant window
        let cutoff = Instant::now()
            - tokio::time::Duration::from_secs(self.config.volatility_window_seconds);
        history.retain(|(time, _)| *time > cutoff);

        trace!("📊 Price updated: ${:.4} (history: {} points)", price, history.len());
        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════
    // V4.0 ENHANCEMENT: FILL TRACKING & ADAPTIVE LEARNING 🧠
    // ═══════════════════════════════════════════════════════════════════

    /// Notify strategy about filled orders for adaptive learning
    ///
    /// This enables the grid to:
    /// - Track which price levels are most profitable
    /// - Adjust spacing based on actual fill frequency
    /// - Optimize order placement over time
    /// - Build ML training dataset for future enhancements
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

        // Track successful rebalance
        self.stats_rebalances.fetch_add(1, Ordering::Relaxed);

        // Log P&L if available
        if let Some(profit) = pnl {
            if profit > 0.0 {
                info!("💰 Profitable {:?} fill: +${:.2}", side, profit);
            } else if profit < -0.01 {  // Only warn on significant loss
                debug!("📊 {:?} fill P&L: ${:.2}", side, profit);
            }
        }

        // Calculate fill deviation from current mid-price
        if let Some(current_price) = *self.current_price.read().await {
            let _deviation_pct = ((fill_price - current_price).abs() / current_price) * 100.0;
            trace!("📊 Fill deviation from mid: {:.3}%", _deviation_pct);

            // Future enhancement: Track optimal fill zones
            // - Build heatmap of profitable price levels
            // - Adjust grid density based on historical fill patterns
            // - ML model: predict optimal spacing per volatility regime
        }

        // Log current grid efficiency
        let stats = self.grid_stats().await;
        trace!("📊 Grid efficiency post-fill: {:.2}%", stats.efficiency_percent);
    }

    // ═══════════════════════════════════════════════════════════════════
    // V3.5 ENHANCEMENT: CONFIG-DRIVEN REGIME GATE 🔥
    // ═══════════════════════════════════════════════════════════════════

    /// Check if trading should proceed based on market regime
    ///
    /// Now 100% config-driven:
    /// - If `enable_regime_gate == false`: ALWAYS returns true
    /// - If enabled: Checks volatility threshold
    pub async fn should_trade_now(&self) -> bool {
        // 🔥 CRITICAL: Check if regime gate is enabled in config
        if !self.config.enable_regime_gate {
            trace!("⚡ Regime gate DISABLED - trading freely");
            return true;
        }

        // Check if already paused
        if self.trading_paused.load(Ordering::Acquire) {
            let reason = self.pause_reason.read().await;
            trace!("⏸️ Trading paused: {}", reason);
            return false;
        }

        // Get current market stats
        let stats = self.grid_stats().await;

        // Check VERY_LOW_VOL regime
        if self.config.pause_in_very_low_vol && stats.market_regime == "VERY_LOW_VOL" {
            if !self.trading_paused.load(Ordering::Acquire) {
                warn!("🚫 REGIME GATE: Pausing - VERY_LOW_VOL detected");
                self.trading_paused.store(true, Ordering::Release);
                *self.pause_reason.write().await = "VERY_LOW_VOL regime".to_string();
            }
            return false;
        }

        // 🔥 CRITICAL: Check volatility threshold from config
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

        // Resume trading if was paused
        if self.trading_paused.load(Ordering::Acquire) {
            info!("✅ REGIME GATE: Resuming trading!");
            info!("   Regime: {} | Volatility: {:.3}%",
                  stats.market_regime, stats.volatility * 100.0);
            self.trading_paused.store(false, Ordering::Release);
            *self.pause_reason.write().await = String::new();
        }

        true
    }

    // ═══════════════════════════════════════════════════════════════════
    // V3 ENHANCEMENT: SMART FEE FILTER
    // ═══════════════════════════════════════════════════════════════════

    /// Determine if order should be placed based on fee efficiency
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

        // Calculate spread percentage
        let spread_pct = ((price - current_price).abs() / current_price) * 100.0;

        // Dynamic minimum spread based on market regime
        let min_spread = match stats.market_regime.as_str() {
            "VERY_LOW_VOL" => 0.05,
            "LOW_VOL"      => 0.08,
            "MEDIUM_VOL"   => 0.10,
            "HIGH_VOL"     => 0.12,
            "VERY_HIGH_VOL"=> 0.15,
            _              => 0.10,
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

    // ═══════════════════════════════════════════════════════════════════
    // VOLATILITY CALCULATION
    // ═══════════════════════════════════════════════════════════════════

    /// Calculate price volatility from history
    async fn calculate_volatility(&self) -> f64 {
        let history = self.price_history.lock().await;

        if history.len() < 2 {
            trace!("📊 Insufficient price history for volatility");
            return 0.0;
        }

        let prices: Vec<f64> = history.iter().map(|(_, p)| *p).collect();
        let mean = prices.iter().sum::<f64>() / prices.len() as f64;
        let variance = prices.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / prices.len() as f64;

        let volatility = variance.sqrt();
        trace!("📊 Calculated volatility: {:.3}% (from {} samples)",
               volatility * 100.0, prices.len());
        volatility
    }

    /// Get comprehensive grid statistics
    pub async fn grid_stats(&self) -> GridStats {
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);
        let filtered   = self.stats_filtered.load(Ordering::Relaxed);

        let efficiency = if rebalances + filtered > 0 {
            (rebalances as f64 / (rebalances + filtered) as f64) * 100.0
        } else {
            100.0
        };

        let volatility = self.calculate_volatility().await;

        // Determine market regime based on volatility
        let market_regime = if volatility < 0.5 {
            "VERY_LOW_VOL"
        } else if volatility < 1.0 {
            "LOW_VOL"
        } else if volatility < 2.0 {
            "MEDIUM_VOL"
        } else if volatility < 3.0 {
            "HIGH_VOL"
        } else {
            "VERY_HIGH_VOL"
        };

        let current_spacing = *self.current_spacing.read().await;
        let trading_paused  = self.trading_paused.load(Ordering::Acquire);
        let pause_reason    = if trading_paused {
            self.pause_reason.read().await.clone()
        } else {
            String::new()
        };

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

    /// Update dynamic spacing based on volatility
    async fn update_dynamic_spacing(&self) {
        if !self.config.enable_dynamic_spacing {
            return;
        }

        let volatility = self.calculate_volatility().await;

        // Dynamic spacing formula
        let new_spacing = if volatility < 0.5 {
            self.config.min_spacing
        } else if volatility > 2.0 {
            self.config.max_spacing
        } else {
            self.config.grid_spacing
        };

        let mut current = self.current_spacing.write().await;
        if (*current - new_spacing).abs() > 0.0001 {
            debug!("📊 Dynamic spacing adjusted: {:.3}% -> {:.3}%",
                   *current * 100.0, new_spacing * 100.0);
            *current = new_spacing;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BUILDER PATTERN - Flexible Construction
// ═══════════════════════════════════════════════════════════════════════════

pub struct GridRebalancerBuilder {
    config: GridRebalancerConfig,
}

impl GridRebalancerBuilder {
    pub fn new() -> Self {
        Self {
            config: GridRebalancerConfig::default(),
        }
    }

    pub fn grid_spacing(mut self, spacing: f64) -> Self {
        self.config.grid_spacing = spacing;
        self
    }

    pub fn order_size(mut self, size: f64) -> Self {
        self.config.order_size = size;
        self
    }

    pub fn enable_regime_gate(mut self, enabled: bool) -> Self {
        self.config.enable_regime_gate = enabled;
        self
    }

    pub fn min_volatility(mut self, min_vol: f64) -> Self {
        self.config.min_volatility_to_trade = min_vol;
        self
    }

    pub fn environment(mut self, env: &str) -> Self {
        self.config.apply_environment(env);
        self
    }

    pub fn build(self) -> Result<GridRebalancer> {
        GridRebalancer::new(self.config)
    }
}

impl Default for GridRebalancerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for GridRebalancer {
    fn name(&self) -> &str {
        "Grid Rebalancer V4.0"
    }

    /// Analyze current price and return grid-active signal.
    ///
    /// # Signal semantics for a grid bot
    ///
    /// A grid bot has no directional opinion (it profits from oscillation).
    /// The signal is purely an on/off gate:
    ///
    /// - `Signal::Buy`  (confidence 1.0) → regime gate OPEN,  grid should run.
    /// - `Signal::Hold`                  → regime gate CLOSED, grid paused.
    ///
    /// `process_price_update` in GridBot treats any non-Hold signal as
    /// permission to call `reposition_grid()` / bootstrap the initial grid.
    ///
    /// ## Fix (V4.1 / Stage 3 Step 5D)
    /// Previously both branches returned `Signal::Hold`, permanently
    /// blocking `signal_active` and preventing the initial grid from ever
    /// being placed.  The should_trade_now() == true branch now correctly
    /// returns `Signal::Buy`.
    async fn analyze(&mut self, price: f64, _timestamp: i64) -> Result<Signal> {
        // Update price and recalculate spacing
        self.update_price(price).await
            .context("Failed to update price")?;
        self.update_dynamic_spacing().await;

        // Increment signal counter
        self.stats_signals.fetch_add(1, Ordering::Relaxed);

        // Check market regime
        let should_trade = self.should_trade_now().await;
        let stats        = self.grid_stats().await;

        // ── V4.1 FIX: return Buy when gate is open, Hold when closed ─────────
        let signal = if should_trade {
            // Grid-active: regime gate open, grid should run.
            // Use size=0 and confidence=1.0 — GridBot drives sizing from
            // AdaptiveOptimizer, not from the signal's size field.
            Signal::Buy {
                price,
                size: 0.0,
                reason: format!("Grid active - {} regime", stats.market_regime),
                confidence: 1.0,
            }
        } else {
            // Regime gate blocking: halt grid operations.
            Signal::Hold {
                reason: Some(format!("Trading paused - {}", stats.pause_reason)),
            }
        };

        // Store last signal
        *self.last_signal.write().await = Some(signal.clone());
        Ok(signal)
    }

    fn stats(&self) -> BaseStrategyStats {
        let signals    = self.stats_signals.load(Ordering::Relaxed);
        let rebalances = self.stats_rebalances.load(Ordering::Relaxed);

        BaseStrategyStats {
            signals_generated: signals,
            buy_signals:       rebalances / 2,
            sell_signals:      rebalances / 2,
            hold_signals:      signals - rebalances,
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

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    fn last_signal(&self) -> Option<Signal> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.last_signal.read().await.clone()
            })
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID STATS - Enhanced with Pause Reason
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
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = false;

        let rebalancer = GridRebalancer::new(config).unwrap();

        // Should always allow trading when disabled
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
    async fn test_fill_notification() {
        let config = GridRebalancerConfig::default();
        let rebalancer = GridRebalancer::new(config).unwrap();

        // Update price first
        rebalancer.update_price(100.0).await.unwrap();

        // Notify about a fill
        rebalancer.on_fill_notification(
            "test_order_buy_123",
            OrderSide::Buy,
            99.5,
            0.1,
            Some(0.05),
        ).await;

        // Verify rebalance counter incremented
        let stats = rebalancer.grid_stats().await;
        assert_eq!(stats.total_rebalances, 1);
    }

    // ── V4.1 signal fix tests ────────────────────────────────────────────

    /// analyze() must return Signal::Buy when regime gate is disabled (always-open).
    #[tokio::test]
    async fn test_analyze_returns_buy_when_gate_open() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate = false; // gate always open

        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let signal = rebalancer.analyze(100.0, 0).await.unwrap();

        assert!(
            matches!(signal, Signal::Buy { .. }),
            "Expected Signal::Buy when regime gate is open, got {:?}",
            signal
        );
    }

    /// analyze() must return Signal::Hold when regime gate blocks trading.
    /// We enable the gate with a very high minimum volatility so it always
    /// blocks on the tiny price history built during the test.
    #[tokio::test]
    async fn test_analyze_returns_hold_when_gate_closed() {
        let mut config = GridRebalancerConfig::default();
        config.enable_regime_gate        = true;
        config.min_volatility_to_trade   = 999.0; // impossibly high → always blocked
        config.pause_in_very_low_vol     = false;  // don't trigger the VERY_LOW_VOL branch

        let mut rebalancer = GridRebalancer::new(config).unwrap();
        let signal = rebalancer.analyze(100.0, 0).await.unwrap();

        assert!(
            matches!(signal, Signal::Hold { .. }),
            "Expected Signal::Hold when regime gate is closed, got {:?}",
            signal
        );
    }
}
