//! ═══════════════════════════════════════════════════════════════════════════
//! 🛡️ MEV PROTECTION - Multi-Layer Defense System
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! Production-grade MEV protection for Solana grid trading:
//!
//! **Layer 1: Jito Bundle Builder** 🎯
//! - Atomic transaction bundles (all-or-nothing execution)
//! - Direct submission to Jito block engine
//! - Tip-based priority for validator inclusion
//!
//! **Layer 2: Priority Fee Optimizer** ⚡
//! - Dynamic fee calculation based on network congestion
//! - Percentile-based targeting (beat X% of transactions)
//! - Real-time RPC sampling from recent blocks
//!
//! **Layer 3: Slippage Guardian** 🛡️
//! - Adaptive slippage tolerance based on volatility
//! - Pre-execution validation (reject bad trades)
//! - Post-execution monitoring and alerts
//!
//! Conservative defaults:
//! - Jito tip: 0.001 SOL/bundle
//! - Fee percentile: 75th (good speed, reasonable cost)
//! - Max slippage: 0.5% (tight protection)
//!
//! Version: 1.0.0
//! Date: February 10, 2026
//! ═══════════════════════════════════════════════════════════════════════════

mod jito_client;
mod priority_fee;
mod slippage;

// Public API exports
pub use jito_client::{
    JitoClient,
    JitoBundle,
    JitoBundleConfig,
    BundleStatus,
};

pub use priority_fee::{
    PriorityFeeOptimizer,
    PriorityFeeConfig,
    FeeEstimate,
    NetworkCongestion,
};

pub use slippage::{
    SlippageGuardian,
    SlippageConfig,
    SlippageValidation,
    SlippageError,
};

// Re-export for convenience
pub use crate::trading::mev_protection as mev;
