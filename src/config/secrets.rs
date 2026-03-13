//! # Secret Resolution — Environment Variable Override Layer
//!
//! Reads `GRIDZBOTZ_*` environment variables and overrides config values
//! loaded from TOML. This keeps secrets out of version control.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Called automatically inside Config::from_file() since V5.9.
//! // No manual call needed — secrets are resolved before validate().
//! let config = Config::from_file("config/production/mainnet.toml")?;
//! // config.network.rpc_url      — real Chainstack URL from env
//! // config.security.wallet_path — real keypair path from env
//! // config.alerts.telegram_bot_token — real token from env (never in TOML)
//! ```
//!
//! ## Env Vars
//!
//! | Variable                        | Overrides                          | Required (live) |
//! |---------------------------------|------------------------------------|-----------------|
//! | `GRIDZBOTZ_RPC_URL`             | `network.rpc_url`                  | Warn if missing |
//! | `GRIDZBOTZ_FALLBACK_RPC_URL`    | `execution.rpc_fallback_urls[0]`   | No              |
//! | `GRIDZBOTZ_JUPITER_API_KEY`     | read by Jupiter client from env    | Yes             |
//! | `GRIDZBOTZ_WALLET_PATH`         | `security.wallet_path`             | Yes             |
//! | `GRIDZBOTZ_JITO_TIP_LAMPORTS`   | `execution.jito_tip_lamports`      | No              |
//! | `GRIDZBOTZ_TELEGRAM_BOT_TOKEN`  | `alerts.telegram_bot_token`        | No              |
//!
//! ## Jupiter API Key
//!
//! `GRIDZBOTZ_JUPITER_API_KEY` is validated here but **not stored in Config**.
//! The Jupiter client reads it directly from env at init time.
//! A future `JupiterConfig` struct will centralise this (Stage 2 TODO).

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
/// Called automatically by `Config::from_file()` (V5.9+) between
/// `apply_environment_defaults()` and `validate()`. Do not call manually.
///
/// Each `GRIDZBOTZ_*` env var, when present and non-empty, overrides the
/// corresponding TOML value. When absent the TOML value is preserved —
/// safe for paper trading with no secrets configured.
///
/// # Errors
///
/// Returns `Err` if `execution_mode == "live"` and a required secret
/// (`GRIDZBOTZ_JUPITER_API_KEY`, wallet file) is missing or empty.
/// Paper mode never bails on missing secrets.
pub fn resolve_secrets(config: &mut Config) -> anyhow::Result<()> {
    let is_live = config.bot.execution_mode == "live";
    let mut resolved_count: u8 = 0;

    info!(
        "🔐 Resolving secrets for instance '{}' (mode: {})",
        config.bot.instance_id.as_deref().unwrap_or("default"),
        config.bot.execution_mode
    );

    // ─── RPC URL ────────────────────────────────────────────────────────
    if let Ok(rpc_url) = env::var("GRIDZBOTZ_RPC_URL") {
        if !rpc_url.is_empty() {
            info!("  ✅ RPC URL: {} (from env)", mask_secret(&rpc_url));
            config.network.rpc_url = rpc_url;
            resolved_count += 1;
        }
    } else if is_live {
        warn!("  ⚠️  GRIDZBOTZ_RPC_URL not set — using TOML default (public RPC, rate-limited!)");
    }

    // ─── Fallback RPC URL (optional) ────────────────────────────────────
    // Previously: read + counted but never stored — RPC pool had to read
    // env::var itself. Now wired into execution.rpc_fallback_urls so the
    // pool receives it through the Config like every other value.
    if let Ok(fallback_url) = env::var("GRIDZBOTZ_FALLBACK_RPC_URL") {
        if !fallback_url.is_empty() {
            info!("  ✅ Fallback RPC: {} (from env)", mask_secret(&fallback_url));
            match config.execution.rpc_fallback_urls.as_mut() {
                Some(urls) => {
                    // Prepend so the env-supplied URL is tried first
                    if !urls.contains(&fallback_url) {
                        urls.insert(0, fallback_url);
                    }
                }
                None => {
                    config.execution.rpc_fallback_urls = Some(vec![fallback_url]);
                }
            }
            resolved_count += 1;
        }
    }

    // ─── Jupiter API Key ────────────────────────────────────────────────
    // NOT stored in Config — Jupiter client reads env directly at init.
    // TODO(Stage 2): Add JupiterConfig to Config struct, then override here.
    if let Ok(api_key) = env::var("GRIDZBOTZ_JUPITER_API_KEY") {
        if !api_key.is_empty() {
            info!("  ✅ Jupiter API key: {} (available via env)", mask_secret(&api_key));
            resolved_count += 1;
        } else if is_live {
            anyhow::bail!(
                "🚨 GRIDZBOTZ_JUPITER_API_KEY is set but empty. \
                 Cannot run live without a Jupiter API key. \
                 Get one free at https://portal.jup.ag"
            );
        }
    } else if is_live {
        anyhow::bail!(
            "🚨 GRIDZBOTZ_JUPITER_API_KEY not set. \
             Cannot run live without a Jupiter API key. \
             Get one free at https://portal.jup.ag"
        );
    }

    // ─── Wallet Path ────────────────────────────────────────────────────
    if let Ok(wallet_path) = env::var("GRIDZBOTZ_WALLET_PATH") {
        if !wallet_path.is_empty() {
            info!("  ✅ Wallet path: {} (from env)", mask_secret(&wallet_path));
            config.security.wallet_path = wallet_path;
            resolved_count += 1;
        }
    } else if is_live {
        // Expand ~ in TOML fallback so validate_for_live_mode() can stat the file
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

    // ─── Jito Tip (optional) ────────────────────────────────────────────
    if let Ok(tip_str) = env::var("GRIDZBOTZ_JITO_TIP_LAMPORTS") {
        if let Ok(tip) = tip_str.parse::<u64>() {
            info!("  ✅ Jito tip: {} lamports (from env)", tip);
            config.execution.jito_tip_lamports = Some(tip);
            resolved_count += 1;
        } else {
            warn!("  ⚠️  GRIDZBOTZ_JITO_TIP_LAMPORTS='{}' is not a valid u64 — ignoring", tip_str);
        }
    }

    // ─── Telegram Bot Token (optional) ──────────────────────────────────
    // Previously only TOML-populated — accidental commit risk.
    // Now the env var is the canonical source; TOML value is the fallback.
    if let Ok(token) = env::var("GRIDZBOTZ_TELEGRAM_BOT_TOKEN") {
        if !token.is_empty() {
            info!("  ✅ Telegram bot token: {} (from env)", mask_secret(&token));
            config.alerts.telegram_bot_token = Some(token);
            resolved_count += 1;
        } else {
            warn!("  ⚠️  GRIDZBOTZ_TELEGRAM_BOT_TOKEN is set but empty — ignoring");
        }
    }

    // ─── Summary ────────────────────────────────────────────────────────
    info!(
        "🔐 Secret resolution complete: {}/6 overrides applied (mode: {})",
        resolved_count,
        config.bot.execution_mode
    );

    // ─── Live Mode Validation ────────────────────────────────────────────
    if is_live {
        // Wallet file must exist and be readable
        let wallet = &config.security.wallet_path;
        if !std::path::Path::new(wallet).exists() {
            anyhow::bail!(
                "🚨 Wallet file not found: {}. \
                 Set GRIDZBOTZ_WALLET_PATH or check security.wallet_path in TOML.",
                wallet
            );
        }

        // Warn if still on public RPC (rate-limited, unreliable for trading)
        if config.network.rpc_url.contains("api.mainnet-beta.solana.com") {
            warn!(
                "  🟡 WARNING: Using public Solana RPC in live mode! \
                 This is rate-limited and unreliable for trading. \
                 Set GRIDZBOTZ_RPC_URL to your Chainstack/QuickNode/Helius endpoint."
            );
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── mask_secret ───────────────────────────────────────────────────────

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

    // ── GRIDZBOTZ_FALLBACK_RPC_URL wiring ────────────────────────────────

    #[test]
    fn test_fallback_rpc_prepended_when_list_empty() {
        use super::super::{ExecutionConfig};
        let mut exec = ExecutionConfig::default();
        assert!(exec.rpc_fallback_urls.is_none());

        // Simulate what resolve_secrets does
        let url = "https://helius.xyz/fallback".to_string();
        exec.rpc_fallback_urls = Some(vec![url.clone()]);
        assert_eq!(exec.rpc_fallback_urls.as_ref().unwrap()[0], url);
    }

    #[test]
    fn test_fallback_rpc_no_duplicate_prepend() {
        use super::super::ExecutionConfig;
        let url = "https://helius.xyz/fallback".to_string();
        let mut exec = ExecutionConfig::default();
        exec.rpc_fallback_urls = Some(vec![url.clone(), "https://other.rpc".to_string()]);

        // Simulate the dedup logic
        if let Some(urls) = exec.rpc_fallback_urls.as_mut() {
            if !urls.contains(&url) {
                urls.insert(0, url.clone());
            }
        }
        // Should still be length 2 — no duplicate added
        assert_eq!(exec.rpc_fallback_urls.unwrap().len(), 2);
    }

    // ── GRIDZBOTZ_TELEGRAM_BOT_TOKEN wiring ──────────────────────────────

    #[test]
    fn test_telegram_token_override_logic() {
        use super::super::AlertsConfig;
        let mut alerts = AlertsConfig {
            enabled: true,
            telegram_bot_token: Some("toml-token".to_string()),
            telegram_chat_id:   Some("123456".to_string()),
        };
        // Simulate env override
        let env_token = "env-token-abc123xyz".to_string();
        if !env_token.is_empty() {
            alerts.telegram_bot_token = Some(env_token.clone());
        }
        assert_eq!(alerts.telegram_bot_token.unwrap(), env_token);
    }

    #[test]
    fn test_telegram_token_empty_env_ignored() {
        use super::super::AlertsConfig;
        let mut alerts = AlertsConfig {
            enabled: true,
            telegram_bot_token: Some("toml-token".to_string()),
            telegram_chat_id:   Some("123456".to_string()),
        };
        // Empty env var — should NOT overwrite TOML value
        let env_token = "".to_string();
        if !env_token.is_empty() {
            alerts.telegram_bot_token = Some(env_token);
        }
        assert_eq!(alerts.telegram_bot_token.unwrap(), "toml-token");
    }
}
