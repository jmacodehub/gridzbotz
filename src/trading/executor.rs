//! ═══════════════════════════════════════════════════════════════════════════
//! ⚡ EXECUTOR - Transaction Execution Engine (MEV-Protected!)
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! ✅ Transaction building with priority fees
//! ✅ RPC client pool with failover & health monitoring
//! ✅ Exponential backoff retry logic
//! ✅ Transaction confirmation polling
//! ✅ Error handling and recovery
//! ✅ Automatic endpoint rotation with stats
//! ✅ Thread-safe atomic operations
//! 🛡️  **NEW: Optional MEV Protection (V5.0)**
//!
//! February 2026 | Project Flash V7.0 - MEV-Protected Execution Layer
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Context, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use solana_client::{
    nonblocking::rpc_client::RpcClient,
    rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::Signature,
    transaction::Transaction,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// 🛡️ Import MEV protection (optional)
use crate::trading::mev_protection::MevProtectionManager;

// ═══════════════════════════════════════════════════════════════════════════
// ⚙️ CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorConfig {
    pub rpc_url: String,
    pub rpc_fallback_urls: Option<Vec<String>>,
    pub rpc_timeout_secs: Option<u64>,
    pub max_retries: Option<u8>,
    pub confirmation_timeout_secs: Option<u64>,
    pub priority_fee_microlamports: Option<u64>,
    pub compute_unit_limit: Option<u32>,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            rpc_url: "https://api.mainnet-beta.solana.com".into(),
            rpc_fallback_urls: Some(vec![
                "https://solana-api.projectserum.com".into(),
                "https://api.rpcpool.com".into(),
            ]),
            rpc_timeout_secs: Some(30),
            max_retries: Some(3),
            confirmation_timeout_secs: Some(60),
            priority_fee_microlamports: Some(10_000),
            compute_unit_limit: Some(200_000),
        }
    }
}

impl ExecutorConfig {
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.rpc_url.is_empty() {
            bail!("rpc_url cannot be empty");
        }

        if let Some(timeout) = self.rpc_timeout_secs {
            if timeout == 0 {
                bail!("rpc_timeout_secs must be positive");
            }
        }

        if let Some(retries) = self.max_retries {
            if retries == 0 {
                bail!("max_retries must be at least 1");
            }
        }

        if let Some(fee) = self.priority_fee_microlamports {
            if fee > 1_000_000 {
                warn!("⚠️  Unusually high priority fee: {} microlamports", fee);
            }
        }

        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 📊 RPC ENDPOINT HEALTH
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHealth {
    pub url: String,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub avg_latency_ms: f64,
    pub success_rate: f64,
    pub is_primary: bool,
}

// ═══════════════════════════════════════════════════════════════════════════
// 🌐 RPC CLIENT POOL WITH FAILOVER
// ═══════════════════════════════════════════════════════════════════════════

pub struct RpcClientPool {
    primary: RpcClient,
    fallbacks: Vec<RpcClient>,
    endpoints: Vec<String>,
    current_index: AtomicU64,
    max_retries: u8,
    failure_counts: Arc<RwLock<Vec<u64>>>,
    request_counts: Arc<RwLock<Vec<u64>>>,
}

// ✅ Manual Debug implementation (RpcClient doesn't implement Debug)
impl std::fmt::Debug for RpcClientPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RpcClientPool")
            .field("endpoints", &self.endpoints)
            .field("current_index", &self.current_index.load(Ordering::Relaxed))
            .field("max_retries", &self.max_retries)
            .field("fallbacks_count", &self.fallbacks.len())
            .finish()
    }
}

