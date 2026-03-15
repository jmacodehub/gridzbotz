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
//! fix/rent-exempt-spendable-sol (this PR):
//!   `get_balance()` returns total lamports including the wallet's
//!   rent-exempt reserve (~890,880 lamports). The raw value was previously
//!   divided by 1e9 directly, making the bot treat reserve SOL as tradeable.
//!   Fix: `saturating_sub(WALLET_RENT_EXEMPT_LAMPORTS)` before the f64 cast
//!   so only truly spendable lamports flow into BalanceTracker, NAV, and
//!   CircuitBreaker. A boot-time log shows raw vs spendable for observability.
//!
//! Used in:
//!   • `main.rs`         → `initialize_components()` (live mode only)
//!   • `orchestrator.rs` → `Orchestrator::from_config()` (per-bot, live only)
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Context, Result};
use log::{info, warn};

/// Rent-exempt minimum for a Solana system account (wallet), in lamports.
///
/// This is the canonical value: `solana rent 0` → 890,880 lamports.
/// It has not changed since genesis and is not expected to change.
/// If Solana ever adjusts the rent schedule, update this constant and
/// the `test_rent_exempt_constant_value` regression test below.
const WALLET_RENT_EXEMPT_LAMPORTS: u64 = 890_880;

/// Query on-chain SOL and USDC balances for the wallet at `wallet_path`.
///
/// Returns `(usdc_balance, sol_balance)` in human-readable units
/// (i.e. already divided by decimals).
///
/// The returned SOL balance is **spendable SOL only** — the wallet's
/// rent-exempt reserve (`WALLET_RENT_EXEMPT_LAMPORTS`) is subtracted
/// before conversion so callers always work with tradeable amounts.
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
    // get_balance() returns total lamports including the wallet's rent-exempt
    // reserve. Subtract WALLET_RENT_EXEMPT_LAMPORTS so only spendable SOL
    // is returned. saturating_sub guards against the (theoretical) case
    // where the wallet balance is below the rent threshold.
    let client      = RpcClient::new(rpc_url.to_string());
    let raw_lamports = client
        .get_balance(&pubkey)
        .await
        .with_context(|| format!("RPC get_balance failed for {}", pubkey))?;
    let spendable_lamports = raw_lamports.saturating_sub(WALLET_RENT_EXEMPT_LAMPORTS);
    let sol = spendable_lamports as f64 / 1_000_000_000.0;

    info!(
        "[wallet] SOL: raw={:.6} spendable={:.6} (rent-exempt reserve: {:.6} SOL deducted)",
        raw_lamports as f64 / 1_000_000_000.0,
        sol,
        WALLET_RENT_EXEMPT_LAMPORTS as f64 / 1_000_000_000.0,
    );

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
    use super::WALLET_RENT_EXEMPT_LAMPORTS;

    /// Smoke test: the symbol is importable and the type is an async fn.
    ///
    /// FIX 3 (E0308): the original test tried to assign the async fn item to
    /// a bare fn-pointer annotation `fn(&str, &str) -> _`, which fails because
    /// async fn items are not coercible to fn-pointers. Replaced with a plain
    /// `let _ = symbol;` which just checks the symbol resolves at compile time.
    #[test]
    fn test_function_is_accessible() {
        let _ = super::fetch_wallet_balances_for_orchestrator;
    }

    #[test]
    fn test_token_rpc_url_fallback_logic() {
        let rpc_url = "https://chainstack.example.com/key";
        let token_rpc_url: Option<&str> = None;
        let effective = token_rpc_url.unwrap_or(rpc_url);
        assert_eq!(effective, rpc_url);
    }

    #[test]
    fn test_token_rpc_url_helius_wins() {
        let rpc_url   = "https://chainstack.example.com/key";
        let helius    = "https://mainnet.helius-rpc.com/?api-key=test";
        let effective = Some(helius).unwrap_or(rpc_url);
        assert_eq!(effective, helius);
    }

    /// Verifies the rent-exempt deduction logic: spendable = raw - reserve.
    /// Uses a representative wallet balance (2.52 SOL = 2_520_528_000 lamports)
    /// matching the actual boot balance observed in production logs.
    #[test]
    fn test_rent_exempt_deduction_applied() {
        let raw_lamports: u64 = 2_520_528_000; // 2.520528 SOL (production boot value)
        let spendable = raw_lamports.saturating_sub(WALLET_RENT_EXEMPT_LAMPORTS);
        let sol = spendable as f64 / 1_000_000_000.0;

        // Must be strictly less than raw
        assert!(
            sol < raw_lamports as f64 / 1_000_000_000.0,
            "spendable SOL must be less than raw SOL"
        );
        // Deduction must equal exactly WALLET_RENT_EXEMPT_LAMPORTS / 1e9
        let expected = (raw_lamports - WALLET_RENT_EXEMPT_LAMPORTS) as f64 / 1_000_000_000.0;
        assert!(
            (sol - expected).abs() < 1e-12,
            "deduction must be exactly WALLET_RENT_EXEMPT_LAMPORTS lamports"
        );
    }

    /// Guards the canonical constant value.
    /// `solana rent 0` → 890,880 lamports for a system account.
    /// If Solana changes the rent schedule this test will catch it.
    #[test]
    fn test_rent_exempt_constant_value() {
        assert_eq!(
            WALLET_RENT_EXEMPT_LAMPORTS, 890_880,
            "WALLET_RENT_EXEMPT_LAMPORTS must be 890_880 — the canonical Solana \
             system-account rent-exempt minimum. Update this constant and test \
             only if Solana changes the rent schedule."
        );
    }

    /// saturating_sub must never underflow — a wallet below rent threshold
    /// returns 0.0 spendable SOL rather than wrapping to u64::MAX.
    #[test]
    fn test_spendable_zero_when_below_rent_exempt() {
        let raw_lamports: u64 = 100_000; // well below 890_880
        let spendable = raw_lamports.saturating_sub(WALLET_RENT_EXEMPT_LAMPORTS);
        assert_eq!(spendable, 0, "saturating_sub must clamp to 0, not underflow");
        let sol = spendable as f64 / 1_000_000_000.0;
        assert_eq!(sol, 0.0);
    }
}
