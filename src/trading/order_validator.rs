//! Order Validation Module - Security layer before transaction signing
//!
//! Validates all order parameters before allowing transaction execution.
//! This prevents:
//! - Invalid order sizes (too small/large)
//! - Unauthorized token trading
//! - Excessive slippage
//! - Duplicate orders
//! - Malformed order parameters

use anyhow::{anyhow, bail, Context, Result};
use solana_sdk::pubkey::Pubkey;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Order validation policy - defines acceptable order parameters
#[derive(Debug, Clone)]
pub struct OrderValidationPolicy {
    /// Maximum notional value in USD (e.g., $10,000)
    pub max_notional_usd: f64,
    
    /// Minimum notional value in USD (e.g., $1)
    pub min_notional_usd: f64,
    
    /// Maximum order size in base token (e.g., 100 SOL)
    pub max_size: f64,
    
    /// Minimum order size in base token (e.g., 0.01 SOL)
    pub min_size: f64,
    
    /// Allowed tokens for trading (whitelist)
    pub allowed_tokens: Vec<Pubkey>,
    
    /// Maximum slippage in basis points (e.g., 100 = 1%)
    pub max_slippage_bps: u16,
    
    /// Enable duplicate order prevention
    pub prevent_duplicates: bool,
    
    /// Maximum price deviation from oracle (sanity check)
    /// e.g., 0.10 = 10% max deviation
    pub max_price_deviation: f64,
}

impl Default for OrderValidationPolicy {
    fn default() -> Self {
        Self {
            max_notional_usd: 10_000.0,  // $10k max per order
            min_notional_usd: 1.0,        // $1 min per order
            max_size: 100.0,              // 100 SOL max
            min_size: 0.001,              // 0.001 SOL min
            allowed_tokens: vec![],       // Empty = allow all (override this!)
            max_slippage_bps: 100,        // 1% max slippage
            prevent_duplicates: true,     // Prevent duplicate orders
            max_price_deviation: 0.10,    // 10% max price deviation
        }
    }
}

impl OrderValidationPolicy {
    /// Create conservative policy (tight limits)
    pub fn conservative() -> Self {
        Self {
            max_notional_usd: 1_000.0,    // $1k max
            min_notional_usd: 5.0,         // $5 min
            max_size: 10.0,                // 10 SOL max
            min_size: 0.01,                // 0.01 SOL min
            allowed_tokens: vec![],        // Must specify whitelist
            max_slippage_bps: 50,          // 0.5% max slippage
            prevent_duplicates: true,
            max_price_deviation: 0.05,     // 5% max price deviation
        }
    }
    
    /// Create aggressive policy (loose limits)
    pub fn aggressive() -> Self {
        Self {
            max_notional_usd: 50_000.0,    // $50k max
            min_notional_usd: 1.0,         // $1 min
            max_size: 500.0,               // 500 SOL max
            min_size: 0.001,               // 0.001 SOL min
            allowed_tokens: vec![],        // Allow all
            max_slippage_bps: 200,         // 2% max slippage
            prevent_duplicates: true,
            max_price_deviation: 0.20,     // 20% max price deviation
        }
    }

    /// Validate policy configuration
    pub fn validate(&self) -> Result<()> {
        if self.max_notional_usd <= self.min_notional_usd {
            bail!("max_notional_usd must be greater than min_notional_usd");
        }

        if self.max_size <= self.min_size {
            bail!("max_size must be greater than min_size");
        }

        if self.max_slippage_bps > 10_000 {
            bail!("max_slippage_bps too high: {} (max 10000 = 100%)", self.max_slippage_bps);
        }

        if self.max_price_deviation < 0.0 || self.max_price_deviation > 1.0 {
            bail!("max_price_deviation must be between 0.0 and 1.0");
        }

        Ok(())
    }
}

/// Order parameters to validate
#[derive(Debug, Clone)]
pub struct OrderParams {
    /// Token being traded
    pub token: Pubkey,
    
    /// Order price in USD
    pub price: f64,
    
    /// Order size in base token units
    pub size: f64,
    
    /// Expected slippage in basis points (optional)
    pub slippage_bps: Option<u16>,
    
    /// Oracle price for sanity check (optional)
    pub oracle_price: Option<f64>,
}

impl OrderParams {
    /// Calculate notional value (price * size)
    pub fn notional_value(&self) -> f64 {
        self.price * self.size
    }
}

