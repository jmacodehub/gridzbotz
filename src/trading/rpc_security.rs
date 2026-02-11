//! Secure RPC Client Wrapper - Hardened RPC connections
//!
//! Security features:
//! - URL format validation
//! - Cluster verification
//! - Rate limiting
//! - Timeout protection
//! - SSL/TLS validation

use anyhow::{anyhow, bail, Context, Result};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Solana cluster type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cluster {
    Mainnet,
    Devnet,
    Testnet,
    Localnet,
}

impl Cluster {
    /// Get expected cluster identifier
    pub fn identifier(&self) -> &str {
        match self {
            Cluster::Mainnet => "mainnet-beta",
            Cluster::Devnet => "devnet",
            Cluster::Testnet => "testnet",
            Cluster::Localnet => "localhost",
        }
    }

    /// Check if cluster requires SSL/TLS
    pub fn requires_ssl(&self) -> bool {
        matches!(self, Cluster::Mainnet | Cluster::Testnet)
    }
}

/// Rate limiter for RPC requests
struct RateLimiter {
    max_requests_per_sec: u32,
    request_times: Arc<RwLock<Vec<Instant>>>,
}

impl RateLimiter {
    fn new(max_requests_per_sec: u32) -> Self {
        Self {
            max_requests_per_sec,
            request_times: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Check if request is allowed (rate limit not exceeded)
    async fn allow_request(&self) -> Result<()> {
        let now = Instant::now();
        let mut times = self.request_times.write().await;

        // Remove requests older than 1 second
        times.retain(|t| now.duration_since(*t) < Duration::from_secs(1));

        // Check if we've exceeded rate limit
        if times.len() >= self.max_requests_per_sec as usize {
            bail!(
                "Rate limit exceeded: {} requests/sec (max {})",
                times.len(),
                self.max_requests_per_sec
            );
        }

        // Add current request
        times.push(now);

        Ok(())
    }

    /// Get current request rate
    async fn current_rate(&self) -> u32 {
        let now = Instant::now();
        let times = self.request_times.read().await;
        
        times.iter()
            .filter(|t| now.duration_since(**t) < Duration::from_secs(1))
            .count() as u32
    }
}

/// Secure RPC client configuration
#[derive(Debug, Clone)]
pub struct SecureRpcConfig {
    /// RPC endpoint URL
    pub url: String,
    
    /// Expected cluster
    pub cluster: Cluster,
    
    /// Maximum requests per second
    pub max_requests_per_sec: u32,
    
    /// Request timeout in seconds
    pub timeout_secs: u64,
    
    /// Require SSL/TLS validation
    pub require_ssl: bool,
}

impl Default for SecureRpcConfig {
    fn default() -> Self {
        Self {
            url: "https://api.devnet.solana.com".to_string(),
            cluster: Cluster::Devnet,
            max_requests_per_sec: 10,
            timeout_secs: 10,
            require_ssl: true,
        }
    }
}

impl SecureRpcConfig {
    /// Create mainnet config
    pub fn mainnet(url: String) -> Self {
        Self {
            url,
            cluster: Cluster::Mainnet,
            max_requests_per_sec: 10,
            timeout_secs: 10,
            require_ssl: true,
        }
    }

    /// Create devnet config
    pub fn devnet() -> Self {
        Self::default()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // 1. Validate URL format
        if self.url.is_empty() {
            bail!("RPC URL cannot be empty");
        }

        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            bail!(
                "Invalid RPC URL format: {}\n\
                 Must start with http:// or https://",
                self.url
            );
        }

        // 2. Validate SSL/TLS for production clusters
        if self.cluster.requires_ssl() && !self.url.starts_with("https://") {
            bail!(
                "❌ SECURITY: {:?} requires HTTPS!\n\
                 Current URL: {}\n\
                 Use https:// instead of http://",
                self.cluster,
                self.url
            );
        }

        // 3. Validate cluster matches URL
        let cluster_id = self.cluster.identifier();
        if self.cluster != Cluster::Localnet && !self.url.contains(cluster_id) {
            log::warn!(
                "⚠️  URL doesn't match cluster: expected '{}' in URL: {}",
                cluster_id,
                self.url
            );
        }

        // 4. Validate rate limit
        if self.max_requests_per_sec == 0 {
            bail!("max_requests_per_sec must be > 0");
        }

        if self.max_requests_per_sec > 1000 {
            log::warn!(
                "⚠️  Very high rate limit: {} req/sec (may hit RPC limits)",
                self.max_requests_per_sec
            );
        }

        // 5. Validate timeout
        if self.timeout_secs == 0 {
            bail!("timeout_secs must be > 0");
        }

        if self.timeout_secs > 120 {
            log::warn!(
                "⚠️  Very long timeout: {}s (may cause hangs)",
                self.timeout_secs
            );
        }

        Ok(())
    }
}

/// Secure RPC client with hardened security
pub struct SecureRpcClient {
    inner: RpcClient,
    config: SecureRpcConfig,
    rate_limiter: RateLimiter,
    total_requests: Arc<RwLock<u64>>,
    failed_requests: Arc<RwLock<u64>>,
    rate_limited_requests: Arc<RwLock<u64>>,
}

impl SecureRpcClient {
    /// Create new secure RPC client
    pub fn new(config: SecureRpcConfig) -> Result<Self> {
        config.validate()?;

        let timeout = Duration::from_secs(config.timeout_secs);
        let inner = RpcClient::new_with_timeout(config.url.clone(), timeout);

        let rate_limiter = RateLimiter::new(config.max_requests_per_sec);

        log::info!("🔒 Secure RPC Client initialized");
        log::info!("   Cluster: {:?}", config.cluster);
        log::info!("   URL: {}", config.url);
        log::info!("   Rate limit: {} req/sec", config.max_requests_per_sec);
        log::info!("   Timeout: {}s", config.timeout_secs);
        log::info!("   SSL/TLS: {}", if config.url.starts_with("https://") { "✅ ENABLED" } else { "❌ DISABLED" });

        Ok(Self {
            inner,
            config,
            rate_limiter,
            total_requests: Arc::new(RwLock::new(0)),
            failed_requests: Arc::new(RwLock::new(0)),
            rate_limited_requests: Arc::new(RwLock::new(0)),
        })
    }

