//! 🔥 HORNET PRODUCTION RPC LAYER - CONFIG-DRIVEN EDITION
//! Updated: November 9, 2025 - Config Integration + Fallback Support
//!
//! Features:
//! - Reads RPC endpoints from config.toml
//! - Dynamic fallback chain based on config
//! - Chainstack/Mainnet/Helius triple redundancy
//! - Automatic failover with latency monitoring

use solana_client::rpc_client::RpcClient;
use std::time::{Duration, Instant};
use log::{info, warn, error, debug};
use std::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::{Result, Context as _};
use solana_grid_bot::config::NetworkConfig;

/// HORNET Production RPC with Dynamic Failover
///
/// Architecture:
/// 1. PRIMARY:   From config (rpc_url)
/// 2. SECONDARY: From config (rpc_fallback_urls[0])
/// 3. TERTIARY:  From config (rpc_fallback_urls[1])
/// 4. QUATERNARY: From config (rpc_fallback_urls[2])
pub struct HornetProductionRpc {
    // Primary RPC client
    primary: RpcClient,

    // Fallback RPC clients (dynamic list from config)
    fallbacks: Vec<RpcClient>,

    // Metrics tracking
    primary_failures: Arc<AtomicU64>,
    fallback_activations: Arc<AtomicU64>,
    total_requests: Arc<AtomicU64>,

    // Health status
    is_primary_healthy: Arc<AtomicBool>,

    // Store URLs for logging
    primary_url: String,
    fallback_urls: Vec<String>,
}

impl HornetProductionRpc {
    /// Initialize from NetworkConfig (reads from TOML)
    pub fn from_config(network_cfg: &NetworkConfig) -> Result<Self> {
        info!("🚀 HORNET PRODUCTION RPC - CONFIG-DRIVEN EDITION");
        info!("════════════════════════════════════════════════════════════════");

        // Get primary RPC from config
        let primary_url = network_cfg.rpc_url.clone();
        info!("✅ PRIMARY:   {}", primary_url);
        info!("   ├─ Timeout: {} sec", network_cfg.timeout_seconds);
        info!("   ├─ Max retries: {}", network_cfg.max_retries);
        info!("   └─ Commitment: {}", network_cfg.commitment);

        // Get fallback RPCs from config
        let fallback_urls = network_cfg.rpc_fallback_urls.clone();
        if fallback_urls.is_empty() {
            warn!("⚠️  No RPC fallbacks configured!");
        } else {
            info!("");
            info!("✅ FALLBACKS: {} endpoints", fallback_urls.len());
            for (i, url) in fallback_urls.iter().enumerate() {
                info!("   {}. {}", i + 1, url);
            }
        }
        info!("");
        info!("📊 Failover Strategy: Auto-switch on timeout or error");
        info!("════════════════════════════════════════════════════════════════");

        let timeout = Duration::from_secs(network_cfg.timeout_seconds as u64);

        // Create primary client
        let primary = RpcClient::new_with_timeout(primary_url.clone(), timeout);

        // Create fallback clients
        let fallbacks: Vec<RpcClient> = fallback_urls
            .iter()
            .map(|url| RpcClient::new_with_timeout(url.clone(), timeout))
            .collect();

        Ok(Self {
            primary,
            fallbacks,
            primary_failures: Arc::new(AtomicU64::new(0)),
            fallback_activations: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
            is_primary_healthy: Arc::new(AtomicBool::new(true)),
            primary_url,
            fallback_urls,
        })
    }

    /// Initialize with HARDCODED defaults (backward compatibility)
    pub fn new() -> Self {
        info!("🚀 HORNET PRODUCTION RPC - HARDCODED DEFAULTS");
        info!("════════════════════════════════════════════════════════════════");
        info!("⚠️  WARNING: Using hardcoded RPC endpoints");
        info!("   Recommendation: Use from_config() instead");
        info!("════════════════════════════════════════════════════════════════");

        let primary_url = "https://solana-mainnet.core.chainstack.com/c28341789b2597fbd3f4d74a5b7026f5".to_string();
        let fallback_urls = vec![
            "https://api.mainnet-beta.solana.com".to_string(),
            "https://mainnet.helius-rpc.com".to_string(),
        ];

        let timeout = Duration::from_secs(5);

        let primary = RpcClient::new_with_timeout(primary_url.clone(), timeout);
        let fallbacks: Vec<RpcClient> = fallback_urls
            .iter()
            .map(|url| RpcClient::new_with_timeout(url.clone(), timeout))
            .collect();

        Self {
            primary,
            fallbacks,
            primary_failures: Arc::new(AtomicU64::new(0)),
            fallback_activations: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
            is_primary_healthy: Arc::new(AtomicBool::new(true)),
            primary_url,
            fallback_urls,
        }
    }

