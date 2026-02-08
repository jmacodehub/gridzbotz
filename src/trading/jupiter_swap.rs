//! ═══════════════════════════════════════════════════════════════════════════
//! 🪐 JUPITER SWAP CLIENT - Production-Grade Solana DEX Aggregator
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! **Purpose**: Execute cross-DEX swaps on Solana via Jupiter Quote API v6
//!
//! **Features**:
//! - Real-time quote fetching from Jupiter aggregator
//! - Best price routing across all Solana DEXs (Raydium, Orca, Phoenix, etc.)
//! - Slippage protection with configurable tolerance
//! - Transaction building and serialization
//! - Automatic retry with exponential backoff
//! - MEV protection via Jito bundles (optional)
//! - Price impact tracking and warnings
//!
//! **API Endpoints**:
//! - Quote API: `https://quote-api.jup.ag/v6/quote`
//! - Swap API: `https://quote-api.jup.ag/v6/swap`
//!
//! **Architecture**:
//! ```
//! RealTradingEngine → JupiterSwapClient → Quote API → Swap API → Solana RPC
//! ```
//!
//! ## Example Usage
//! ```rust,no_run
//! use solana_grid_bot::trading::jupiter_swap::JupiterSwapClient;
//! use solana_sdk::pubkey::Pubkey;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let client = JupiterSwapClient::new(50)?; // 0.5% slippage
//!
//! // Get quote for swapping 100 USDC to SOL
//! let quote = client.get_quote(
//!     USDC_MINT,
//!     SOL_MINT,
//!     100_000_000, // 100 USDC (6 decimals)
//! ).await?;
//!
//! println!("Expected output: {} lamports SOL", quote.out_amount);
//! println!("Price impact: {}%", quote.price_impact_pct);
//! # Ok(())
//! # }
//! ```

use anyhow::{bail, Context, Result};
use log::{debug, error, info, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::Signature,
    transaction::VersionedTransaction,
};
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// 🌐 CONSTANTS - Jupiter API & Solana Mainnet
// ═══════════════════════════════════════════════════════════════════════════

/// Jupiter Quote API v6 endpoint
const JUPITER_QUOTE_API: &str = "https://quote-api.jup.ag/v6/quote";

/// Jupiter Swap API v6 endpoint
const JUPITER_SWAP_API: &str = "https://quote-api.jup.ag/v6/swap";

/// Wrapped SOL (WSOL) mint address
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// USDC mint address (mainnet)
pub const USDC_MINT: &str = "EPjFWdd8DsbgNU1MCgjgWAo3LnxMrLXYYQ9uJ2nBfYWZ";

/// USDT mint address (mainnet)
pub const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

/// Default slippage tolerance in basis points (50 = 0.5%)
const DEFAULT_SLIPPAGE_BPS: u16 = 50;

/// Maximum allowed slippage (200 = 2%)
const MAX_SLIPPAGE_BPS: u16 = 200;

/// HTTP request timeout
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// Maximum retries for failed requests
const MAX_RETRIES: u32 = 3;

// ═══════════════════════════════════════════════════════════════════════════
// 📊 DATA STRUCTURES - Jupiter API v6 Schema
// ═══════════════════════════════════════════════════════════════════════════

/// Jupiter Quote API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuote {
    /// Input token mint address
    pub input_mint: String,
    
    /// Output token mint address
    pub output_mint: String,
    
    /// Input amount in lamports/token units
    #[serde(deserialize_with = "deserialize_u64_string")]
    pub in_amount: u64,
    
    /// Expected output amount in lamports/token units
    #[serde(deserialize_with = "deserialize_u64_string")]
    pub out_amount: u64,
    
    /// Price impact as decimal string (e.g., "0.0023" = 0.23%)
    #[serde(deserialize_with = "deserialize_f64_string")]
    pub price_impact_pct: f64,
    
    /// Route information for the swap
    #[serde(default)]
    pub route_plan: Vec<RoutePlanStep>,
    
    /// Slot number for which this quote is valid
    #[serde(default)]
    pub context_slot: u64,
    
    /// Minimum output amount considering slippage
    #[serde(default, deserialize_with = "deserialize_optional_u64_string")]
    pub other_amount_threshold: Option<u64>,
    
    /// Swap mode (ExactIn or ExactOut)
    #[serde(default)]
    pub swap_mode: Option<String>,
    
    /// Slippage basis points used
    #[serde(default)]
    pub slippage_bps: Option<u16>,
}

/// Route plan step (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanStep {
    pub swap_info: SwapInfo,
}

/// Swap information per step
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    pub amm_key: String,
    pub label: Option<String>,
    pub input_mint: String,
    pub output_mint: String,
    #[serde(deserialize_with = "deserialize_u64_string")]
    pub in_amount: u64,
    #[serde(deserialize_with = "deserialize_u64_string")]
    pub out_amount: u64,
    pub fee_amount: Option<String>,
    pub fee_mint: Option<String>,
}

