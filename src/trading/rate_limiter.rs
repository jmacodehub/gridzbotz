//! Trade Rate Limiter - Prevents excessive trading
//!
//! Features:
//! - Global trade rate limiting
//! - Per-token rate limiting
//! - Sliding window algorithm
//! - Thread-safe async implementation

use anyhow::{bail, Result};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum trades per minute (global)
    pub max_trades_per_minute: u32,
    
    /// Maximum trades per minute per token
    pub max_trades_per_token_per_minute: u32,
    
    /// Window size for rate limiting (seconds)
    pub window_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_trades_per_minute: 60,      // 1 trade/sec average
            max_trades_per_token_per_minute: 20,  // Per token limit
            window_secs: 60,                // 1 minute window
        }
    }
}

impl RateLimitConfig {
    /// Conservative limits
    pub fn conservative() -> Self {
        Self {
            max_trades_per_minute: 30,
            max_trades_per_token_per_minute: 10,
            window_secs: 60,
        }
    }

    /// Aggressive limits
    pub fn aggressive() -> Self {
        Self {
            max_trades_per_minute: 120,
            max_trades_per_token_per_minute: 40,
            window_secs: 60,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.max_trades_per_minute == 0 {
            bail!("max_trades_per_minute must be > 0");
        }

        if self.max_trades_per_token_per_minute == 0 {
            bail!("max_trades_per_token_per_minute must be > 0");
        }

        if self.window_secs == 0 {
            bail!("window_secs must be > 0");
        }

        Ok(())
    }
}

/// Trade rate limiter
pub struct TradeRateLimiter {
    config: RateLimitConfig,
    global_trades: Arc<RwLock<Vec<Instant>>>,
    per_token_trades: Arc<RwLock<HashMap<Pubkey, Vec<Instant>>>>,
}

impl TradeRateLimiter {
    pub fn new(config: RateLimitConfig) -> Result<Self> {
        config.validate()?;

        Ok(Self {
            config,
            global_trades: Arc::new(RwLock::new(Vec::new())),
            per_token_trades: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Check if trade is allowed
    pub async fn allow_trade(&self, token: &Pubkey) -> Result<()> {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_secs);

        // Check global limit
        let mut global = self.global_trades.write().await;
        global.retain(|t| now.duration_since(*t) < window);

        if global.len() >= self.config.max_trades_per_minute as usize {
            bail!(
                "Global rate limit exceeded: {} trades in last {} seconds (max {})",
                global.len(),
                self.config.window_secs,
                self.config.max_trades_per_minute
            );
        }

        // Check per-token limit
        let mut per_token = self.per_token_trades.write().await;
        let token_trades = per_token.entry(*token).or_insert_with(Vec::new);
        token_trades.retain(|t| now.duration_since(*t) < window);

        if token_trades.len() >= self.config.max_trades_per_token_per_minute as usize {
            bail!(
                "Per-token rate limit exceeded: {} trades for {} in last {} seconds (max {})",
                token_trades.len(),
                token,
                self.config.window_secs,
                self.config.max_trades_per_token_per_minute
            );
        }

        // Record trade
        global.push(now);
        token_trades.push(now);

        Ok(())
    }

    /// Get current global trade rate
    pub async fn current_global_rate(&self) -> u32 {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_secs);
        let trades = self.global_trades.read().await;
        
        trades.iter()
            .filter(|t| now.duration_since(**t) < window)
            .count() as u32
    }

    /// Get current rate for specific token
    pub async fn current_token_rate(&self, token: &Pubkey) -> u32 {
        let now = Instant::now();
        let window = Duration::from_secs(self.config.window_secs);
        let per_token = self.per_token_trades.read().await;
        
        per_token.get(token)
            .map(|trades| {
                trades.iter()
                    .filter(|t| now.duration_since(**t) < window)
                    .count() as u32
            })
            .unwrap_or(0)
    }

    /// Clear all rate limit history (for testing)
    pub async fn clear(&self) {
        self.global_trades.write().await.clear();
        self.per_token_trades.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn test_token() -> Pubkey {
        Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap()
    }

    #[tokio::test]
    async fn test_allow_trade() {
        let limiter = TradeRateLimiter::new(RateLimitConfig {
            max_trades_per_minute: 5,
            max_trades_per_token_per_minute: 3,
            window_secs: 60,
        }).unwrap();

        let token = test_token();

        // First 3 trades should succeed
        assert!(limiter.allow_trade(&token).await.is_ok());
        assert!(limiter.allow_trade(&token).await.is_ok());
        assert!(limiter.allow_trade(&token).await.is_ok());

        // 4th trade should fail (per-token limit)
        assert!(limiter.allow_trade(&token).await.is_err());
    }

    #[tokio::test]
    async fn test_global_limit() {
        let limiter = TradeRateLimiter::new(RateLimitConfig {
            max_trades_per_minute: 2,
            max_trades_per_token_per_minute: 10,
            window_secs: 60,
        }).unwrap();

        let token1 = test_token();
        let token2 = Pubkey::new_unique();

        // First 2 trades (different tokens) should succeed
        assert!(limiter.allow_trade(&token1).await.is_ok());
        assert!(limiter.allow_trade(&token2).await.is_ok());

        // 3rd trade should fail (global limit)
        assert!(limiter.allow_trade(&token1).await.is_err());
    }

    #[tokio::test]
    async fn test_rate_resets_after_window() {
        let limiter = TradeRateLimiter::new(RateLimitConfig {
            max_trades_per_minute: 1,
            max_trades_per_token_per_minute: 1,
            window_secs: 1,  // 1 second window for fast test
        }).unwrap();

        let token = test_token();

        // First trade succeeds
        assert!(limiter.allow_trade(&token).await.is_ok());

        // Second trade fails
        assert!(limiter.allow_trade(&token).await.is_err());

        // Wait for window to expire
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should work again
        assert!(limiter.allow_trade(&token).await.is_ok());
    }

    #[tokio::test]
    async fn test_current_rates() {
        let limiter = TradeRateLimiter::new(RateLimitConfig::default()).unwrap();
        let token = test_token();

        assert_eq!(limiter.current_global_rate().await, 0);
        assert_eq!(limiter.current_token_rate(&token).await, 0);

        limiter.allow_trade(&token).await.unwrap();
        limiter.allow_trade(&token).await.unwrap();

        assert_eq!(limiter.current_global_rate().await, 2);
        assert_eq!(limiter.current_token_rate(&token).await, 2);
    }
}
