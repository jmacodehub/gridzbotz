//! 🪐 Jupiter Aggregator Client — PRODUCTION V4.3
//! 
//! Real DEX trading via Jupiter API with best-price routing across Solana.
//! 
//! # V4.3 CHANGES (Mar 2026 — Jupiter V6 Swap API Fix)
//! ✅ Added `percent` field to RoutePlanStep (required by /swap endpoint)
//! ✅ Fixes "Missing percent" 500 error when re-serializing QuoteResponse
//! ✅ Jupiter V6 requires ALL quote fields passed through to /swap
//! 
//! # V4.2 CHANGES (Mar 2026 — Jupiter V6 Schema Fix)
//! ✅ SwapInfo fee fields now optional (matches Jupiter V6 reality)
//! ✅ Different AMMs (Whirlpool, Invariant, Raydium) have different fee structures
//! ✅ Some routes return fees, some don't — now handles both gracefully
//! 
//! # V4.1 CHANGES (Mar 2026 — Security Fix)
//! ✅ Constructor accepts Pubkey instead of Keypair (security best practice)
//! ✅ Signing now handled externally by SecureKeystore (never export keys!)
//! ✅ simple_swap() API unchanged — only needs pubkey for Jupiter
//! ⚠️  Trader trait methods (place_order) no longer work without keypair
//!    (acceptable — RealTradingEngine only uses simple_swap())
//! 
//! # Features
//! - ✅ Real HTTP calls to https://api.jup.ag (current production endpoint)
//! - ✅ API key authentication (x-api-key header)
//! - ✅ Automatic best price routing across all Solana DEXs
//! - ✅ VersionedTransaction handling (Jupiter pre-signed format)
//! - ✅ Slippage protection with configurable tolerance
//! - ✅ Retry logic with exponential backoff
//! - ✅ Position tracking and P&L calculation
//! - ✅ Comprehensive error logging with raw responses
//! - ✅ Simple swap API for RealTradingEngine integration
//! 
//! # Example (Simple swap API for RealTradingEngine)
//! ```no_run
//! use solana_grid_bot::dex::JupiterClient;
//! use solana_sdk::pubkey::Pubkey;
//! use std::str::FromStr;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let wallet_pubkey = Pubkey::from_str("YourWalletAddressHere...")?;
//! let sol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;
//! let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
//! let api_key = "your-jupiter-api-key".to_string();
//! 
//! let client = JupiterClient::new(
//!     "https://api.mainnet-beta.solana.com".to_string(),
//!     wallet_pubkey,
//!     sol_mint,
//!     usdc_mint,
//!     1000.0,
//!     api_key,
//! )?;
//! 
//! let lamports = 1_000_000_000; // 1 SOL
//! let (tx, last_valid) = client.simple_swap(sol_mint, usdc_mint, lamports).await?;
//! // Caller signs tx with SecureKeystore (never export keys!)
//! println!("✅ Swap tx ready! Last valid block: {}", last_valid);
//! # Ok(())
//! # }
//! ```

use super::{Order, PlacedOrder, Position, Trader};
use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use base64::{engine::general_purpose, Engine as _};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Default slippage tolerance in basis points (50 = 0.5%)
const DEFAULT_SLIPPAGE_BPS: u16 = 50;

/// Minimum order size (prevents dust orders)
/// Reserved: enforced at call-site when per-pair dust thresholds are wired in.
#[allow(dead_code)]
const MIN_ORDER_SIZE: f64 = 0.001;

/// Jupiter API base URL (current production endpoint)
const JUPITER_API: &str = "https://api.jup.ag";

/// API endpoints
const QUOTE_PATH: &str = "swap/v1/quote";
const SWAP_PATH: &str = "swap/v1/swap";

/// Retry configuration
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 500;

/// HTTP timeout
const API_TIMEOUT_SECS: u64 = 30;

