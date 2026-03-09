//! ═══════════════════════════════════════════════════════════════════════════
//! 🤖 GRIDZBOTZ V5.7 — PRODUCTION GRID TRADING BOT
//!
//! High-performance Rust implementation with:
//! • Dynamic grid repositioning
//! • Multi-strategy consensus engine (MACD, RSI, Mean Reversion)
//! • Engine factory (paper ↔ live from config)
//! • impl Bot for GridBot (GAP-1 resolved — PR #84)
//! • Box<dyn Bot> dispatch + process_tick() (PR #85)
//! • Real-time risk management
//! • Market regime detection
//! • Automatic order lifecycle management
//! • Technical indicators library (ATR, MACD, EMA, SMA)
//!
//! Built for production trading on Solana DEX (Jupiter V6)
//!
//! Architecture:
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      GridBot (Orchestrator)                     │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Config  │  Trading  │  Strategies  │  Risk  │  Metrics  │ DEX │
//! │          │  Engine   │  Indicators  │        │           │     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! Version: 5.7.0
//! License: MIT
//! Date: March 9, 2026
//! ═══════════════════════════════════════════════════════════════════════════

#![allow(missing_docs)]
#![allow(missing_debug_implementations)]

#![allow(dead_code)]
#![allow(clippy::empty_line_after_doc_comments)]
#![allow(clippy::manual_range_contains)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::if_same_then_else)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::should_implement_trait)]
#![allow(clippy::derivable_impls)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::len_zero)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::needless_return)]
#![allow(unused_qualifications)]
#![allow(single_use_lifetimes)]

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

pub use bots::{GridBot, Bot};

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

pub const VERSION: &str  = env!("CARGO_PKG_VERSION");
pub const NAME:    &str  = env!("CARGO_PKG_NAME");
pub const CODENAME: &str = "GRIDZBOTZ V5.7 — Production Grid Trading";

pub const BUILD_INFO: BuildInfo = BuildInfo {
    version:      VERSION,
    name:         NAME,
    codename:     CODENAME,
    git_hash:     "v5.7-box-dyn-bot",
    build_date:   "2026-03-09",
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

// ═══════════════════════════════════════════════════════════════════════════
// Library Initialization
// ═══════════════════════════════════════════════════════════════════════════

/// Initialize the trading bot library.
///
/// # Examples
///
/// ```no_run
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     solana_grid_bot::init()?;
///     Ok(())
/// }
/// ```
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    Ok(())
}

pub fn init_with_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    config.validate()?;
    println!("✅ Configuration validated successfully!");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Display & Utility
// ═══════════════════════════════════════════════════════════════════════════

pub fn print_startup_banner() {
    let border = "═".repeat(70);
    println!("\n{}", border);
    println!("  🤖 GRIDZBOTZ V5.7 — Production Grid Trading");
    println!("{}", border);
    println!("  💪 Built with Rust for MAXIMUM PERFORMANCE!");
    println!("  🎯 Box<dyn Bot> dispatch · process_tick() · GAP-1 Complete");
    println!("  🔥 MACD · RSI · Mean Reversion · Grid · Consensus");
    println!("  📦 Version:  {}", VERSION);
    println!("  🏗️  Build:    {} ({})", BUILD_INFO.build_date, BUILD_INFO.git_hash);
    println!("  🦀 Rust:     {}", BUILD_INFO.rust_version);
    println!("{}\n", border);
}

pub fn print_build_info() {
    println!("Build Information:");
    println!("  Version:      {}", BUILD_INFO.version);
    println!("  Name:         {}", BUILD_INFO.name);
    println!("  Codename:     {}", BUILD_INFO.codename);
    println!("  Git Hash:     {}", BUILD_INFO.git_hash);
    println!("  Build Date:   {}", BUILD_INFO.build_date);
    println!("  Rust Version: {}", BUILD_INFO.rust_version);
}

pub fn version() -> &'static str { VERSION }
pub fn version_string() -> String { format!("{} v{} ({})", NAME, VERSION, CODENAME) }
pub fn is_test_mode()  -> bool { cfg!(test) }
pub fn is_debug_mode() -> bool { cfg!(debug_assertions) }
pub fn has_backtrace() -> bool { std::env::var("RUST_BACKTRACE").is_ok() }

// ═══════════════════════════════════════════════════════════════════════════
// Prelude
// ═══════════════════════════════════════════════════════════════════════════

/// Prelude module for convenient imports.
///
/// # Examples
///
/// ```no_run
/// use solana_grid_bot::prelude::*;
/// use solana_grid_bot::trading::PaperTradingEngine;
/// use solana_grid_bot::trading::PriceFeed;
/// use std::sync::Arc;
///
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = Config::from_file("config/master.toml")?;
///
///     // V5.7: initialize_components returns Box<dyn Bot>
///     let engine = Arc::new(PaperTradingEngine::new(10_000.0, 5.0));
///     let price_history_size = config.trading.volatility_window as usize;
///     let feed = Arc::new(PriceFeed::new(price_history_size));
///     let mut bot: Box<dyn Bot> = Box::new(GridBot::new(config, engine, feed)?);
///     bot.initialize().await?;
///
///     Ok(())
/// }
/// ```
pub mod prelude {
    pub use crate::{
        Config, GridBot, Bot, init, version,
    };
    pub use crate::trading::{
        OrderSide, OrderType, Order,
    };
    pub use crate::strategies::Strategy;
    pub use crate::indicators::{
        Indicator, ATR, MACD, EMA, SMA,
    };
    pub use anyhow::{Result, Context};
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() { assert!(init().is_ok()); }

    #[test]
    fn test_version() { assert!(!version().is_empty()); }

    #[test]
    fn test_version_string() {
        let s = version_string();
        assert!(s.contains(VERSION));
        assert!(s.contains("GRIDZBOTZ V5.7"));
    }

    #[test]
    fn test_build_info() {
        assert!(!BUILD_INFO.version.is_empty());
        assert!(!BUILD_INFO.name.is_empty());
        assert_eq!(BUILD_INFO.git_hash, "v5.7-box-dyn-bot");
    }

    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;
        let _ver = version();
    }
}

#[cfg(doctest)]
mod doctests {
    /// ```no_run
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     solana_grid_bot::init()?;
    ///     println!("Version: {}", solana_grid_bot::version());
    ///     Ok(())
    /// }
    /// ```
    fn _documentation_example() {}
}
