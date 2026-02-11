//! ═══════════════════════════════════════════════════════════════════════
//! 🤖 SOLANA GRID TRADING BOT - V5.1 "SECURITY HARDENING"
//! 
//! High-performance Rust implementation with:
//! • Dynamic grid repositioning
//! • Multi-strategy consensus engine (MACD, RSI, Mean Reversion)
//! • Real-time risk management
//! • Market regime detection
//! • Automatic order lifecycle management
//! • Technical indicators library (ATR, MACD, EMA, SMA)
//! • MEV Protection (🛡️ Priority fees, slippage guard, Jito bundles)
//! • SECURITY HARDENING (🔒 Order validation, RPC security, rate limiting)
//! 
//! Built for production trading on Solana DEX
//! 
//! V5.1 SECURITY AUDIT COMPLETE (Feb 11, 2026):
//! ✓ Keystore encryption with transaction rate limits
//! ✓ Pre-signature order validation with whitelist
//! ✓ Secure RPC wrapper with SSL enforcement
//! ✓ Trade rate limiter (global + per-token)
//! ✓ Config security auditor
//! 
//! Version: 0.2.6
//! License: MIT
//! Date: February 11, 2026
//! ═══════════════════════════════════════════════════════════════════════

#![allow(missing_docs)] 
#![allow(missing_debug_implementations)]

#![warn(
    rust_2018_idioms,
    unreachable_pub
)]
#![deny(unsafe_code)]
#![allow(clippy::too_many_arguments)]

// ═══════════════════════════════════════════════════════════════════════
// Module Declarations
//═══════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════
// Security & Observability (V5.1) - Need warp dependency first!
// ═══════════════════════════════════════════════════════════════════════

pub mod config_audit;
// TODO: Add warp dependency, then uncomment:
// pub mod health;
// pub mod prometheus;

// ═══════════════════════════════════════════════════════════════════════
// Public API Exports
// ═══════════════════════════════════════════════════════════════════════

pub use bots::GridBot;

pub use config::{
    Config,
    BotConfig,
    NetworkConfig,
    TradingConfig,
    StrategiesConfig,
    RiskConfig,
    PythConfig,
};

pub use trading::{
    OrderSide,
    OrderType,
    Order,
    OrderStatus,
};

pub use strategies::{
    Strategy,
    GridRebalancer,
};

pub use indicators::{
    Indicator,
    ATR,
    MACD,
    EMA,
    SMA,
};

// Security (V5.1)
pub use config_audit::{ConfigAuditor, AuditReport, AuditLevel, AuditFinding};

// ═══════════════════════════════════════════════════════════════════════
// Library Metadata
// ═══════════════════════════════════════════════════════════════════════

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const CODENAME: &str = "V5.1 - SECURITY HARDENING COMPLETE";

pub const BUILD_INFO: BuildInfo = BuildInfo {
    version: VERSION,
    name: NAME,
    codename: CODENAME,
    git_hash: "feature/phase4-security-hardening",           
    build_date: "2026-02-11",  
    rust_version: "1.70",      
};

#[derive(Debug, Clone, Copy)]
pub struct BuildInfo {
    pub version: &'static str,
    pub name: &'static str,
    pub codename: &'static str,
    pub git_hash: &'static str,
    pub build_date: &'static str,
    pub rust_version: &'static str,
}

pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    print_startup_banner();
    
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    
    Ok(())
}

pub fn init_with_config(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
    print_startup_banner();
    config.validate()?;
    println!("✅ Configuration validated successfully!");
    println!();
    Ok(())
}

fn print_startup_banner() {
    let border = "═".repeat(75);
    
    println!("\n{}", border);
    println!("  🤖 {} V{}", NAME.to_uppercase(), VERSION);
    println!("  🔒 {}", CODENAME);
    println!("{}", border);
    println!();
    println!("  💪 Built with Rust for MAXIMUM PERFORMANCE!");
    println!("  🎯 Production-ready for Solana DEX trading");
    println!("  🔥 MACD • RSI • Mean Reversion • Grid • Consensus AI");
    println!("  🛡️  MEV Protection • Priority Fees • Slippage Guard • Jito Bundles");
    println!("  🔒 ORDER VALIDATION • RPC SECURITY • RATE LIMITING");
    println!();
    println!("  📦 Version:     {}", VERSION);
    println!("  🏭  Build:       {} ({})", BUILD_INFO.build_date, BUILD_INFO.git_hash);
    println!("  🦀 Rust:        {}", BUILD_INFO.rust_version);
    println!();
    println!("{}\n", border);
}

pub fn print_build_info() {
    println!("Build Information:");
    println!("  Version:        {}", BUILD_INFO.version);
    println!("  Name:           {}", BUILD_INFO.name);
    println!("  Codename:       {}", BUILD_INFO.codename);
    println!("  Git Hash:       {}", BUILD_INFO.git_hash);
    println!("  Build Date:     {}", BUILD_INFO.build_date);
    println!("  Rust Version:   {}", BUILD_INFO.rust_version);
}

pub fn version() -> &'static str {
    VERSION
}

pub fn version_string() -> String {
    format!("{} v{} ({})", NAME, VERSION, CODENAME)
}

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
    };
    
    pub use crate::indicators::{
        Indicator,
        ATR,
        MACD,
        EMA,
        SMA,
    };
    
    pub use crate::config_audit::{
        ConfigAuditor,
        AuditReport,
    };
    
    pub use anyhow::{Result, Context};
}

pub fn is_test_mode() -> bool {
    cfg!(test)
}

pub fn is_debug_mode() -> bool {
    cfg!(debug_assertions)
}

pub fn has_backtrace() -> bool {
    std::env::var("RUST_BACKTRACE").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init() {
        assert!(init().is_ok());
    }
    
    #[test]
    fn test_version() {
        let ver = version();
        assert!(!ver.is_empty());
    }
    
    #[test]
    fn test_security_modules() {
        use crate::prelude::*;
        let _: Option<ConfigAuditor> = None;
    }
}
