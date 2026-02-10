//! ═══════════════════════════════════════════════════════════════════════════
//! 🤖 SOLANA GRID TRADING BOT - MULTI-STRATEGY V4.0 "CONSERVATIVE AI"
//! 
//! High-performance Rust implementation with:
//! • Dynamic grid repositioning
//! • Multi-strategy consensus engine (MACD, RSI, Mean Reversion)
//! • Real-time risk management
//! • Market regime detection
//! • Automatic order lifecycle management
//! • Technical indicators library (ATR, MACD, EMA, SMA)
//! 
//! Built for production trading on Solana DEX (OpenBook/Serum)
//! 
//! Architecture:
//! ```
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      GridBot (Orchestrator)                     │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Config  │  Trading  │  Strategies  │  Risk  │  Metrics  │ DEX │
//! │          │           │  Indicators  │        │           │     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//! 
//! Version: 4.0.0
//! License: MIT
//! Date: February 10, 2026
//! ═══════════════════════════════════════════════════════════════════════════

#![allow(missing_docs)] 
#![allow(missing_debug_implementations)]

// ═══════════════════════════════════════════════════════════════════════════
//Standard Library & External Dependencies
// ═══════════════════════════════════════════════════════════════════════════

#![warn(
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![allow(clippy::too_many_arguments)]

// ═══════════════════════════════════════════════════════════════════════════
// Module Declarations - Organized by Domain
//═══════════════════════════════════════════════════════════════════════════

/// Configuration management (TOML-based + programmatic)
pub mod config;

/// Trading engine (paper trading, order management, execution)
pub mod trading;

/// Strategy implementations (grid, momentum, RSI, mean reversion)
pub mod strategies;

/// Technical indicators (ATR, MACD, EMA, SMA) - NEW in v4.0!
pub mod indicators;

/// Risk management (circuit breakers, position sizing, stop loss)
pub mod risk;

/// Security layer (keystore, transaction signing, wallet management)
pub mod security;

/// Performance metrics and analytics
pub mod metrics;

/// DEX integration (OpenBook/Serum)
pub mod dex;

/// Utility functions and helpers
pub mod utils;

/// Main bot orchestrator
pub mod bots;

// ═══════════════════════════════════════════════════════════════════════════
// Public API Exports - Clean & Organized
// ═══════════════════════════════════════════════════════════════════════════

// Core bot
pub use bots::GridBot;

// Configuration
pub use config::{
    Config,
    BotConfig,
    NetworkConfig,
    TradingConfig,
    StrategiesConfig,
    RiskConfig,
    PythConfig,
};

// Trading types
pub use trading::{
    OrderSide,
    OrderType,
    Order,
    OrderStatus,
};

// Strategy types
pub use strategies::{
    Strategy,
    // StrategySignal,
    GridRebalancer,
};

// Indicators - NEW!
pub use indicators::{
    Indicator,
    ATR,
    MACD,
    EMA,
    SMA,
};

// ═══════════════════════════════════════════════════════════════════════════
// Library Metadata
// ═══════════════════════════════════════════════════════════════════════════

/// Library version from Cargo.toml
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Project codename
pub const CODENAME: &str = "MULTI-STRATEGY V4.0 - CONSERVATIVE AI";

/// Build information
pub const BUILD_INFO: BuildInfo = BuildInfo {
    version: VERSION,
    name: NAME,
    codename: CODENAME,
    git_hash: "phase3a",           
    build_date: "2026-02-10",  
    rust_version: "1.70",      
};


/// Build metadata structure
#[derive(Debug, Clone, Copy)]
pub struct BuildInfo {
    /// Semantic version
    pub version: &'static str,
    /// Package name
    pub name: &'static str,
    /// Project codename
    pub codename: &'static str,
    /// Git commit hash
    pub git_hash: &'static str,
    /// Build date
    pub build_date: &'static str,
    /// Rust compiler version
    pub rust_version: &'static str,
}

// ═══════════════════════════════════════════════════════════════════════════
// Library Initialization
// ═══════════════════════════════════════════════════════════════════════════

/// Initialize the trading bot library with enhanced startup banner
/// 
/// # Returns
/// 
/// Returns `Ok(())` on successful initialization, or an error if setup fails.
/// 
/// # Examples
/// 
/// ```
/// use solana_grid_bot;
/// 
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     solana_grid_bot::init()?;
///     // Your bot code here
///     Ok(())
/// }
/// ```
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    print_startup_banner();
    
    // Initialize logging if not already configured
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    
    Ok(())
}

