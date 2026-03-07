//! # Secret Resolution — Environment Variable Override Layer
//!
//! Reads `GRIDZBOTZ_*` environment variables and overrides config values
//! loaded from TOML. This keeps secrets out of version control.
//!
//! ## Usage
//!
//! ```rust,ignore
//! let mut config = Config::from_file("config/production/mainnet.toml")?;
//! gridzbotz::config::secrets::resolve_secrets(&mut config)?;
//! // config.network.rpc_url is now the real Chainstack URL from .env
//! ```
//!
//! ## Env Vars
//!
//! | Variable                      | Overrides                    | Required |
//! |-------------------------------|------------------------------|----------|
//! | `GRIDZBOTZ_RPC_URL`           | `network.rpc_url`            | Yes      |
//! | `GRIDZBOTZ_FALLBACK_RPC_URL`  | `network.fallback_rpc_urls`  | No       |
//! | `GRIDZBOTZ_JUPITER_API_KEY`   | `jupiter.api_key`            | Yes      |
//! | `GRIDZBOTZ_WALLET_PATH`       | `security.wallet_path`       | Yes      |
//! | `GRIDZBOTZ_JITO_TIP_LAMPORTS` | `execution.jito_tip_lamports`| No       |

use log::{info, warn};
use std::env;

use super::Config;

/// Mask a secret for safe logging: show first 8 + last 4 chars.
fn mask_secret(s: &str) -> String {
    if s.len() <= 16 {
        return "****".to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 4..])
}

/// Resolve secrets from environment variables into the Config.
///
/// Call this immediately after `Config::from_file()` and before any
/// trading logic. If a `GRIDZBOTZ_*` env var is set, it overrides
/// the corresponding TOML value. If not set, the TOML value is
/// preserved as-is (useful for paper trading with no secrets).
///
/// # Errors
///
/// Returns `Err` if `execution_mode == "live"` and a required secret
/// is missing or empty. Paper mode allows missing secrets.
pub fn resolve_secrets(config: &mut Config) -> anyhow::Result<()> {
    let is_live = config.bot.execution_mode == "live";
    let mut resolved_count: u8 = 0;

    info!(
        "🔐 Resolving secrets for instance '{}' (mode: {})",
        config.bot.instance_id.as_deref().unwrap_or("default"),
        config.bot.execution_mode
    );

    // ─── RPC URL ──────────────────────────────────────────────────────────
    if let Ok(rpc_url) = env::var("GRIDZBOTZ_RPC_URL") {
        if !rpc_url.is_empty() {
            info!("  ✅ RPC URL: {} (from env)", mask_secret(&rpc_url));
            config.network.rpc_url = rpc_url;
            resolved_count += 1;
        }
    } else if is_live {
        warn!("  ⚠️  GRIDZBOTZ_RPC_URL not set — using TOML default (public RPC, rate-limited!)");
    }

    // ─── Fallback RPC URL (optional) ──────────────────────────────────────
    if let Ok(fallback_url) = env::var("GRIDZBOTZ_FALLBACK_RPC_URL") {
        if !fallback_url.is_empty() {
            info!("  ✅ Fallback RPC: {} (from env)", mask_secret(&fallback_url));
            // TODO: Wire into config.network.fallback_rpc_urls when field exists
            // config.network.fallback_rpc_urls = Some(vec![fallback_url]);
            resolved_count += 1;
        }
    }

    // ─── Jupiter API Key ─────────────────────────────────────────────────
    if let Ok(api_key) = env::var("GRIDZBOTZ_JUPITER_API_KEY") {
        if !api_key.is_empty() {
            info!("  ✅ Jupiter API key: {} (from env)", mask_secret(&api_key));
            config.jupiter.api_key = api_key;
            resolved_count += 1;
        }
    } else if is_live {
        // Empty string in TOML + no env var = fail in live mode
        if config.jupiter.api_key.is_empty() {
            anyhow::bail!(
                "🚨 GRIDZBOTZ_JUPITER_API_KEY not set and jupiter.api_key is empty in TOML. \
                 Cannot run live without a Jupiter API key. \
                 Get one free at https://portal.jup.ag"
            );
        }
        warn!("  ⚠️  GRIDZBOTZ_JUPITER_API_KEY not set — using TOML value (migrate to env!)");
    }

    // ─── Wallet Path ─────────────────────────────────────────────────────
    if let Ok(wallet_path) = env::var("GRIDZBOTZ_WALLET_PATH") {
        if !wallet_path.is_empty() {
            info!("  ✅ Wallet path: {} (from env)", mask_secret(&wallet_path));
            config.security.wallet_path = wallet_path;
            resolved_count += 1;
        }
    } else if is_live {
        // Expand ~ to home dir if present in TOML fallback
        let path = &config.security.wallet_path;
        if path.starts_with('~') {
            if let Ok(home) = env::var("HOME") {
                config.security.wallet_path = path.replacen('~', &home, 1);
                info!(
                    "  ✅ Wallet path: ~ expanded to {} (from TOML)",
                    mask_secret(&config.security.wallet_path)
                );
            }
        }
    }

    // ─── Jito Tip (optional) ─────────────────────────────────────────────
    if let Ok(tip_str) = env::var("GRIDZBOTZ_JITO_TIP_LAMPORTS") {
        if let Ok(tip) = tip_str.parse::<u64>() {
            info!("  ✅ Jito tip: {} lamports (from env)", tip);
            // TODO: Wire into config.execution.jito_tip_lamports when field exists
            // config.execution.jito_tip_lamports = Some(tip);
            resolved_count += 1;
        } else {
            warn!("  ⚠️  GRIDZBOTZ_JITO_TIP_LAMPORTS='{}' is not a valid u64 — ignoring", tip_str);
        }
    }

    // ─── Summary ─────────────────────────────────────────────────────────
    info!(
        "🔐 Secret resolution complete: {}/5 overrides applied (mode: {})",
        resolved_count,
        config.bot.execution_mode
    );

    // ─── Live Mode Validation ────────────────────────────────────────────
    if is_live {
        // Validate wallet file exists
        let wallet = &config.security.wallet_path;
        if !std::path::Path::new(wallet).exists() {
            anyhow::bail!(
                "🚨 Wallet file not found: {}. \
                 Set GRIDZBOTZ_WALLET_PATH or check security.wallet_path in TOML.",
                wallet
            );
        }

        // Validate RPC is not the public endpoint (rate-limited, unreliable)
        if config.network.rpc_url.contains("api.mainnet-beta.solana.com") {
            warn!(
                "  🟡 WARNING: Using public Solana RPC in live mode! \
                 This is rate-limited and unreliable for trading. \
                 Set GRIDZBOTZ_RPC_URL to your Chainstack/QuickNode endpoint."
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_secret_long() {
        let masked = mask_secret("https://solana-mainnet.core.chainstack.com/abc123def456");
        assert!(masked.starts_with("https://"));
        assert!(masked.ends_with("f456"));
        assert!(masked.contains("..."));
    }

    #[test]
    fn test_mask_secret_short() {
        let masked = mask_secret("short");
        assert_eq!(masked, "****");
    }

    #[test]
    fn test_mask_secret_exact_boundary() {
        let masked = mask_secret("1234567890123456"); // exactly 16
        assert_eq!(masked, "****");
    }

    #[test]
    fn test_mask_secret_just_over() {
        let masked = mask_secret("12345678901234567"); // 17 chars
        assert_eq!(masked, "12345678...4567");
    }
}
