// src/rpc/mod.rs
// ═══════════════════════════════════════════════════════════════════════════
// 🔥 RPC MODULE REGISTRY
// This file declares all available RPC implementations
// ═══════════════════════════════════════════════════════════════════════════

/// RPC fee data source for priority fee estimation
pub mod fee_source;
pub use fee_source::RpcFeeSource;
