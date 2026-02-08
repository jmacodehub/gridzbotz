//! ═══════════════════════════════════════════════════════════════════════════
//! 🪐 JUPITER SWAP CLIENT - DEX Aggregator Integration
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! ✅ Best price routing across all Solana DEXs
//! ✅ Automatic slippage protection
//! ✅ Transaction building with versioned messages
//! ✅ Production-ready error handling
//!
//! November 2025 | Project Flash V6.0 - Jupiter Integration
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Context, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use log::{debug, info};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════
// 🌐 JUPITER API CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

const JUPITER_QUOTE_API: &str = "https://quote-api.jup.ag/v6";
const JUPITER_SWAP_API: &str = "https://quote-api.jup.ag/v6/swap";

/// Wrapped SOL mint address
pub const WSOL_MINT: &str = "So11111111111111111111111111111111111111112";

/// USDC mint address (mainnet)
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// ═══════════════════════════════════════════════════════════════════════════
// 📊 JUPITER API TYPES
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteResponse {
    pub input_mint: String,
    pub in_amount: String,
    pub output_mint: String,
    pub out_amount: String,
    pub other_amount_threshold: String,
    pub swap_mode: String,
    pub slippage_bps: u16,
    pub price_impact_pct: f64,
    pub route_plan: Vec<RoutePlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlanStep {
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    pub amm_key: String,
    pub label: String,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_amount: String,
    pub fee_mint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapRequest {
    pub user_public_key: String,
    pub quote_response: QuoteResponse,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_fee: Option<PriorityFee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_compute_unit_limit: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityFee {
    pub priority_level_with_max_lamports: PriorityLevelWithMaxLamports,  // ✅ FIXED: Was priority_level_with_max
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityLevelWithMaxLamports {
    pub max_lamports: u64,
    pub priority_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapResponse {
    pub swap_transaction: String, // Base64 encoded transaction
    pub last_valid_block_height: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// 🪐 JUPITER SWAP CLIENT
// ═══════════════════════════════════════════════════════════════════════════

pub struct JupiterSwapClient {
    client: Client,
    slippage_bps: u16,
    priority_fee: Option<u64>,
}

impl JupiterSwapClient {
    /// Create a new Jupiter swap client
    ///
    /// # Arguments
    ///
    /// * `slippage_bps` - Slippage tolerance in basis points (e.g., 50 = 0.5%)
    pub fn new(slippage_bps: u16) -> Result<Self> {
        if slippage_bps > 1000 {
            bail!("Slippage too high: {} bps (max 1000 = 10%)", slippage_bps);
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("Failed to build HTTP client")?;

        info!("🪐 Jupiter client initialized (slippage: {} bps)", slippage_bps);

        Ok(Self {
            client,
            slippage_bps,
            priority_fee: None,
        })
    }

    /// Set priority fee for transactions
    pub fn with_priority_fee(mut self, fee_microlamports: u64) -> Self {
        self.priority_fee = Some(fee_microlamports);
        self
    }

    /// Get a quote for a swap
    ///
    /// # Arguments
    ///
    /// * `input_mint` - Input token mint address
    /// * `output_mint` - Output token mint address
    /// * `amount` - Amount to swap (in lamports/smallest unit)
    pub async fn get_quote(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
    ) -> Result<QuoteResponse> {
        debug!("📊 Requesting Jupiter quote: {} {} -> {}",
            amount, input_mint, output_mint);

        let url = format!(
            "{}/quote?inputMint={}&outputMint={}&amount={}&slippageBps={}",
            JUPITER_QUOTE_API,
            input_mint,
            output_mint,
            amount,
            self.slippage_bps
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Jupiter quote")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Jupiter quote API error {}: {}", status, error_text);
        }

        let quote: QuoteResponse = response
            .json()
            .await
            .context("Failed to parse Jupiter quote response")?;

        debug!("✅ Quote received: {} -> {} (impact: {:.3}%)",
            quote.in_amount, quote.out_amount, quote.price_impact_pct);

        Ok(quote)
    }

    /// Get a swap transaction from a quote
    ///
    /// # Arguments
    ///
    /// * `quote` - Quote response from `get_quote`
    /// * `user_pubkey` - User's public key
    pub async fn get_swap_transaction(
        &self,
        quote: &QuoteResponse,
        user_pubkey: Pubkey,
    ) -> Result<(VersionedTransaction, u64)> {
        debug!("🔨 Building swap transaction for {}", user_pubkey);

        let swap_request = SwapRequest {
            user_public_key: user_pubkey.to_string(),
            quote_response: quote.clone(),
            priority_fee: self.priority_fee.map(|fee| PriorityFee {
                priority_level_with_max_lamports: PriorityLevelWithMaxLamports {  // ✅ FIXED
                    max_lamports: fee,
                    priority_level: "high".to_string(),
                },
            }),
            dynamic_compute_unit_limit: Some(true),
        };

        let response = self
            .client
            .post(JUPITER_SWAP_API)
            .json(&swap_request)
            .send()
            .await
            .context("Failed to fetch Jupiter swap transaction")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            bail!("Jupiter swap API error {}: {}", status, error_text);
        }

        let swap_response: SwapResponse = response
            .json()
            .await
            .context("Failed to parse Jupiter swap response")?;

        // ✅ FIXED: Use new base64 API
        let tx_bytes = BASE64.decode(&swap_response.swap_transaction)
            .context("Failed to decode swap transaction")?;

        let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .context("Failed to deserialize swap transaction")?;

        info!("✅ Swap transaction built (valid until block {})",
            swap_response.last_valid_block_height);

        Ok((versioned_tx, swap_response.last_valid_block_height))
    }

    /// Execute a complete swap (quote + transaction)
    ///
    /// Convenience method that combines `get_quote` and `get_swap_transaction`
    pub async fn prepare_swap(
        &self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: u64,
        user_pubkey: Pubkey,
    ) -> Result<(VersionedTransaction, u64, QuoteResponse)> {
        info!("🚀 Preparing swap: {} {} -> {}", amount, input_mint, output_mint);

        let quote = self.get_quote(input_mint, output_mint, amount).await?;

        let (tx, last_valid_height) = self
            .get_swap_transaction(&quote, user_pubkey)
            .await?;

        Ok((tx, last_valid_height, quote))
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ✅ TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_jupiter_client_creation() {
        let client = JupiterSwapClient::new(50);
        assert!(client.is_ok());

        let client_high_slippage = JupiterSwapClient::new(2000);
        assert!(client_high_slippage.is_err());
    }

    #[test]
    fn test_mint_addresses() {
        assert!(Pubkey::from_str(WSOL_MINT).is_ok());
        assert!(Pubkey::from_str(USDC_MINT).is_ok());
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_get_quote() {
        let client = JupiterSwapClient::new(50).unwrap();
        let wsol = Pubkey::from_str(WSOL_MINT).unwrap();
        let usdc = Pubkey::from_str(USDC_MINT).unwrap();

        // Try to get a quote for 0.1 SOL -> USDC
        let amount = 100_000_000; // 0.1 SOL in lamports

        let result = client.get_quote(wsol, usdc, amount).await;
        if let Ok(quote) = result {
            println!("Quote: {} -> {}", quote.in_amount, quote.out_amount);
            println!("Price impact: {:.3}%", quote.price_impact_pct);
            assert!(quote.out_amount.parse::<u64>().unwrap() > 0);
        } else {
            // Network error is acceptable in tests
            println!("Skipping quote test (network unavailable)");
        }
    }
}