/// Validation result with detailed error information
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn success() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            is_valid: false,
            errors: vec![message],
            warnings: vec![],
        }
    }

    pub fn add_error(&mut self, message: String) {
        self.is_valid = false;
        self.errors.push(message);
    }

    pub fn add_warning(&mut self, message: String) {
        self.warnings.push(message);
    }
}

/// Order validator - enforces trading policies
pub struct OrderValidator {
    policy: OrderValidationPolicy,
    recent_orders: Arc<RwLock<HashSet<String>>>, // For duplicate detection
}

impl OrderValidator {
    /// Create validator with policy
    pub fn new(policy: OrderValidationPolicy) -> Result<Self> {
        policy.validate()?;
        
        Ok(Self {
            policy,
            recent_orders: Arc::new(RwLock::new(HashSet::new())),
        })
    }

    /// Create validator with default policy
    pub fn default_policy() -> Result<Self> {
        Self::new(OrderValidationPolicy::default())
    }

    /// Create validator with conservative policy
    pub fn conservative() -> Result<Self> {
        Self::new(OrderValidationPolicy::conservative())
    }

    /// Validate order parameters
    pub async fn validate(&self, params: &OrderParams) -> Result<ValidationResult> {
        let mut result = ValidationResult::success();

        // 1. Validate notional value
        let notional = params.notional_value();
        
        if notional < self.policy.min_notional_usd {
            result.add_error(format!(
                "Order notional ${:.2} below minimum ${:.2}",
                notional, self.policy.min_notional_usd
            ));
        }

        if notional > self.policy.max_notional_usd {
            result.add_error(format!(
                "Order notional ${:.2} exceeds maximum ${:.2}",
                notional, self.policy.max_notional_usd
            ));
        }

        // 2. Validate order size
        if params.size < self.policy.min_size {
            result.add_error(format!(
                "Order size {:.6} below minimum {:.6}",
                params.size, self.policy.min_size
            ));
        }

        if params.size > self.policy.max_size {
            result.add_error(format!(
                "Order size {:.6} exceeds maximum {:.6}",
                params.size, self.policy.max_size
            ));
        }

        // 3. Validate price is positive and reasonable
        if params.price <= 0.0 {
            result.add_error(format!("Invalid price: ${:.2}", params.price));
        }

        if params.price > 1_000_000.0 {
            result.add_warning(format!("Unusually high price: ${:.2}", params.price));
        }

        // 4. Validate token whitelist (if configured)
        if !self.policy.allowed_tokens.is_empty() {
            if !self.policy.allowed_tokens.contains(&params.token) {
                result.add_error(format!(
                    "Token {} not in whitelist (allowed: {:?})",
                    params.token,
                    self.policy.allowed_tokens
                ));
            }
        }

        // 5. Validate slippage
        if let Some(slippage) = params.slippage_bps {
            if slippage > self.policy.max_slippage_bps {
                result.add_error(format!(
                    "Slippage {:.2}% exceeds maximum {:.2}%",
                    slippage as f64 / 100.0,
                    self.policy.max_slippage_bps as f64 / 100.0
                ));
            }
        }

        // 6. Validate price vs oracle (sanity check)
        if let Some(oracle_price) = params.oracle_price {
            let deviation = (params.price - oracle_price).abs() / oracle_price;
            
            if deviation > self.policy.max_price_deviation {
                result.add_error(format!(
                    "Price ${:.4} deviates {:.2}% from oracle ${:.4} (max {:.2}%)",
                    params.price,
                    deviation * 100.0,
                    oracle_price,
                    self.policy.max_price_deviation * 100.0
                ));
            }
        }

        // 7. Check for duplicate orders
        if self.policy.prevent_duplicates {
            let order_key = format!("{}-{:.6}-{:.4}", params.token, params.size, params.price);
            
            let recent = self.recent_orders.read().await;
            if recent.contains(&order_key) {
                result.add_error(format!(
                    "Duplicate order detected: {} @ ${:.4}",
                    params.size, params.price
                ));
            }
            drop(recent);

            // Add to recent orders if valid so far
            if result.is_valid {
                let mut recent = self.recent_orders.write().await;
                recent.insert(order_key);
                
                // Keep set size limited (last 1000 orders)
                if recent.len() > 1000 {
                    // Remove oldest entries (simple FIFO approximation)
                    let to_remove: Vec<_> = recent.iter().take(500).cloned().collect();
                    for key in to_remove {
                        recent.remove(&key);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Validate and return Result (convenience method)
    pub async fn validate_or_error(&self, params: &OrderParams) -> Result<()> {
        let validation = self.validate(params).await?;

        if !validation.is_valid {
            let error_msg = validation.errors.join("; ");
            bail!(
                "❌ Order validation failed:\n{}",
                error_msg
            );
        }

        // Log warnings
        for warning in &validation.warnings {
            log::warn!("⚠️  Order validation warning: {}", warning);
        }

        Ok(())
    }

    /// Clear recent orders cache (for testing)
    pub async fn clear_cache(&self) {
        self.recent_orders.write().await.clear();
    }

    /// Get current policy
    pub fn policy(&self) -> &OrderValidationPolicy {
        &self.policy
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
    async fn test_valid_order() {
        let validator = OrderValidator::default_policy().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 100.0,
            size: 1.0,
            slippage_bps: Some(50),
            oracle_price: Some(100.0),
        };

        let result = validator.validate(&params).await.unwrap();
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_notional_too_high() {
        let validator = OrderValidator::conservative().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 1000.0,  // $1000
            size: 10.0,     // 10 SOL = $10,000 notional (exceeds $1k max)
            slippage_bps: Some(50),
            oracle_price: None,
        };

        let result = validator.validate(&params).await.unwrap();
        assert!(!result.is_valid);
        assert!(result.errors[0].contains("exceeds maximum"));
    }

    #[tokio::test]
    async fn test_size_too_small() {
        let validator = OrderValidator::conservative().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 100.0,
            size: 0.001,  // Below 0.01 min for conservative
            slippage_bps: Some(50),
            oracle_price: None,
        };

        let result = validator.validate(&params).await.unwrap();
        assert!(!result.is_valid);
        assert!(result.errors[0].contains("below minimum"));
    }

    #[tokio::test]
    async fn test_slippage_too_high() {
        let validator = OrderValidator::default_policy().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 100.0,
            size: 1.0,
            slippage_bps: Some(500),  // 5% exceeds 1% max
            oracle_price: None,
        };

        let result = validator.validate(&params).await.unwrap();
        assert!(!result.is_valid);
        assert!(result.errors[0].contains("exceeds maximum"));
    }

    #[tokio::test]
    async fn test_price_deviation() {
        let validator = OrderValidator::conservative().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 110.0,      // 10% above oracle
            size: 0.1,
            slippage_bps: Some(50),
            oracle_price: Some(100.0),  // Oracle says $100
        };

        let result = validator.validate(&params).await.unwrap();
        assert!(!result.is_valid);  // Conservative allows only 5% deviation
        assert!(result.errors[0].contains("deviates"));
    }

    #[tokio::test]
    async fn test_duplicate_order() {
        let validator = OrderValidator::default_policy().unwrap();
        
        let params = OrderParams {
            token: test_token(),
            price: 100.0,
            size: 1.0,
            slippage_bps: Some(50),
            oracle_price: None,
        };

        // First order should pass
        let result1 = validator.validate(&params).await.unwrap();
        assert!(result1.is_valid);

        // Duplicate should fail
        let result2 = validator.validate(&params).await.unwrap();
        assert!(!result2.is_valid);
        assert!(result2.errors[0].contains("Duplicate"));
    }

    #[tokio::test]
    async fn test_token_whitelist() {
        let mut policy = OrderValidationPolicy::default();
        let allowed_token = test_token();
        policy.allowed_tokens = vec![allowed_token];
        
        let validator = OrderValidator::new(policy).unwrap();
        
        // Allowed token should pass
        let params1 = OrderParams {
            token: allowed_token,
            price: 100.0,
            size: 1.0,
            slippage_bps: Some(50),
            oracle_price: None,
        };
        let result1 = validator.validate(&params1).await.unwrap();
        assert!(result1.is_valid);
        
        // Different token should fail
        let other_token = Pubkey::new_unique();
        let params2 = OrderParams {
            token: other_token,
            price: 100.0,
            size: 1.0,
            slippage_bps: Some(50),
            oracle_price: None,
        };
        let result2 = validator.validate(&params2).await.unwrap();
        assert!(!result2.is_valid);
        assert!(result2.errors[0].contains("not in whitelist"));
    }

    #[test]
    fn test_policy_validation() {
        let mut policy = OrderValidationPolicy::default();
        assert!(policy.validate().is_ok());
        
        // Invalid: max < min
        policy.max_notional_usd = 100.0;
        policy.min_notional_usd = 200.0;
        assert!(policy.validate().is_err());
    }
}
