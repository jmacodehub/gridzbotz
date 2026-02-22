//! =============================================================================
//! JUPITER CLIENT - DEX Aggregator Integration V6.0
//!
//! BREAKING CHANGE (Oct 2025): quote-api.jup.ag/v6 is DEAD.
//! New endpoint: https://api.jup.ag/swap/v1  (requires API key)
//! Get your free key at: https://portal.jup.ag
//! Set env var:  export JUPITER_API_KEY=your_key_here
//!
//! [OK] Full VersionedTransaction swap - Address Lookup Tables (ALTs) preserved
//! [OK] Dynamic priority fees via Jupiter "high" level
//! [OK] prepare_swap() all-in-one convenience (quote + tx in single call)
//! [OK] with_priority_fee() / with_priority_level() builder pattern
//! [OK] with_api_key() builder - x-api-key header auth for api.jup.ag
//! [OK] with_resolved_host() - bypass system DNS with a pre-resolved IP
//! [OK] resolve_via_doh() - Cloudflare + Google DoH, full CNAME chain following
//! [OK] Price impact safety guard (warns at > 1%)
//!
//! February 2026 - V6.0 Migrated to api.jup.ag/swap/v1
//! =============================================================================

use anyhow::{bail, Context, Result};
use log::{debug, info, warn};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    transaction::VersionedTransaction,
};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

// =============================================================================
// JUPITER API CONFIGURATION
// =============================================================================

/// New Jupiter Swap API base (quote-api.jup.ag/v6 was deprecated Oct 2025)
pub const JUPITER_API_BASE: &str = "https://api.jup.ag/swap/v1";
const JUPITER_API_TIMEOUT_SECS: u64 = 10;

/// Wrapped SOL mint address (mainnet)
pub const SOL_MINT: &str  = "So11111111111111111111111111111111111111112";
/// Backwards-compatibility alias
pub const WSOL_MINT: &str = SOL_MINT;
/// USDC mint address (mainnet)
pub const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

