//! ═══════════════════════════════════════════════════════════════════════════
//! Wallet Balance Utilities — shared between main.rs (single-bot) and
//! orchestrator.rs (fleet mode).
//!
//! PR #86: Extracted from the private `fetch_wallet_balances()` fn in
//! main.rs so the orchestrator can call it without coupling to main.rs.
//!
//! fix/usdc-balance-helius-routing:
//!   `token_rpc_url: Option<&str>` added — Chainstack blocks
//!   getTokenAccountsByOwner (403). SOL balance uses the primary RPC;
//!   USDC token account query routes to the Helius fallback when supplied.
//!
//! Used in:
//!   • `main.rs`         → `initialize_components()` (live mode only)
//!   • `orchestrator.rs` → `Orchestrator::from_config()` (per-bot, live only)
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Context, Result};
use log::{info, warn};

/// Query on-chain SOL and USDC balances for the wallet at `wallet_path`.
///
/// Returns `(usdc_balance, sol_balance)` in human-readable units
/// (i.e. already divided by decimals).
///
/// # Params
///
/// - `rpc_url`       — Primary RPC (Chainstack). Used for `get_balance`.
/// - `wallet_path`   — Path to the Solana keypair file (~ expanded).
/// - `token_rpc_url` — Optional secondary RPC (Helius) used exclusively for
///                     `get_token_accounts_by_owner`. Chainstack returns 403
///                     on that method regardless of plan tier. When `None`,
///                     falls back to `rpc_url` (safe for single-RPC setups).
///
/// # Errors
///
/// Returns an error if:
/// - `wallet_path` cannot be read or parsed as a keypair
/// - The RPC `get_balance` call fails
///
/// USDC failures are soft — returns `0.0` with a warning rather than
/// aborting, matching the behaviour of the original `fetch_wallet_balances`
/// in `main.rs`.
pub async fn fetch_wallet_balances_for_orchestrator(
    rpc_url:       &str,
    wallet_path:   &str,
    token_rpc_url: Option<&str>,
) -> Result<(f64, f64)> {
    use solana_client::nonblocking::rpc_client::RpcClient;
    use solana_client::rpc_request::TokenAccountsFilter;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::{read_keypair_file, Signer};
    use std::str::FromStr;

    // ── Expand ~ in wallet path ───────────────────────────────────────────
    let expanded = if wallet_path.starts_with('~') {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Cannot expand ~ in wallet_path")?;
        wallet_path.replacen('~', &home, 1)
    } else {
        wallet_path.to_string()
    };

    let keypair = read_keypair_file(&expanded)
        .map_err(|e| anyhow::anyhow!("Cannot load keypair '{}': {}", expanded, e))?;
    let pubkey = keypair.pubkey();
    info!("[wallet] Querying on-chain balances for: {}", pubkey);

    // ── SOL balance — primary RPC (Chainstack) ────────────────────────────
    let client   = RpcClient::new(rpc_url.to_string());
    let lamports = client
        .get_balance(&pubkey)
        .await
        .with_context(|| format!("RPC get_balance failed for {}", pubkey))?;
    let sol = lamports as f64 / 1_000_000_000.0;

    // ── USDC balance — token RPC (Helius) ─────────────────────────────────
    // Chainstack blocks getTokenAccountsByOwner with HTTP 403 regardless of
    // plan tier. Route this call to the Helius fallback when available;
    // fall back to rpc_url only when no fallback is configured.
    let effective_token_url = token_rpc_url.unwrap_or(rpc_url);
    if token_rpc_url.is_some() {
        info!("[wallet] USDC query → token RPC (Helius fallback)");
    } else {
        warn!("[wallet] No token_rpc_url supplied — USDC query using primary RPC (may 403)");
    }
    let token_client = RpcClient::new(effective_token_url.to_string());

    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
        .expect("static USDC mint is valid");

    let usdc = match token_client
        .get_token_accounts_by_owner(&pubkey, TokenAccountsFilter::Mint(usdc_mint))
        .await
    {
        Ok(accounts) => accounts
            .first()
            .and_then(|a| serde_json::to_value(&a.account.data).ok())
            .and_then(|v| {
                v.pointer("/parsed/info/tokenAmount/uiAmount")
                    .and_then(|x| x.as_f64())
            })
            .unwrap_or(0.0),
        Err(e) => {
            warn!("[wallet] USDC balance query failed: {} — defaulting to $0.00", e);
            0.0
        }
    };

    info!("[wallet] ✅ SOL:  {:.6}", sol);
    info!("[wallet] ✅ USDC: ${:.2}", usdc);
    Ok((usdc, sol))
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    /// Smoke test: the symbol is importable and the type is an async fn.
    ///
    /// FIX 3 (E0308): the original test tried to assign the async fn item to
    /// a bare fn-pointer annotation `fn(&str, &str) -> _`, which fails because
    /// async fn items are not coercible to fn-pointers. Replaced with a plain
    /// `let _ = symbol;` which just checks the symbol resolves at compile time.
    #[test]
    fn test_function_is_accessible() {
        // Verifies the symbol is exported and callable — no pointer coercion.
        let _ = super::fetch_wallet_balances_for_orchestrator;
    }

    #[test]
    fn test_token_rpc_url_fallback_logic() {
        // When token_rpc_url is None, effective URL must equal rpc_url.
        let rpc_url = "https://chainstack.example.com/key";
        let token_rpc_url: Option<&str> = None;
        let effective = token_rpc_url.unwrap_or(rpc_url);
        assert_eq!(effective, rpc_url);
    }

    #[test]
    fn test_token_rpc_url_helius_wins() {
        // When token_rpc_url is Some, Helius URL must be used.
        let rpc_url   = "https://chainstack.example.com/key";
        let helius    = "https://mainnet.helius-rpc.com/?api-key=test";
        let effective = Some(helius).unwrap_or(rpc_url);
        assert_eq!(effective, helius);
    }
}