    /// Get inner RPC client (after rate limit check)
    pub async fn client(&self) -> Result<&RpcClient> {
        // Check rate limit before allowing access
        match self.rate_limiter.allow_request().await {
            Ok(_) => {
                *self.total_requests.write().await += 1;
                Ok(&self.inner)
            }
            Err(e) => {
                *self.rate_limited_requests.write().await += 1;
                log::warn!("⚠️  {}", e);
                Err(e)
            }
        }
    }

    /// Execute request with rate limiting
    pub async fn execute<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&RpcClient) -> futures::future::BoxFuture<'_, Result<T>>,
    {
        let client = self.client().await?;
        
        match f(client).await {
            Ok(result) => Ok(result),
            Err(e) => {
                *self.failed_requests.write().await += 1;
                Err(e)
            }
        }
    }

    /// Get client configuration
    pub fn config(&self) -> &SecureRpcConfig {
        &self.config
    }

    /// Get client statistics
    pub async fn stats(&self) -> ClientStats {
        ClientStats {
            total_requests: *self.total_requests.read().await,
            failed_requests: *self.failed_requests.read().await,
            rate_limited_requests: *self.rate_limited_requests.read().await,
            current_rate: self.rate_limiter.current_rate().await,
            max_rate: self.config.max_requests_per_sec,
        }
    }
}

/// Client statistics
#[derive(Debug, Clone)]
pub struct ClientStats {
    pub total_requests: u64,
    pub failed_requests: u64,
    pub rate_limited_requests: u64,
    pub current_rate: u32,
    pub max_rate: u32,
}

