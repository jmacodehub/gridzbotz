//! ═══════════════════════════════════════════════════════════════════════════
//! Wallet Balance Utilities — shared between main.rs (single-bot) and
//! orchestrator.rs (fleet mode).
//!
//! PR #86: Extracted from the private `fetch_wallet_balances()` fn in
//! main.rs so the orchestrator can call it without coupling to main.rs.
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
    rpc_url:     &str,
    wallet_path: &str,
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

    let client   = RpcClient::new(rpc_url.to_string());
    let lamports = client
        .get_balance(&pubkey)
        .await
        .with_context(|| format!("RPC get_balance failed for {}", pubkey))?;
    let sol = lamports as f64 / 1_000_000_000.0;

    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
        .expect("static USDC mint is valid");

    let usdc = match client
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
    /// Smoke test: function is importable and the type signature compiles.
    /// Actual RPC calls require a live cluster and are integration-tested only.
    #[test]
    fn test_function_is_accessible() {
        // If this compiles, the symbol is correctly exported.
        let _f: fn(&str, &str) -> _ = super::fetch_wallet_balances_for_orchestrator;
    }
}