    /// Get latest blockhash with intelligent failover
    pub async fn get_latest_blockhash(&self) -> Result<solana_program::hash::Hash> {
        let start = Instant::now();
        self.total_requests.fetch_add(1, Ordering::SeqCst);

        // ═══════════════════════════════════════════════════════════════
        // ATTEMPT 1: PRIMARY RPC
        // ═══════════════════════════════════════════════════════════════
        debug!("🔍 Attempting RPC: Primary ({})", self.primary_url);
        match self.primary.get_latest_blockhash() {
            Ok(hash) => {
                let latency_ms = start.elapsed().as_millis() as u64;

                if latency_ms < 150 {
                    debug!("✅ Primary: {}ms (excellent)", latency_ms);
                } else if latency_ms < 250 {
                    info!("✅ Primary: {}ms (good)", latency_ms);
                } else {
                    warn!("⚠️  Primary: {}ms (slow)", latency_ms);
                }

                self.is_primary_healthy.store(true, Ordering::SeqCst);
                return Ok(hash);
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                warn!("❌ Primary failed after {}ms: {}", latency_ms, e);
                self.primary_failures.fetch_add(1, Ordering::SeqCst);
                self.is_primary_healthy.store(false, Ordering::SeqCst);
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // ATTEMPT 2+: FALLBACK RPCs
        // ═══════════════════════════════════════════════════════════════
        for (i, fallback) in self.fallbacks.iter().enumerate() {
            let fallback_url = self.fallback_urls.get(i)
                .map(|s| s.as_str())
                .unwrap_or("unknown");

            warn!("🔄 Failover engaged: Fallback #{} ({})", i + 1, fallback_url);

            match fallback.get_latest_blockhash() {
                Ok(hash) => {
                    let latency_ms = start.elapsed().as_millis() as u64;
                    warn!("🥈 Fallback #{} SUCCESS: {}ms", i + 1, latency_ms);
                    self.fallback_activations.fetch_add(1, Ordering::SeqCst);
                    return Ok(hash);
                }
                Err(e) => {
                    let latency_ms = start.elapsed().as_millis() as u64;
                    error!("❌ Fallback #{} FAILED after {}ms: {}", i + 1, latency_ms, e);
                    // Continue to next fallback
                }
            }
        }

        // ═══════════════════════════════════════════════════════════════
        // ALL RPCS FAILED
        // ═══════════════════════════════════════════════════════════════
        let latency_ms = start.elapsed().as_millis() as u64;
        error!("════════════════════════════════════════════════════════════════");
        error!("❌ TOTAL RPC FAILURE: All {} endpoints down after {}ms",
            1 + self.fallbacks.len(), latency_ms);
        error!("════════════════════════════════════════════════════════════════");
        error!("   Primary: {} - FAILED", self.primary_url);
        for (i, url) in self.fallback_urls.iter().enumerate() {
            error!("   Fallback #{}: {} - FAILED", i + 1, url);
        }
        error!("");
        error!("   🚨 ACTION REQUIRED:");
        error!("   1. Check internet connection");
        error!("   2. Verify RPC endpoint status");
        error!("   3. Grid trading PAUSED until RPC restored");
        error!("════════════════════════════════════════════════════════════════");

        Err(anyhow::anyhow!(
            "Complete RPC failure - all {} endpoints unavailable after {}ms",
            1 + self.fallbacks.len(), latency_ms
        ))
    }

    /// Get current RPC health status
    pub fn health_check(&self) -> RpcHealthStatus {
        RpcHealthStatus {
            primary_healthy: self.is_primary_healthy.load(Ordering::SeqCst),
            primary_failures: self.primary_failures.load(Ordering::SeqCst),
            fallback_activations: self.fallback_activations.load(Ordering::SeqCst),
            total_requests: self.total_requests.load(Ordering::SeqCst),
        }
    }

    /// Get detailed metrics for logging/monitoring
    pub fn get_metrics(&self) -> String {
        let health = self.health_check();

        let success_rate = if health.total_requests > 0 {
            let successful = health.total_requests - health.primary_failures;
            successful as f64 / health.total_requests as f64 * 100.0
        } else {
            100.0
        };

        let primary_ratio = if health.total_requests > 0 {
            let primary_success = health.total_requests - health.primary_failures;
            primary_success as f64 / health.total_requests as f64 * 100.0
        } else {
            100.0
        };

        format!(
            "🔥 HORNET RPC METRICS\n\
            ├─ Total Requests: {}\n\
            ├─ Overall Success: {:.2}%\n\
            ├─ Primary Success: {:.2}%\n\
            ├─ Primary Failures: {}\n\
            ├─ Fallback Activations: {}\n\
            └─ Primary Status: {}",
            health.total_requests,
            success_rate,
            primary_ratio,
            health.primary_failures,
            health.fallback_activations,
            if health.primary_healthy { "🟢 HEALTHY" } else { "🔴 FAILED" }
        )
    }
}

/// RPC Health Status snapshot for monitoring
#[derive(Debug, Clone)]
pub struct RpcHealthStatus {
    pub primary_healthy: bool,
    pub primary_failures: u64,
    pub fallback_activations: u64,
    pub total_requests: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hornet_rpc_new() {
        let rpc = HornetProductionRpc::new();
        let health = rpc.health_check();

        assert!(health.primary_healthy);
        assert_eq!(health.total_requests, 0);
        assert_eq!(health.primary_failures, 0);
    }

    #[test]
    fn test_metrics_formatting() {
        let rpc = HornetProductionRpc::new();
        let metrics = rpc.get_metrics();

        assert!(metrics.contains("HORNET RPC METRICS"));
        assert!(metrics.contains("Total Requests"));
    }
}