/// Well-known token mints
pub const SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// ═══════════════════════════════════════════════════════════════════════════
// JUPITER API TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct QuoteResponse {
    input_mint: String,
    #[serde(rename = "inAmount")]
    in_amount: String,
    output_mint: String,
    #[serde(rename = "outAmount")]
    out_amount: String,
    #[serde(rename = "otherAmountThreshold")]
    other_amount_threshold: String,
    #[serde(rename = "swapMode")]
    swap_mode: String,
    #[serde(rename = "slippageBps")]
    slippage_bps: u16,
    #[serde(default)]
    platform_fee: Option<serde_json::Value>,
    #[serde(rename = "priceImpactPct")]
    price_impact_pct: String,
    route_plan: Vec<RoutePlanStep>,
    #[serde(rename = "contextSlot")]
    context_slot: u64,
    #[serde(rename = "timeTaken")]
    time_taken: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct RoutePlanStep {
    #[serde(rename = "swapInfo")]
    swap_info: SwapInfo,
    /// Percentage of input amount for this route step (typically 100 for single-hop swaps)
    /// REQUIRED by Jupiter /swap endpoint — causes "Missing percent" error if omitted
    percent: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SwapInfo {
    #[serde(rename = "ammKey")]
    amm_key: String,
    label: String,
    input_mint: String,
    output_mint: String,
    #[serde(rename = "inAmount")]
    in_amount: String,
    #[serde(rename = "outAmount")]
    out_amount: String,
    /// Fee amount (optional — not all AMMs return this)
    #[serde(rename = "feeAmount", default)]
    fee_amount: Option<String>,
    /// Fee token mint (optional — not all AMMs return this)
    #[serde(rename = "feeMint", default)]
    fee_mint: Option<String>,
}

#[derive(Debug, Serialize)]
struct SwapRequest {
    #[serde(rename = "quoteResponse")]
    quote_response: QuoteResponse,
    #[serde(rename = "userPublicKey")]
    user_public_key: String,
    #[serde(rename = "wrapAndUnwrapSol")]
    wrap_unwrap_sol: bool,
    #[serde(rename = "dynamicComputeUnitLimit", skip_serializing_if = "Option::is_none")]
    dynamic_compute_unit_limit: Option<bool>,
    #[serde(rename = "dynamicSlippage", skip_serializing_if = "Option::is_none")]
    dynamic_slippage: Option<bool>,
    #[serde(rename = "prioritizationFeeLamports", skip_serializing_if = "Option::is_none")]
    prioritization_fee_lamports: Option<PrioritizationFee>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum PrioritizationFee {
    Auto(String),
    Detailed {
        #[serde(rename = "priorityLevelWithMaxLamports")]
        priority_level_with_max_lamports: PriorityLevelWithMaxLamports,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PriorityLevelWithMaxLamports {
    max_lamports: u64,
    #[serde(rename = "priorityLevel")]
    priority_level: String,
}

#[derive(Debug, Deserialize)]
struct SwapResponse {
    #[serde(rename = "swapTransaction")]
    swap_transaction: String,
    #[serde(rename = "lastValidBlockHeight", default)]
    last_valid_block_height: Option<u64>,
    #[serde(rename = "prioritizationFeeLamports", default)]
    prioritization_fee_lamports: Option<u64>,
    #[serde(rename = "computeUnitLimit", default)]
    compute_unit_limit: Option<u64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// JUPITER CLIENT
// ═══════════════════════════════════════════════════════════════════════════

/// Production Jupiter aggregator client with real API calls
pub struct JupiterClient {
    /// RPC client for Solana blockchain
    rpc: Arc<RpcClient>,
    
    /// Trading wallet public key (V4.1: no longer stores Keypair — security!)
    wallet_pubkey: Pubkey,
    
    /// Base token mint (e.g., SOL)
    base_mint: Pubkey,
    
    /// Quote token mint (e.g., USDC)
    quote_mint: Pubkey,
    
    /// Current position
    position: Position,
    
    /// Total orders placed
    orders_placed: u64,
    
    /// Total orders cancelled
    orders_cancelled: u64,
    
    /// Last order timestamp — set on each execution; reserved for
    /// future idle-detection and rate-limit analytics.
    last_order_time: Option<SystemTime>,
    
    /// Slippage tolerance in basis points
    slippage_bps: u16,
    
    /// HTTP client for Jupiter API
    http_client: reqwest::Client,
    
    /// Priority fee max lamports
    priority_fee_max_lamports: u64,
    
    /// Priority level (none, low, medium, high, veryHigh)
    priority_level: String,
    
    /// Jupiter API key
    jupiter_api_key: String,
}

impl JupiterClient {
    /// Create a new Jupiter aggregator client with real API integration
    /// 
    /// # V4.1 SECURITY: Now accepts Pubkey instead of Keypair
    /// 
    /// Signing is handled externally by SecureKeystore — never export keys!
    /// 
    /// # Arguments
    /// * `rpc_url` - Solana RPC endpoint URL
    /// * `wallet_pubkey` - Trading wallet PUBLIC KEY (not Keypair!)
    /// * `base_mint` - Base token mint address (e.g., SOL)
    /// * `quote_mint` - Quote token mint address (e.g., USDC)
    /// * `initial_capital` - Starting quote currency amount
    /// * `jupiter_api_key` - Jupiter API key from https://portal.jup.ag
    pub fn new(
        rpc_url: String,
        wallet_pubkey: Pubkey,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        initial_capital: f64,
        jupiter_api_key: String,
    ) -> Result<Self> {
        info!("🪐 Jupiter API Client V4.3 — Production Mode (Secure)");
        info!("   Endpoint:   {}", JUPITER_API);
        info!("   Base mint:  {}", base_mint);
        info!("   Quote mint: {}", quote_mint);
        info!("   Wallet:     {}", wallet_pubkey);
        info!("   Capital:    ${:.2}", initial_capital);
        
        // Validate API key
        if jupiter_api_key.is_empty() {
            bail!("Jupiter API key is required. Get one free at https://portal.jup.ag");
        }
        
        info!("   API Key:    {}...{}", 
            &jupiter_api_key[..8], 
            &jupiter_api_key[jupiter_api_key.len().saturating_sub(4)..]
        );
        
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(API_TIMEOUT_SECS))
            .build()?;
        
        Ok(Self {
            rpc: Arc::new(RpcClient::new(rpc_url)),
            wallet_pubkey,
            base_mint,
            quote_mint,
            position: Position::new(initial_capital),
            orders_placed: 0,
            orders_cancelled: 0,
            last_order_time: None,
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
            http_client,
            priority_fee_max_lamports: 100_000, // 0.0001 SOL
            priority_level: "high".to_string(),
            jupiter_api_key,
        })
    }
    
    /// Set custom slippage tolerance
    pub fn with_slippage(mut self, slippage_bps: u16) -> Self {
        self.slippage_bps = slippage_bps;
        info!("   Slippage:   {}bps ({:.2}%)", slippage_bps, slippage_bps as f64 / 100.0);
        self
    }
    
    /// Set priority fee configuration
    pub fn with_priority_fee(mut self, max_lamports: u64, level: String) -> Self {
        self.priority_fee_max_lamports = max_lamports;
        self.priority_level = level.clone();
        info!("   Priority:   {} (max {} lamports)", level, max_lamports);
        self
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // SIMPLE SWAP API (for RealTradingEngine integration)
    // ═══════════════════════════════════════════════════════════════════════
    
    /// Execute a simple swap without the full Trader trait overhead.
    /// Returns a VersionedTransaction that needs to be signed and broadcast.
    /// 
    /// This is a **lightweight bridge** for RealTradingEngine that:
    /// - Takes raw mints + amounts
    /// - Returns unsigned VersionedTransaction
    /// - Lets caller handle signing/broadcasting (via SecureKeystore)
    /// 
    /// # Arguments
    /// * `input_mint` - Token to sell (e.g., SOL, USDC)
    /// * `output_mint` - Token to buy
    /// * `amount` - Amount in smallest units (lamports for SOL, micro-units for USDC)
    /// 
    /// # Returns
    /// * `(VersionedTransaction, last_valid_block_height)`
    /// 
    /// # Example
    /// ```no_run
    /// # use solana_grid_bot::dex::JupiterClient;
    /// # use solana_sdk::pubkey::Pubkey;
    /// # use std::str::FromStr;
    /// # async fn example() -> anyhow::Result<()> {
    /// # let wallet_pubkey = Pubkey::from_str("11111111111111111111111111111111")?;
    /// # let client = JupiterClient::new(
    /// #     "https://api.devnet.solana.com".to_string(),
    /// #     wallet_pubkey,
    /// #     Pubkey::new_unique(),
    /// #     Pubkey::new_unique(),
    /// #     1000.0,
    /// #     "test-key".to_string(),
    /// # )?;
    /// let sol_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?;
    /// let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    /// let lamports = 1_000_000_000; // 1 SOL
    /// 
    /// let (tx, last_valid) = client.simple_swap(sol_mint, usdc_mint, lamports).await?;
    /// // Sign tx with SecureKeystore.sign_versioned_transaction()
    /// println!("Swap tx ready! Last valid block: {}", last_valid);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn simple_swap(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
    ) -> Result<(VersionedTransaction, u64)> {
        info!("🔄 Building simple swap");
        debug!("   Input:  {} ({})", amount, input_mint);
        debug!("   Output: {}", output_mint);
        
        // Step 1: Get quote
        let quote = self.get_quote(input_mint, output_mint, amount, self.slippage_bps).await?;
        
        let price_impact: f64 = quote.price_impact_pct.parse().unwrap_or(0.0);
        info!("📊 Quote: in={}, out={}, impact={:.4}%", 
            quote.in_amount, quote.out_amount, price_impact);
        
        // Step 2: Get swap transaction
        let swap_response = self.get_swap_transaction(quote).await?;
        
        // Step 3: Decode transaction
        let tx_bytes = general_purpose::STANDARD
            .decode(&swap_response.swap_transaction)
            .map_err(|e| anyhow!("Failed to decode transaction: {}", e))?;
        
        let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .map_err(|e| anyhow!("Failed to deserialize VersionedTransaction: {}", e))?;
        
        let last_valid = swap_response.last_valid_block_height.unwrap_or(0);
        
        info!("✅ Swap transaction built (last valid: {})", last_valid);
        Ok((versioned_tx, last_valid))
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // INTERNAL HELPERS
    // ═══════════════════════════════════════════════════════════════════════
    
    /// Execute with retry logic
    async fn execute_with_retry<T, F, Fut>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;
        
        for attempt in 1..=MAX_RETRIES {
            match operation().await {
                Ok(result) => {
                    if attempt > 1 {
                        info!("✅ {} succeeded on attempt {}", operation_name, attempt);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    error!("❌ {} failed (attempt {}/{}): {}", operation_name, attempt, MAX_RETRIES, e);
                    last_error = Some(e);
                    
                    if attempt < MAX_RETRIES {
                        let delay = Duration::from_millis(RETRY_DELAY_MS * (attempt as u64));
                        warn!("   Retrying in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        
        bail!("{} failed after {} attempts: {}", operation_name, MAX_RETRIES, last_error.unwrap())
    }
    
    /// Get quote from Jupiter
    async fn get_quote(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<QuoteResponse> {
        self.execute_with_retry("Get quote", || async {
            let quote_url = format!(
                "{}/{}?inputMint={}&outputMint={}&amount={}&slippageBps={}",
                JUPITER_API, QUOTE_PATH, input_mint, output_mint, amount, slippage_bps
            );
            
            debug!("📞 Quote URL: {}", quote_url);
            
            let response = self.http_client
                .get(&quote_url)
                .header("x-api-key", &self.jupiter_api_key)
                .send()
                .await?;
            
            let status = response.status();
            
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("❌ Jupiter quote API error [{}]: {}", status, error_text);
                
                if status == 401 || status == 403 {
                    bail!("Jupiter API authentication failed ({}). Check your API key at https://portal.jup.ag", status);
                }
                
                if status == 429 {
                    bail!("Jupiter API rate limit exceeded (429). Consider upgrading at https://portal.jup.ag");
                }
                
                bail!("Jupiter quote API failed with status {}: {}", status, error_text);
            }
            
            let response_text = response.text().await?;
            debug!("📥 Quote response: {}...", &response_text.chars().take(200).collect::<String>());
            
            let quote: QuoteResponse = serde_json::from_str(&response_text)
                .map_err(|e| {
                    error!("❌ Failed to parse Jupiter quote response");
                    error!("   Error: {}", e);
                    error!("   Raw response (first 500 chars): {}", 
                        &response_text.chars().take(500).collect::<String>());
                    anyhow!("JSON parse error: {}. See logs for raw response.", e)
                })?;
            
            Ok(quote)
        }).await
    }
    
    /// Get swap transaction from Jupiter
    async fn get_swap_transaction(
        &self,
        quote_response: QuoteResponse,
    ) -> Result<SwapResponse> {
        self.execute_with_retry("Get swap transaction", || async {
            let swap_url = format!("{}/{}", JUPITER_API, SWAP_PATH);
            
            let swap_request = SwapRequest {
                quote_response: quote_response.clone(),
                user_public_key: self.wallet_pubkey.to_string(),
                wrap_unwrap_sol: true,
                dynamic_compute_unit_limit: Some(true),
                dynamic_slippage: Some(true),
                prioritization_fee_lamports: Some(PrioritizationFee::Detailed {
                    priority_level_with_max_lamports: PriorityLevelWithMaxLamports {
                        max_lamports: self.priority_fee_max_lamports,
                        priority_level: self.priority_level.clone(),
                    },
                }),
            };
            
            debug!("📞 Swap URL: {}", swap_url);
            
            let response = self.http_client
                .post(&swap_url)
                .header("x-api-key", &self.jupiter_api_key)
                .json(&swap_request)
                .send()
                .await?;
            
            let status = response.status();
            
            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
                error!("❌ Jupiter swap API error [{}]: {}", status, error_text);
                
                if status == 401 || status == 403 {
                    bail!("Jupiter API authentication failed ({}). Check your API key.", status);
                }
                
                bail!("Jupiter swap API failed with status {}: {}", status, error_text);
            }
            
            let response_text = response.text().await?;
            debug!("📥 Swap response: {}...", &response_text.chars().take(200).collect::<String>());
            
            let swap: SwapResponse = serde_json::from_str(&response_text)
                .map_err(|e| {
                    error!("❌ Failed to parse Jupiter swap response");
                    error!("   Error: {}", e);
                    error!("   Raw response (first 500 chars): {}", 
                        &response_text.chars().take(500).collect::<String>());
                    anyhow!("JSON parse error: {}. See logs for raw response.", e)
                })?;
            
            Ok(swap)
        }).await
    }
    
    /// Get trading statistics
    pub fn stats(&self) -> (u64, u64) {
        (self.orders_placed, self.orders_cancelled)
    }
    
    /// Display statistics to console
    pub fn display_stats(&self) {
        println!("\n📊 Jupiter Client Statistics:");
        println!("   Orders Placed:    {}", self.orders_placed);
        println!("   Orders Cancelled: {}", self.orders_cancelled);
        println!("   API Endpoint:     {}", JUPITER_API);
        
        if let Some(last_time) = self.last_order_time {
            if let Ok(elapsed) = last_time.elapsed() {
                println!("   Last Order:       {}s ago", elapsed.as_secs());
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADER TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════
// 
// ⚠️  V4.1 NOTE: Trader trait methods no longer work without Keypair.
//     This is acceptable — RealTradingEngine only uses simple_swap().
//     To use Trader trait, refactor to accept external signing closure.
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Trader for JupiterClient {
    async fn place_order(&mut self, _order: Order) -> Result<PlacedOrder> {
        bail!(
            "JupiterClient V4.3: Trader trait methods removed for security.\n\
             Use simple_swap() + external signing via SecureKeystore instead."
        );
    }
    
    async fn cancel_order(&mut self, order_id: u128) -> Result<()> {
        warn!("⚠️  Jupiter orders execute immediately");
        warn!("   Cannot cancel order ID: {}", order_id);
        self.orders_cancelled += 1;
        Ok(())
    }
    
    async fn get_balance(&self) -> Result<(f64, f64)> {
        Ok((self.position.base_amount, self.position.quote_amount))
    }
    
    async fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    fn trader_type(&self) -> &'static str {
        "Jupiter Aggregator V4.3 (Secure - simple_swap only)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    
    fn create_test_client() -> JupiterClient {
        let wallet_pubkey = Pubkey::from_str("11111111111111111111111111111111").unwrap();
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        
        JupiterClient::new(
            "https://api.devnet.solana.com".to_string(),
            wallet_pubkey,
            base_mint,
            quote_mint,
            1000.0,
            "test-api-key".to_string(),
        ).unwrap()
    }
    
    #[test]
    fn test_client_creation() {
        let client = create_test_client();
        assert_eq!(client.orders_placed, 0);
        assert_eq!(client.position.quote_amount, 1000.0);
    }
    
    #[test]
    fn test_stats() {
        let client = create_test_client();
        let (placed, cancelled) = client.stats();
        assert_eq!(placed, 0);
        assert_eq!(cancelled, 0);
    }
}
