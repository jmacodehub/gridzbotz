//! ═══════════════════════════════════════════════════════════════════════════
//! 🤖 GRIDZBOTZ V7.0 — PRODUCTION GRID TRADING BOT
//!
//! High-performance Rust implementation with:
//! • Dynamic grid repositioning
//! • Multi-strategy consensus engine (MACD, RSI, Mean Reversion)
//! • Engine factory (paper ⇔ live from config)
//! • impl Bot for GridBot (GAP-1 resolved — PR #84)
//! • Box<dyn Bot> dispatch + process_tick() (PR #85)
//! • Multi-Bot Orchestrator V1.0 (GAP-3 resolved — PR #86)
//! • Real-time risk management
//! • Market regime detection (volatility floor + ceiling — PR #127)
//! • Automatic order lifecycle management
//! • Technical indicators library (ATR, MACD, EMA, SMA)
//!
//! Built for production trading on Solana DEX (Jupiter V6)
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │          Orchestrator (fleet) | GridBot (single)          │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Config  │  Trading  │  Strategies  │  Risk  │  Metrics  │ DEX │
//! │          │  Engine   │  Indicators  │        │           │     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Version: 7.0.0
//! License: MIT
//! Date: March 15, 2026
//! ═══════════════════════════════════════════════════════════════════════════

#![allow(missing_docs)]
#![allow(missing_debug_implementations)]
#![allow(dead_code)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::doc_overindented_list_items)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::len_zero)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::needless_return)]
#![allow(unused_qualifications)]
#![allow(single_use_lifetimes)]
// Added PR #87: orchestrator uses Arc<Mutex<Box<dyn Bot>>> which clippy flags
// as type_complexity. The BotEntry type alias in orchestrator.rs already
// documents intent; this gate prevents false positives elsewhere in the lib.
#![allow(clippy::type_complexity)]
#![deny(unsafe_code)]
#![allow(clippy::too_many_arguments)]

// ═══════════════════════════════════════════════════════════════════════════
// Module Declarations
// ═══════════════════════════════════════════════════════════════════════════

pub mod config;
pub mod trading;
pub mod strategies;
pub mod indicators;
pub mod risk;
pub mod security;
pub mod metrics;
pub mod dex;
pub mod utils;
pub mod bots;

// ═══════════════════════════════════════════════════════════════════════════
// Public API Exports
// ═══════════════════════════════════════════════════════════════════════════

// Bot trait, GridBot, and Orchestrator (V7.0 — PR #127)
pub use bots::{
    GridBot,
    Bot,
    BotStats,
    OrchestratorStats,
    Orchestrator,
    OrchestratorConfig,
    new_intent_registry,
    IntentRegistry,
};

pub use config::{
    Config, BotConfig, NetworkConfig, TradingConfig,
    StrategiesConfig, RiskConfig, PythConfig,
};

pub use trading::{
    OrderSide, OrderType, Order, OrderStatus,
};

pub use strategies::{
    Strategy, GridRebalancer,
};

pub use indicators::{
    Indicator, ATR, MACD, EMA, SMA,
};

// ═══════════════════════════════════════════════════════════════════════════
// Library Metadata
// ═══════════════════════════════════════════════════════════════════════════

pub const VERSION:  &str = env!("CARGO_PKG_VERSION");
pub const NAME:     &str = env!("CARGO_PKG_NAME");
pub const CODENAME: &str = "GRIDZBOTZ V7.0 — Forensics Edition";

pub const BUILD_INFO: BuildInfo = BuildInfo {
    version:      VERSION,
    name:         NAME,
    codename:     CODENAME,
    git_hash:     "v7.0-pre-launch-blockers",
    build_date:   "2026-03-15",
    rust_version: "1.85",
};

#[derive(Debug, Clone, Copy)]
pub struct BuildInfo {
    pub version:      &'static str,
    pub name:         &'static str,
    pub codename:     &'static str,
    pub git_hash:     &'static str,
    pub build_date:   &'static str,
    pub rust_version: &'static str,
}

// ════════════════════════════════════════════════════════════════════════════