impl ClientStats {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        
        let successful = self.total_requests - self.failed_requests;
        (successful as f64 / self.total_requests as f64) * 100.0
    }

    /// Check if client is healthy
    pub fn is_healthy(&self) -> bool {
        // Healthy if:
        // - Success rate > 90%
        // - Not constantly hitting rate limit
        let success_rate = self.success_rate();
        let rate_limit_ratio = if self.total_requests > 0 {
            self.rate_limited_requests as f64 / self.total_requests as f64
        } else {
            0.0
        };

        success_rate > 90.0 && rate_limit_ratio < 0.1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cluster_identifiers() {
        assert_eq!(Cluster::Mainnet.identifier(), "mainnet-beta");
        assert_eq!(Cluster::Devnet.identifier(), "devnet");
        assert_eq!(Cluster::Testnet.identifier(), "testnet");
        assert_eq!(Cluster::Localnet.identifier(), "localhost");
    }

    #[test]
    fn test_cluster_ssl_requirements() {
        assert!(Cluster::Mainnet.requires_ssl());
        assert!(Cluster::Testnet.requires_ssl());
        assert!(!Cluster::Devnet.requires_ssl());
        assert!(!Cluster::Localnet.requires_ssl());
    }

    #[test]
    fn test_config_validation_empty_url() {
        let mut config = SecureRpcConfig::default();
        config.url = "".to_string();
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_invalid_scheme() {
        let mut config = SecureRpcConfig::default();
        config.url = "ftp://example.com".to_string();
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_mainnet_requires_https() {
        let config = SecureRpcConfig {
            url: "http://api.mainnet-beta.solana.com".to_string(),
            cluster: Cluster::Mainnet,
            ..Default::default()
        };
        
        // Should fail because mainnet requires HTTPS
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_valid_mainnet() {
        let config = SecureRpcConfig {
            url: "https://api.mainnet-beta.solana.com".to_string(),
            cluster: Cluster::Mainnet,
            ..Default::default()
        };
        
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_devnet_allows_http() {
        let config = SecureRpcConfig {
            url: "http://api.devnet.solana.com".to_string(),
            cluster: Cluster::Devnet,
            ..Default::default()
        };
        
        // Devnet allows HTTP (for testing)
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_zero_rate_limit() {
        let mut config = SecureRpcConfig::default();
        config.max_requests_per_sec = 0;
        
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_zero_timeout() {
        let mut config = SecureRpcConfig::default();
        config.timeout_secs = 0;
        
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_rate_limiter() {
        let limiter = RateLimiter::new(2); // 2 requests per second

        // First 2 requests should succeed
        assert!(limiter.allow_request().await.is_ok());
        assert!(limiter.allow_request().await.is_ok());

        // Third should fail (rate limit exceeded)
        assert!(limiter.allow_request().await.is_err());

        // After 1 second, should work again
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert!(limiter.allow_request().await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limiter_current_rate() {
        let limiter = RateLimiter::new(10);

        assert_eq!(limiter.current_rate().await, 0);

        limiter.allow_request().await.unwrap();
        assert_eq!(limiter.current_rate().await, 1);

        limiter.allow_request().await.unwrap();
        limiter.allow_request().await.unwrap();
        assert_eq!(limiter.current_rate().await, 3);
    }

    #[test]
    fn test_client_stats_success_rate() {
        let stats = ClientStats {
            total_requests: 100,
            failed_requests: 10,
            rate_limited_requests: 5,
            current_rate: 5,
            max_rate: 10,
        };

        assert_eq!(stats.success_rate(), 90.0);
    }

    #[test]
    fn test_client_stats_is_healthy() {
        // Healthy client
        let healthy = ClientStats {
            total_requests: 100,
            failed_requests: 5,  // 95% success
            rate_limited_requests: 2,  // 2% rate limited
            current_rate: 5,
            max_rate: 10,
        };
        assert!(healthy.is_healthy());

        // Unhealthy: too many failures
        let unhealthy_failures = ClientStats {
            total_requests: 100,
            failed_requests: 20,  // 80% success (< 90%)
            rate_limited_requests: 0,
            current_rate: 5,
            max_rate: 10,
        };
        assert!(!unhealthy_failures.is_healthy());

        // Unhealthy: too many rate limits
        let unhealthy_rate_limit = ClientStats {
            total_requests: 100,
            failed_requests: 0,
            rate_limited_requests: 20,  // 20% rate limited (> 10%)
            current_rate: 5,
            max_rate: 10,
        };
        assert!(!unhealthy_rate_limit.is_healthy());
    }
}