/// Initialize with custom configuration
/// 
/// # Arguments
/// 
/// * `config` - Configuration to validate and use
/// 
/// # Returns
/// 
/// Returns `Ok(())` if configuration is valid, otherwise returns validation errors.
pub fn init_with_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    print_startup_banner();
    
    // Validate configuration
    config.validate()?;
    
    println!("✅ Configuration validated successfully!");
    println!();
    
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Display & Utility Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Print enhanced startup banner with version info
fn print_startup_banner() {
    let border = "═".repeat(70);
    
    println!("\n{}", border);
    println!("  🤖 {} V{}", NAME.to_uppercase(), VERSION);
    println!("  🧠 {}", CODENAME);
    println!("{}", border);
    println!();
    println!("  💪 Built with Rust for MAXIMUM PERFORMANCE!");
    println!("  🎯 Production-ready for Solana DEX trading");
    println!("  🔥 MACD • RSI • Mean Reversion • Grid • Consensus AI");
    println!();
    println!("  📦 Version:     {}", VERSION);
    println!("  🏗️  Build:       {} ({})", BUILD_INFO.build_date, BUILD_INFO.git_hash);
    println!("  🦀 Rust:        {}", BUILD_INFO.rust_version);
    println!();
    println!("{}\n", border);
}

/// Print build information
pub fn print_build_info() {
    println!("Build Information:");
    println!("  Version:        {}", BUILD_INFO.version);
    println!("  Name:           {}", BUILD_INFO.name);
    println!("  Codename:       {}", BUILD_INFO.codename);
    println!("  Git Hash:       {}", BUILD_INFO.git_hash);
    println!("  Build Date:     {}", BUILD_INFO.build_date);
    println!("  Rust Version:   {}", BUILD_INFO.rust_version);
}

/// Get library version
pub fn version() -> &'static str {
    VERSION
}

/// Get full version string with codename
pub fn version_string() -> String {
    format!("{} v{} ({})", NAME, VERSION, CODENAME)
}

// ═══════════════════════════════════════════════════════════════════════════
// Prelude - Common imports for convenience
// ═══════════════════════════════════════════════════════════════════════════

/// Prelude module for convenient imports
/// 
/// # Examples
/// 
/// ```
/// use solana_grid_bot::prelude::*;
/// 
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     let config = Config::load()?;
///     let bot = GridBot::new(config)?;
///     bot.initialize().await?;
///     Ok(())
/// }
/// ```
pub mod prelude {
    pub use crate::{
        Config,
        GridBot,
        init,
        version,
    };
    
    pub use crate::trading::{
        OrderSide,
        OrderType,
        Order,
    };
    
    pub use crate::strategies::{
        Strategy,
        // StrategySignal,
    };
    
    pub use crate::indicators::{
        Indicator,
        ATR,
        MACD,
        EMA,
        SMA,
    };
    
    pub use anyhow::{Result, Context};
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature Flags & Conditional Compilation
// ═══════════════════════════════════════════════════════════════════════════

/// Check if running in test mode
pub fn is_test_mode() -> bool {
    cfg!(test)
}

/// Check if running in debug mode
pub fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
}

/// Check if running with backtrace enabled
pub fn has_backtrace() -> bool {
    std::env::var("RUST_BACKTRACE").is_ok()
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
        println!("✅ Library initialization test passed!");
    }
    
    #[test]
    fn test_version() {
        let ver = version();
        assert!(!ver.is_empty());
        println!("✅ Version: {}", ver);
    }
    
    #[test]
    fn test_version_string() {
        let ver_str = version_string();
        assert!(ver_str.contains(VERSION));
        assert!(ver_str.contains("CONSERVATIVE AI"));
        println!("✅ Version string: {}", ver_str);
    }
    
    #[test]
    fn test_build_info() {
        assert!(!BUILD_INFO.version.is_empty());
        assert!(!BUILD_INFO.name.is_empty());
        println!("✅ Build info validated!");
    }
    
    #[test]
    fn test_prelude_imports() {
        use crate::prelude::*;
        
        // Test that common types are available
        let _ver = version();
        println!("✅ Prelude imports working!");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Documentation Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(doctest)]
mod doctests {
    /// Example usage in documentation
    /// 
    /// ```
    /// use solana_grid_bot::prelude::*;
    /// 
    /// fn main() -> Result<()> {
    ///     // Initialize library
    ///     init()?;
    ///     
    ///     // Print version
    ///     println!("Version: {}", version());
    ///     
    ///     Ok(())
    /// }
    /// ```
    fn _documentation_example() {}
}