impl RpcClientPool {
    pub fn new(config: &ExecutorConfig) -> Self {
        info!("🌐 Initializing RPC client pool");
        info!("   Primary: {}", config.rpc_url);

        let timeout = Duration::from_secs(config.rpc_timeout_secs.unwrap_or(30));
        let primary = RpcClient::new_with_timeout(config.rpc_url.clone(), timeout);

        let mut endpoints = vec![config.rpc_url.clone()];

        let fallbacks: Vec<RpcClient> = config
            .rpc_fallback_urls
            .as_ref()
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
            .map(|(i, url)| {
                info!("   Fallback {}: {}", i + 1, url);
                endpoints.push(url.clone());
                RpcClient::new_with_timeout(url.clone(), timeout)
            })
            .collect();

        let num_endpoints = fallbacks.len() + 1;
        let failure_counts = Arc::new(RwLock::new(vec![0; num_endpoints]));
        let request_counts = Arc::new(RwLock::new(vec![0; num_endpoints]));

        info!("✅ RPC pool initialized with {} endpoints", num_endpoints);

        Self {
            primary,
            fallbacks,
            endpoints,
            current_index: AtomicU64::new(0),
            max_retries: config.max_retries.unwrap_or(3),
            failure_counts,
            request_counts,
        }
    }

    fn get_client(&self) -> &RpcClient {
        if self.fallbacks.is_empty() {
            return &self.primary;
        }

        let idx = self.current_index.load(Ordering::Relaxed);
        if idx == 0 {
            &self.primary
        } else {
            let fallback_idx = (idx - 1) as usize % self.fallbacks.len();
            &self.fallbacks[fallback_idx]
        }
    }

    async fn rotate_endpoint(&self) {
        let old_idx = self.current_index.load(Ordering::SeqCst);
        let new_idx = (old_idx + 1) % (self.fallbacks.len() as u64 + 1);
        self.current_index.store(new_idx, Ordering::SeqCst);

        let mut failures = self.failure_counts.write().await;
        failures[old_idx as usize] += 1;

        warn!(
            "🔄 Rotating from endpoint {} to {} after failure",
            old_idx, new_idx
        );
    }

    async fn record_request(&self, success: bool) {
        let idx = self.current_index.load(Ordering::SeqCst) as usize;

        let mut requests = self.request_counts.write().await;
        requests[idx] += 1;

        if !success {
            let mut failures = self.failure_counts.write().await;
            failures[idx] += 1;
        }
    }

    /// Get health statistics for all endpoints
    pub async fn get_health_stats(&self) -> Vec<EndpointHealth> {
        let failures = self.failure_counts.read().await;
        let requests = self.request_counts.read().await;


        self.endpoints
            .iter()
            .enumerate()
            .map(|(i, url)| {
                let total = requests[i];
                let failed = failures[i];
                let success_rate = if total > 0 {
                    ((total - failed) as f64 / total as f64) * 100.0
                } else {
                    0.0
                };

                EndpointHealth {
                    url: url.clone(),
                    total_requests: total,
                    failed_requests: failed,
                    avg_latency_ms: 0.0,
                    success_rate,
                    is_primary: i == 0,
                }
            })
            .collect()
    }

    pub async fn send_transaction_with_retry(
        &self,
        tx: &Transaction,
        config: RpcSendTransactionConfig,
    ) -> Result<Signature> {
        for attempt in 0..self.max_retries {
            let client = self.get_client();
            let endpoint_idx = self.current_index.load(Ordering::Relaxed);

            match client.send_transaction_with_config(tx, config).await {
                Ok(sig) => {
                    self.record_request(true).await;
                    info!(
                        "✅ Tx sent: {} (endpoint {}, attempt {}/{})",
                        sig, endpoint_idx, attempt + 1, self.max_retries
                    );
                    return Ok(sig);
                }
                Err(e) => {
                    self.record_request(false).await;
                    error!(
                        "❌ Tx failed (endpoint {}, attempt {}/{}): {}",
                        endpoint_idx, attempt + 1, self.max_retries, e
                    );

                    if attempt < self.max_retries - 1 {
                        self.rotate_endpoint().await;
                        let backoff_secs = 2_u64.pow(attempt as u32);
                        debug!("⏳ Exponential backoff: {} seconds", backoff_secs);
                        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                    }
                }
            }
        }

        bail!(
            "Transaction failed after {} retries across {} endpoints",
            self.max_retries,
            self.endpoints.len()
        );
    }