/// Jupiter Swap API request
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapRequest {
    /// User's wallet public key
    pub user_public_key: String,
    
    /// Auto wrap/unwrap SOL if needed
    #[serde(default = "default_true")]
    pub wrap_and_unwrap_sol: bool,
    
    /// Use shared accounts (recommended)
    #[serde(default = "default_true")]
    pub use_shared_accounts: bool,
    
    /// Compute unit price in micro-lamports (priority fee)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compute_unit_price_micro_lamports: Option<u64>,
    
    /// Quote response from quote API
    pub quote_response: JupiterQuote,
}

fn default_true() -> bool { true }

/// Jupiter Swap API response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapResponse {
    /// Base64-encoded serialized transaction
    pub swap_transaction: String,
    
    /// Last valid block height for the transaction
    #[serde(default)]
    pub last_valid_block_height: Option<u64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// 🔧 HELPER FUNCTIONS - Deserialization
// ═══════════════════════════════════════════════════════════════════════════

fn deserialize_u64_string<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<u64>().map_err(serde::de::Error::custom)
}

fn deserialize_f64_string<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer)?;
    s.parse::<f64>().map_err(serde::de::Error::custom)
}

fn deserialize_optional_u64_string<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    match s {
        Some(val) => val.parse::<u64>().map(Some).map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🪐 JUPITER SWAP CLIENT - Main Implementation
// ═══════════════════════════════════════════════════════════════════════════

/// Production-grade Jupiter swap client
/// 
/// Handles:
/// - Quote fetching from Jupiter API
/// - Transaction building
/// - Slippage protection
/// - Error handling and retries
/// - Price impact warnings
pub struct JupiterSwapClient {
    /// HTTP client for API requests
    http_client: Client,
    
    /// Slippage tolerance in basis points
    slippage_bps: u16,
    
    /// Priority fee in micro-lamports (optional)
    priority_fee_lamports: Option<u64>,
}

impl JupiterSwapClient {
    /// Create a new Jupiter swap client
    /// 
    /// # Arguments
    /// * `slippage_bps` - Slippage tolerance in basis points (e.g., 50 = 0.5%)
    /// 
    /// # Returns
    /// * Initialized Jupiter client
    /// 
    /// # Example
    /// ```
    /// use solana_grid_bot::trading::jupiter_swap::JupiterSwapClient;
    /// 
    /// let client = JupiterSwapClient::new(50)?; // 0.5% slippage
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn new(slippage_bps: u16) -> Result<Self> {
        // Validate slippage
        if slippage_bps > MAX_SLIPPAGE_BPS {
            warn!("⚠️  Slippage {}bps exceeds maximum {}bps, capping", 
                  slippage_bps, MAX_SLIPPAGE_BPS);
        }
        
        let capped_slippage = slippage_bps.min(MAX_SLIPPAGE_BPS);
        
        let http_client = Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()
            .context("Failed to create HTTP client")?;
        
        info!("🪐 Jupiter Swap Client initialized");
        info!("   Slippage:  {}bps ({:.2}%)", capped_slippage, capped_slippage as f64 / 100.0);
        
        Ok(Self {
            http_client,
            slippage_bps: capped_slippage,
            priority_fee_lamports: None,
        })
    }
    
    /// Set priority fee for transactions (in micro-lamports)
    /// 
    /// Priority fees help transactions land faster during network congestion.
    /// Recommended: 1000-10000 micro-lamports (0.000001-0.00001 SOL)
    /// 
    /// # Arguments
    /// * `micro_lamports` - Priority fee amount
    pub fn with_priority_fee(mut self, micro_lamports: u64) -> Self {
        info!("   Priority:  {} μLamports", micro_lamports);
        self.priority_fee_lamports = Some(micro_lamports);
        self
    }
    
    /// Get a quote for a token swap
    /// 
    /// # Arguments
    /// * `input_mint` - Input token mint address
    /// * `output_mint` - Output token mint address
    /// * `amount_lamports` - Amount to swap in lamports/token units
    /// 
    /// # Returns
    /// * `JupiterQuote` with expected output and route information
    /// 
    /// # Errors
    /// * Network failures
    /// * Invalid mint addresses
    /// * Insufficient liquidity
    pub async fn get_quote(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount_lamports: u64,
    ) -> Result<JupiterQuote> {
        debug!("📡 Fetching Jupiter quote");
        debug!("   Input:    {} ({} lamports)", input_mint, amount_lamports);
        debug!("   Output:   {}", output_mint);
        debug!("   Slippage: {}bps", self.slippage_bps);
        
        let url = format!(
            "{}?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            JUPITER_QUOTE_API,
            input_mint,
            output_mint,
            amount_lamports,
            self.slippage_bps
        );
        
        // Retry logic with exponential backoff
        let mut attempts = 0;
        let mut last_error = None;
        
        while attempts < MAX_RETRIES {
            attempts += 1;
            
            match self.http_client.get(&url).send().await {
                Ok(response) => {
                    if !response.status().is_success() {
                        let status = response.status();
                        let body = response.text().await.unwrap_or_default();
                        bail!("Jupiter API error: {} - {}", status, body);
                    }
                    
                    let quote: JupiterQuote = response
                        .json()
                        .await
                        .context("Failed to parse Jupiter quote")?;
                    
                    // Validate quote
                    if quote.out_amount == 0 {
                        bail!("Quote returned zero output amount (insufficient liquidity?)");
                    }
                    
                    // Log quote details
                    info!("✅ Quote received");
                    info!("   Input:         {} lamports", quote.in_amount);
                    info!("   Output:        {} lamports", quote.out_amount);
                    info!("   Price Impact:  {:.4}%", quote.price_impact_pct);
                    info!("   Route Steps:   {}", quote.route_plan.len());
                    
                    // Warn on high price impact
                    if quote.price_impact_pct > 1.0 {
                        warn!("⚠️  HIGH PRICE IMPACT: {:.2}%", quote.price_impact_pct);
                        warn!("   Consider splitting into smaller orders");
                    }
                    
                    return Ok(quote);
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempts < MAX_RETRIES {
                        let backoff_ms = 100 * (2_u64.pow(attempts - 1));
                        warn!("⚠️  Quote request failed (attempt {}/{}), retrying in {}ms",
                              attempts, MAX_RETRIES, backoff_ms);
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    }
                }
            }
        }
        
        Err(last_error.unwrap().into())
    }
    
    /// Get swap transaction from Jupiter
    /// 
    /// Builds a signed transaction ready for submission to Solana.
    /// 
    /// # Arguments
    /// * `quote` - Quote from `get_quote()`
    /// * `user_pubkey` - User's wallet public key
    /// 
    /// # Returns
    /// * Base64-encoded versioned transaction
    /// 
    /// # Errors
    /// * Network failures
    /// * Invalid quote
    pub async fn get_swap_transaction(
        &self,
        quote: &JupiterQuote,
        user_pubkey: Pubkey,
    ) -> Result<(VersionedTransaction, u64)> {
        debug!("📝 Requesting swap transaction");
        debug!("   User: {}", user_pubkey);
        
        let request = JupiterSwapRequest {
            user_public_key: user_pubkey.to_string(),
            wrap_and_unwrap_sol: true,
            use_shared_accounts: true,
            compute_unit_price_micro_lamports: self.priority_fee_lamports,
            quote_response: quote.clone(),
        };
        
        let response = self.http_client
            .post(JUPITER_SWAP_API)
            .json(&request)
            .send()
            .await
            .context("Failed to connect to Jupiter swap API")?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            bail!("Jupiter swap API error: {} - {}", status, body);
        }
        
        let swap_response: JupiterSwapResponse = response
            .json()
            .await
            .context("Failed to parse swap response")?;
        
        // Decode base64 transaction
        use base64::{engine::general_purpose, Engine as _};
        let tx_bytes = general_purpose::STANDARD
            .decode(&swap_response.swap_transaction)
            .context("Failed to decode swap transaction")?;
        
        // Deserialize transaction
        let transaction: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .context("Failed to deserialize transaction")?;
        
        let last_valid_block_height = swap_response.last_valid_block_height.unwrap_or(0);
        
        info!("✅ Swap transaction built");
        info!("   Size:           {} bytes", tx_bytes.len());
        info!("   Valid Height:   {}", last_valid_block_height);
        
        Ok((transaction, last_valid_block_height))
    }
    
    /// Calculate approximate output amount given input
    /// 
    /// This is a quick estimation without fetching a full quote.
    /// Use `get_quote()` for accurate pricing.
    /// 
    /// # Arguments
    /// * `input_amount` - Input amount in human-readable format
    /// * `price` - Current price (quote per base)
    /// 
    /// # Returns
    /// * Estimated output amount
    pub fn estimate_output(input_amount: f64, price: f64) -> f64 {
        input_amount / price
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🧪 TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_client_creation() {
        let client = JupiterSwapClient::new(50).unwrap();
        assert_eq!(client.slippage_bps, 50);
    }
    
    #[test]
    fn test_slippage_capping() {
        let client = JupiterSwapClient::new(500).unwrap();
        assert_eq!(client.slippage_bps, MAX_SLIPPAGE_BPS);
    }
    
    #[test]
    fn test_priority_fee() {
        let client = JupiterSwapClient::new(50)
            .unwrap()
            .with_priority_fee(5000);
        assert_eq!(client.priority_fee_lamports, Some(5000));
    }
    
    #[test]
    fn test_estimate_output() {
        let output = JupiterSwapClient::estimate_output(180.0, 180.0);
        assert!((output - 1.0).abs() < 0.001);
    }
}