// =============================================================================
// JUPITER API TYPES
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuoteRequest {
    pub input_mint: String,
    pub output_mint: String,
    pub amount: u64,
    pub slippage_bps: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only_direct_routes: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_legacy_transaction: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterQuoteResponse {
    pub input_mint: String,
    pub in_amount: String,
    pub output_mint: String,
    pub out_amount: String,
    pub other_amount_threshold: String,
    pub swap_mode: String,
    pub slippage_bps: u16,
    pub platform_fee: Option<PlatformFee>,
    pub price_impact_pct: String,
    pub route_plan: Vec<RoutePlan>,
    pub context_slot: Option<u64>,
    pub time_taken: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformFee {
    pub amount: String,
    pub fee_bps: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoutePlan {
    pub swap_info: SwapInfo,
    pub percent: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwapInfo {
    pub amm_key: String,
    pub label: Option<String>,
    pub input_mint: String,
    pub output_mint: String,
    pub in_amount: String,
    pub out_amount: String,
    pub fee_amount: String,
    pub fee_mint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityFee {
    pub priority_level_with_max_lamports: PriorityLevelWithMaxLamports,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriorityLevelWithMaxLamports {
    pub max_lamports: u64,
    pub priority_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapRequest {
    pub quote_response: JupiterQuoteResponse,
    pub user_public_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap_and_unwrap_sol: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority_fee: Option<PriorityFee>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_compute_unit_limit: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_legacy_transaction: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JupiterSwapResponse {
    pub swap_transaction: String,
    pub last_valid_block_height: Option<u64>,
    pub prioritization_fee_lamports: Option<u64>,
}

// =============================================================================
// JUPITER CLIENT CONFIGURATION
// =============================================================================

#[derive(Debug, Clone)]
pub struct JupiterConfig {
    /// Base URL - defaults to https://api.jup.ag/swap/v1
    pub api_url: String,
    /// API key from portal.jup.ag - load from JUPITER_API_KEY env var
    /// Sent as: x-api-key: <key>  (per dev.jup.ag docs)
    pub api_key: Option<String>,
    pub slippage_bps: u16,
    pub priority_fee_lamports: u64,
    pub priority_level: String,
    pub only_direct_routes: bool,
    pub timeout_secs: u64,
}

impl Default for JupiterConfig {
    fn default() -> Self {
        Self {
            api_url: JUPITER_API_BASE.to_string(),
            api_key: std::env::var("JUPITER_API_KEY").ok(),
            slippage_bps: 50,
            priority_fee_lamports: 10_000,
            priority_level: "high".to_string(),
            only_direct_routes: false,
            timeout_secs: JUPITER_API_TIMEOUT_SECS,
        }
    }
}

// =============================================================================
// JUPITER CLIENT
// =============================================================================

pub struct JupiterClient {
    config: JupiterConfig,
    http_client: Arc<HttpClient>,
    sol_mint: Pubkey,
    usdc_mint: Pubkey,
}

impl JupiterClient {
    pub fn new(config: JupiterConfig) -> Result<Self> {
        let has_key = config.api_key.is_some();
        info!("[Jupiter] V6.0 | endpoint: {} | auth: {} | slippage: {} BPS | priority: {} (max {} lamports)",
            config.api_url,
            if has_key { "API key set" } else { "NO KEY - set JUPITER_API_KEY" },
            config.slippage_bps,
            config.priority_level,
            config.priority_fee_lamports);

        if !has_key {
            warn!("[Jupiter] No API key configured. Get a free key at https://portal.jup.ag and set JUPITER_API_KEY env var.");
        }

        let http_client = Arc::new(
            HttpClient::builder()
                .timeout(Duration::from_secs(config.timeout_secs))
                .build()
                .context("Failed to build HTTP client")?
        );

        let sol_mint  = Pubkey::from_str(SOL_MINT).context("Invalid SOL mint")?;
        let usdc_mint = Pubkey::from_str(USDC_MINT).context("Invalid USDC mint")?;

        Ok(Self { config, http_client, sol_mint, usdc_mint })
    }

    // -- Builders ------------------------------------------------------------

    /// Set the Jupiter API key (from portal.jup.ag).
    /// Prefer loading from JUPITER_API_KEY env var over hardcoding.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.config.api_key = Some(key.into());
        self
    }

    pub fn with_priority_fee(mut self, max_lamports: u64) -> Self {
        self.config.priority_fee_lamports = max_lamports;
        self
    }

    pub fn with_priority_level(mut self, level: &str) -> Self {
        self.config.priority_level = level.to_string();
        self
    }

    /// Override DNS for a specific hostname with a pre-resolved IP.
    /// Use with `resolve_via_doh()` to bypass ISP/router DNS filtering.
    pub fn with_resolved_host(self, host: &str, ip: IpAddr) -> Result<Self> {
        let addr = SocketAddr::new(ip, 443);
        let http_client = Arc::new(
            HttpClient::builder()
                .timeout(Duration::from_secs(self.config.timeout_secs))
                .resolve(host, addr)
                .build()
                .context("Failed to rebuild HTTP client with resolved host")?
        );
        info!("[Jupiter] DNS override: {} -> {} (system DNS bypassed)", host, ip);
        Ok(Self { http_client, ..self })
    }

    // -- Internal: auth header -----------------------------------------------
    // Jupiter API uses x-api-key header (NOT Authorization: Bearer)
    // Reference: https://dev.jup.ag/api-reference/swap/quote

    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(ref key) = self.config.api_key {
            builder.header("x-api-key", key.as_str())
        } else {
            builder
        }
    }

    // -- Quote helpers -------------------------------------------------------

    pub async fn get_quote_sol_to_usdc(&self, amount_lamports: u64) -> Result<JupiterQuoteResponse> {
        self.get_quote(SOL_MINT, USDC_MINT, amount_lamports, self.config.slippage_bps).await
    }

    pub async fn get_quote_usdc_to_sol(&self, amount_usdc_micro: u64) -> Result<JupiterQuoteResponse> {
        self.get_quote(USDC_MINT, SOL_MINT, amount_usdc_micro, self.config.slippage_bps).await
    }

    pub async fn get_quote(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        slippage_bps: u16,
    ) -> Result<JupiterQuoteResponse> {
        let url = format!("{}/quote", self.config.api_url);
        let request = JupiterQuoteRequest {
            input_mint: input_mint.to_string(),
            output_mint: output_mint.to_string(),
            amount,
            slippage_bps,
            only_direct_routes: Some(self.config.only_direct_routes),
            as_legacy_transaction: Some(false),
        };

        debug!("[Jupiter] quote: {} {} -> {}", amount, input_mint, output_mint);

        let response = self.apply_auth(self.http_client.get(&url).query(&request))
            .send()
            .await
            .context("Failed to send quote request to Jupiter")?;

        if !response.status().is_success() {
            let status = response.status();
            let err = response.text().await.unwrap_or_else(|_| "unknown".to_string());
            bail!("Jupiter quote API error {}: {}", status, err);
        }

        let quote: JupiterQuoteResponse = response
            .json()
            .await
            .context("Failed to parse Jupiter quote response")?;

        let in_amt  = quote.in_amount.parse::<u64>().unwrap_or(0);
        let out_amt = quote.out_amount.parse::<u64>().unwrap_or(0);
        let impact  = quote.price_impact_pct.parse::<f64>().unwrap_or(0.0);

        info!("[Jupiter] quote ok: {} -> {} | impact: {:.4}%",
            Self::fmt_amount(in_amt, input_mint),
            Self::fmt_amount(out_amt, output_mint),
            impact);

        if impact > 1.0 {
            warn!("[Jupiter] HIGH PRICE IMPACT: {:.2}%! Consider smaller trade size.", impact);
        }

        Ok(quote)
    }

    // -- Swap Transaction ----------------------------------------------------

    pub async fn get_swap_transaction(
        &self,
        quote: &JupiterQuoteResponse,
        user_pubkey: Pubkey,
    ) -> Result<(VersionedTransaction, u64)> {
        debug!("[Jupiter] building swap tx for {}", user_pubkey);

        let swap_request = JupiterSwapRequest {
            quote_response: quote.clone(),
            user_public_key: user_pubkey.to_string(),
            wrap_and_unwrap_sol: Some(true),
            priority_fee: Some(PriorityFee {
                priority_level_with_max_lamports: PriorityLevelWithMaxLamports {
                    max_lamports: self.config.priority_fee_lamports,
                    priority_level: self.config.priority_level.clone(),
                },
            }),
            dynamic_compute_unit_limit: Some(true),
            as_legacy_transaction: Some(false),
        };

        let response = self.apply_auth(
            self.http_client
                .post(format!("{}/swap", self.config.api_url))
                .json(&swap_request)
        )
            .send()
            .await
            .context("Failed to send swap request to Jupiter")?;

        if !response.status().is_success() {
            let status = response.status();
            let err = response.text().await.unwrap_or_else(|_| "unknown".to_string());
            bail!("Jupiter swap API error {}: {}", status, err);
        }

        let swap_response: JupiterSwapResponse = response
            .json()
            .await
            .context("Failed to parse Jupiter swap response")?;

        let tx_bytes = BASE64_STANDARD
            .decode(&swap_response.swap_transaction)
            .context("Failed to decode base64 swap transaction")?;

        let versioned_tx: VersionedTransaction = bincode::deserialize(&tx_bytes)
            .context("Failed to deserialize VersionedTransaction")?;

        let last_valid = swap_response.last_valid_block_height.unwrap_or(0);
        info!("[Jupiter] swap tx ready (valid until block {})", last_valid);

        Ok((versioned_tx, last_valid))
    }

    pub async fn prepare_swap(
        &self,
        input_mint: &str,
        output_mint: &str,
        amount: u64,
        user_pubkey: Pubkey,
    ) -> Result<(VersionedTransaction, u64, JupiterQuoteResponse)> {
        info!("[Jupiter] prepare_swap: {} {} -> {}", amount, input_mint, output_mint);
        let quote = self.get_quote(input_mint, output_mint, amount, self.config.slippage_bps).await?;
        let (tx, last_valid) = self.get_swap_transaction(&quote, user_pubkey).await?;
        Ok((tx, last_valid, quote))
    }

    // -- Utilities -----------------------------------------------------------

    fn fmt_amount(amount: u64, mint: &str) -> String {
        match mint {
            SOL_MINT  => format!("{:.4} SOL",  amount as f64 / 1e9),
            USDC_MINT => format!("${:.2} USDC", amount as f64 / 1e6),
            _         => format!("{} tokens",  amount),
        }
    }

    pub fn parse_output_amount(quote: &JupiterQuoteResponse) -> Result<u64> {
        quote.out_amount.parse::<u64>().context("Failed to parse output amount")
    }

    pub fn parse_price_impact(quote: &JupiterQuoteResponse) -> Result<f64> {
        quote.price_impact_pct.parse::<f64>().context("Failed to parse price impact")
    }

    pub fn is_price_impact_acceptable(quote: &JupiterQuoteResponse, max_pct: f64) -> bool {
        Self::parse_price_impact(quote).map(|i| i <= max_pct).unwrap_or(false)
    }

    pub fn sol_mint(&self)  -> &Pubkey { &self.sol_mint }
    pub fn usdc_mint(&self) -> &Pubkey { &self.usdc_mint }
}

// =============================================================================
// DNS-over-HTTPS FALLBACK
// =============================================================================

/// Resolve a hostname via DNS-over-HTTPS.
/// Tries Cloudflare (1.1.1.1) then Google (8.8.8.8). Follows CNAME chains.
pub async fn resolve_via_doh(hostname: &str) -> Result<IpAddr> {
    let providers = ["1.1.1.1", "8.8.8.8"];
    for server in &providers {
        match resolve_via_doh_provider(hostname, server).await {
            Ok(ip) => {
                info!("[DoH] {} -> {} (via {})", hostname, ip, server);
                return Ok(ip);
            }
            Err(e) => debug!("[DoH] {} failed for {}: {}", server, hostname, e),
        }
    }
    bail!("All DoH providers (1.1.1.1, 8.8.8.8) failed to resolve '{}'", hostname)
}

async fn resolve_via_doh_provider(hostname: &str, server: &str) -> Result<IpAddr> {
    const TYPE_A: u16     = 1;
    const TYPE_CNAME: u16 = 5;
    const MAX_HOPS: usize = 5;

    #[derive(Deserialize)]
    struct DohAnswer {
        #[serde(rename = "type")]
        record_type: u16,
        data: String,
    }
    #[derive(Deserialize)]
    struct DohResponse {
        #[serde(rename = "Answer")]
        answer: Option<Vec<DohAnswer>>,
    }

    let client = HttpClient::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .context("Failed to build DoH client")?;

    let mut target = hostname.to_string();
    for hop in 0..MAX_HOPS {
        let url = format!("https://{}/dns-query?name={}&type=A", server, target);
        let doh: DohResponse = client
            .get(&url)
            .header("Accept", "application/dns-json")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("DoH request to {} failed: {}", server, e))?
            .json()
            .await
            .with_context(|| format!("Failed to parse DoH response from {}", server))?;

        let answers = doh.answer.unwrap_or_default();

        if let Some(a) = answers.iter().find(|r| r.record_type == TYPE_A) {
            return a.data.trim_end_matches('.')
                .parse::<IpAddr>()
                .with_context(|| format!("Invalid IP in DoH response: '{}'", a.data));
        }

        if let Some(cname) = answers.iter().find(|r| r.record_type == TYPE_CNAME) {
            let next = cname.data.trim_end_matches('.').to_string();
            debug!("[DoH] CNAME hop {}: {} -> {}", hop + 1, target, next);
            target = next;
            continue;
        }

        bail!("No A or CNAME records for '{}' from {}", target, server);
    }
    bail!("Too many CNAME hops resolving '{}' via {}", hostname, server)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default_url() {
        let c = JupiterConfig::default();
        assert!(c.api_url.contains("api.jup.ag"), "Must use new api.jup.ag endpoint");
        assert!(!c.api_url.contains("quote-api"), "Old quote-api endpoint is deprecated");
    }

    #[test]
    fn test_config_default() {
        let c = JupiterConfig::default();
        assert_eq!(c.slippage_bps, 50);
        assert_eq!(c.priority_level, "high");
        assert!(c.priority_fee_lamports > 0);
    }

    #[test]
    fn test_wsol_alias() {
        assert_eq!(SOL_MINT, WSOL_MINT);
    }

    #[test]
    fn test_with_api_key() {
        let client = JupiterClient::new(JupiterConfig::default())
            .unwrap()
            .with_api_key("test_key_123");
        assert_eq!(client.config.api_key.as_deref(), Some("test_key_123"));
    }

    #[test]
    fn test_builder_priority_fee() {
        let client = JupiterClient::new(JupiterConfig::default())
            .unwrap()
            .with_priority_fee(50_000)
            .with_priority_level("veryHigh");
        assert_eq!(client.config.priority_fee_lamports, 50_000);
        assert_eq!(client.config.priority_level, "veryHigh");
    }

    #[test]
    fn test_format_amounts() {
        assert!(JupiterClient::fmt_amount(1_000_000_000, SOL_MINT).contains("1.0000 SOL"));
        assert!(JupiterClient::fmt_amount(1_000_000, USDC_MINT).contains("$1.00 USDC"));
        assert!(JupiterClient::fmt_amount(100, "unknown").contains("tokens"));
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_quote_live() {
        // Set JUPITER_API_KEY before running
        let client = JupiterClient::new(JupiterConfig::default()).unwrap();
        match client.get_quote_sol_to_usdc(100_000_000).await {
            Ok(q)  => println!("Quote: {} -> {}", q.in_amount, q.out_amount),
            Err(e) => println!("Skipping (network/key unavailable): {}", e),
        }
    }
}