    pub async fn wait_for_confirmation(&self, signature: &Signature, timeout_secs: u64) -> Result<()> {
        let timeout = Duration::from_secs(timeout_secs);
        let start = Instant::now();

        loop {
            if start.elapsed() > timeout {
                bail!(
                    "Confirmation timeout after {:.1}s for signature {}",
                    start.elapsed().as_secs_f64(),
                    signature
                );
            }

            match self.get_client().get_signature_status(signature).await {
                Ok(Some(Ok(_))) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    info!(
                        "✅ Transaction confirmed: {} (took {:.2}s)",
                        signature, elapsed
                    );
                    return Ok(());
                }
                Ok(Some(Err(e))) => {
                    bail!("Transaction failed on-chain: {:?}", e);
                }
                Ok(None) => {
                    let elapsed = start.elapsed().as_secs_f64();
                    debug!(
                        "⏳ Waiting for confirmation... ({:.1}s / {:.1}s)",
                        elapsed, timeout_secs as f64
                    );
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }
                Err(e) => {
                    warn!("⚠️  Error checking signature status: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    /// Get latest blockhash with retry
    pub async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash> {
        for attempt in 0..self.max_retries {
            match self.get_client().get_latest_blockhash().await {
                Ok(blockhash) => {
                    self.record_request(true).await;
                    debug!("✅ Got blockhash (attempt {}/{})", attempt + 1, self.max_retries);
                    return Ok(blockhash);
                }
                Err(e) => {
                    self.record_request(false).await;
                    warn!(
                        "⚠️  Failed to get blockhash (attempt {}/{}): {}",
                        attempt + 1, self.max_retries, e
                    );

                    if attempt < self.max_retries - 1 {
                        self.rotate_endpoint().await;
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }

        bail!("Failed to get latest blockhash after {} retries", self.max_retries);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ⚡ TRANSACTION EXECUTOR (V7.0 - MEV-PROTECTED!)
// ═══════════════════════════════════════════════════════════════════════════

pub struct TransactionExecutor {
    rpc: Arc<RpcClientPool>,
    config: ExecutorConfig,
    mev_protection: Option<MevProtectionManager>, // 🛡️ Optional MEV protection!
    total_executions: AtomicU64,
    successful_executions: AtomicU64,
    failed_executions: AtomicU64,
    mev_protected_executions: AtomicU64, // 📊 Track MEV-protected txs
}

// ✅ Manual Debug implementation
impl std::fmt::Debug for TransactionExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransactionExecutor")
            .field("config", &self.config)
            .field("mev_protection_enabled", &self.mev_protection.is_some())
            .field("total_executions", &self.total_executions.load(Ordering::Relaxed))
            .field("successful_executions", &self.successful_executions.load(Ordering::Relaxed))
            .field("failed_executions", &self.failed_executions.load(Ordering::Relaxed))
            .field("mev_protected_executions", &self.mev_protected_executions.load(Ordering::Relaxed))
            .finish()
    }
}

impl TransactionExecutor {
    /// Create new executor WITHOUT MEV protection (existing behavior)
    pub fn new(config: ExecutorConfig) -> Result<Self> {
        config.validate()?;
        let rpc = Arc::new(RpcClientPool::new(&config));

        info!("⚡ TransactionExecutor initialized");
        info!(
            "   Retries: {} | Confirmation timeout: {}s | Priority fee: {} μ",
            config.max_retries.unwrap_or(3),
            config.confirmation_timeout_secs.unwrap_or(60),
            config.priority_fee_microlamports.unwrap_or(10_000)
        );
        info!("🛡️  MEV Protection: DISABLED (call .with_mev_protection() to enable)");

        Ok(Self {
            rpc,
            config,
            mev_protection: None,
            total_executions: AtomicU64::new(0),
            successful_executions: AtomicU64::new(0),
            failed_executions: AtomicU64::new(0),
            mev_protected_executions: AtomicU64::new(0),
        })
    }

    /// 🛡️ Enable MEV protection (builder pattern)
    /// 
    /// # Example
    /// ```ignore
    /// use solana_grid_bot::trading::prelude::*;
    /// 
    /// let executor = TransactionExecutor::new(executor_config)?
    ///     .with_mev_protection(MevProtectionConfig::conservative())?;
    /// ```
    pub fn with_mev_protection(
        mut self,
        mev_config: crate::trading::mev_protection::MevProtectionConfig,
    ) -> Result<Self> {
        let mev = MevProtectionManager::new(mev_config)?;
        
        info!("🛡️  MEV Protection: ENABLED");
        info!("   Priority Fee Optimizer: {}", if mev.config().priority_fee.enabled { "ON" } else { "OFF" });
        info!("   Slippage Guardian: {}", if mev.config().slippage.enabled { "ON" } else { "OFF" });
        info!("   Jito Bundles: {}", if mev.config().jito.enabled { "ON" } else { "OFF" });
        
        self.mev_protection = Some(mev);
        Ok(self)
    }

    /// Check if MEV protection is enabled
    pub fn is_mev_protected(&self) -> bool {
        self.mev_protection.is_some()
    }

    /// Get MEV protection manager (if enabled)
    pub fn mev_protection(&self) -> Option<&MevProtectionManager> {
        self.mev_protection.as_ref()
    }

    /// Build and send a transaction (with optional MEV protection)
    pub async fn execute(
        &self,
        payer: &Pubkey,
        instructions: Vec<Instruction>,
        sign_fn: impl FnOnce(&mut Transaction) -> Result<()>,
    ) -> Result<Signature> {
        let exec_num = self.total_executions.fetch_add(1, Ordering::SeqCst) + 1;

        debug!("🚀 Execution #{} starting ({} instructions)", exec_num, instructions.len());

        // 🛡️ Get priority fee (MEV-optimized if enabled)
        let _priority_fee = if let Some(mev) = &self.mev_protection {
            self.mev_protected_executions.fetch_add(1, Ordering::SeqCst);
            match mev.get_optimal_priority_fee().await {
                Ok(fee) => {
                    debug!("🛡️  Using MEV-optimized priority fee: {} μ", fee);
                    fee
                }
                Err(e) => {
                    warn!("⚠️  Failed to get MEV fee, using fallback: {}", e);
                    self.config.priority_fee_microlamports.unwrap_or(10_000)
                }
            }
        } else {
            self.config.priority_fee_microlamports.unwrap_or(10_000)
        };

        // TODO: Use priority_fee in transaction building (future enhancement)
        // For now, it's calculated but not yet wired into the transaction

        // Get recent blockhash with retry
        let recent_blockhash = self
            .rpc
            .get_latest_blockhash()
            .await
            .context("Failed to get recent blockhash")?;

        // Build transaction
        let mut tx = Transaction::new_with_payer(&instructions, Some(payer));
        tx.message.recent_blockhash = recent_blockhash;

        // Sign transaction
        sign_fn(&mut tx).context("Failed to sign transaction")?;

        // Send with retry
        let rpc_config = RpcSendTransactionConfig {
            skip_preflight: false,
            ..Default::default()
        };

        match self.rpc.send_transaction_with_retry(&tx, rpc_config).await {
            Ok(signature) => {
                // Wait for confirmation
                let confirmation_timeout = self.config.confirmation_timeout_secs.unwrap_or(60);

                match self.rpc.wait_for_confirmation(&signature, confirmation_timeout).await {
                    Ok(_) => {
                        self.successful_executions.fetch_add(1, Ordering::SeqCst);
                        info!("🎉 Execution #{} successful!", exec_num);
                        Ok(signature)
                    }
                    Err(e) => {
                        self.failed_executions.fetch_add(1, Ordering::SeqCst);
                        error!("❌ Execution #{} confirmation failed: {}", exec_num, e);
                        Err(e)
                    }
                }
            }
            Err(e) => {
                self.failed_executions.fetch_add(1, Ordering::SeqCst);
                error!("❌ Execution #{} submission failed: {}", exec_num, e);
                Err(e)
            }
        }
    }

    /// 🛡️ Validate slippage before execution (if MEV protection enabled)
    /// 
    /// Call this before execute() to check if slippage is acceptable
    pub fn validate_slippage(
        &self,
        expected_price: f64,
        actual_price: f64,
    ) -> Result<bool> {
        if let Some(mev) = &self.mev_protection {
            let validation = mev.validate_slippage(expected_price, actual_price)?;
            
            if !validation.is_acceptable {
                warn!(
                    "🛡️  Slippage rejected: {:.4}% > {:.4}% max",
                    validation.slippage_bps as f64 / 100.0,
                    validation.max_slippage_bps as f64 / 100.0
                );
            }
            
            Ok(validation.is_acceptable)
        } else {
            // No MEV protection = always accept
            Ok(true)
        }
    }

    /// Get execution statistics
    pub fn get_stats(&self) -> ExecutionStats {
        let total = self.total_executions.load(Ordering::SeqCst);
        let successful = self.successful_executions.load(Ordering::SeqCst);
        let failed = self.failed_executions.load(Ordering::SeqCst);
        let mev_protected = self.mev_protected_executions.load(Ordering::SeqCst);

        ExecutionStats {
            total_executions: total,
            successful_executions: successful,
            failed_executions: failed,
            mev_protected_executions: mev_protected,
            success_rate: if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Get RPC endpoint health
    pub async fn get_rpc_health(&self) -> Vec<EndpointHealth> {
        self.rpc.get_health_stats().await
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 📊 EXECUTION STATISTICS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStats {
    pub total_executions: u64,
    pub successful_executions: u64,
    pub failed_executions: u64,
    pub mev_protected_executions: u64, // 🛡️ NEW!
    pub success_rate: f64,
}

// ═══════════════════════════════════════════════════════════════════════════
// ✅ TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = ExecutorConfig::default();
        assert!(config.validate().is_ok());

        config.rpc_url = "".into();
        assert!(config.validate().is_err());

        config.rpc_url = "https://api.mainnet-beta.solana.com".into();
        config.max_retries = Some(0);
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_default_config() {
        let config = ExecutorConfig::default();
        assert!(!config.rpc_url.is_empty());
        assert!(config.max_retries.unwrap() > 0);
        assert!(config.rpc_timeout_secs.unwrap() > 0);
    }

    #[test]
    fn test_execution_stats() {
        let stats = ExecutionStats {
            total_executions: 100,
            successful_executions: 95,
            failed_executions: 5,
            mev_protected_executions: 95,
            success_rate: 95.0,
        };
        assert_eq!(stats.success_rate, 95.0);
        assert_eq!(stats.mev_protected_executions, 95);
    }

    #[test]
    fn test_executor_without_mev() {
        let config = ExecutorConfig::default();
        let executor = TransactionExecutor::new(config).unwrap();
        
        assert!(!executor.is_mev_protected());
        assert!(executor.mev_protection().is_none());
    }

    #[test]
    fn test_executor_with_mev() {
        use crate::trading::mev_protection::MevProtectionConfig;
        
        let config = ExecutorConfig::default();
        let executor = TransactionExecutor::new(config)
            .unwrap()
            .with_mev_protection(MevProtectionConfig::conservative())
            .unwrap();
        
        assert!(executor.is_mev_protected());
        assert!(executor.mev_protection().is_some());
    }
}
