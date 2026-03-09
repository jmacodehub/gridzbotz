// src/rpc/mod.rs
// ═══════════════════════════════════════════════════════════════════════════
// 🔥 HORNET RPC MODULE REGISTRY
// This file declares all available RPC implementations
// ═══════════════════════════════════════════════════════════════════════════

/// Production-ready RPC layer with automatic failover
pub mod hornet_production;

// Re-export the main production RPC for convenience
pub use hornet_production::HornetProductionRpc;

// Additional RPC implementations can be added here as needed
pub mod fee_source;
pub use fee_source::RpcFeeSource;

