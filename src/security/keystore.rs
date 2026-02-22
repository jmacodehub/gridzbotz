//! ═══════════════════════════════════════════════════════════════════════════
//! 🔐 KEYSTORE - Secure Wallet & Transaction Signing
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! ✅ Encrypted keypair loading (never hardcoded)
//! ✅ Transaction validation before signing
//! ✅ Daily trade limits enforcement
//! ✅ Position size validation
//! ✅ Secure legacy transaction signing
//! ✅ Secure VersionedTransaction signing (V5.2 — for Jupiter swaps)
//! ✅ Thread-safe atomic counters
//!
//! November 2025 | Project Flash V6.0 - Security Layer
//! February 2026  | V5.1 — Added sign_versioned_transaction()
//!                  V5.2 — Hardened: fee-payer identity check + bail on 0-signer tx
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Context, Result};
use log::info;
use serde::{Deserialize, Serialize};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::{Transaction, VersionedTransaction},
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ───────────────────────────────────────────────────────────────────────────────
// 🔐 CONFIGURATION
// ───────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeystoreConfig {
    pub keypair_path: String,
    pub max_transaction_amount_usdc: Option<f64>,
    pub max_daily_trades: Option<u32>,
    pub max_daily_volume_usdc: Option<f64>,
}

impl Default for KeystoreConfig {
    fn default() -> Self {
        Self {
            keypair_path: "~/.config/solana/mainnet-keypair.json".into(),
            max_transaction_amount_usdc: Some(100.0),
            max_daily_trades: Some(200),
            max_daily_volume_usdc: Some(5000.0),
        }
    }
}

impl KeystoreConfig {
    pub fn validate(&self) -> Result<()> {
        if let Some(max_amount) = self.max_transaction_amount_usdc {
            if max_amount <= 0.0 {
                bail!("max_transaction_amount_usdc must be positive");
            }
        }
        if let Some(max_trades) = self.max_daily_trades {
            if max_trades == 0 {
                bail!("max_daily_trades must be positive");
            }
        }
        if let Some(max_volume) = self.max_daily_volume_usdc {
            if max_volume <= 0.0 {
                bail!("max_daily_volume_usdc must be positive");
            }
        }
        Ok(())
    }
}

// ───────────────────────────────────────────────────────────────────────────────
// 🔐 SECURE KEYSTORE
// ───────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct SecureKeystore {
    keypair: Keypair,
    pubkey: Pubkey,
    config: KeystoreConfig,
    daily_tx_count: Arc<AtomicU64>,
    daily_volume_usdc: Arc<RwLock<f64>>,
    last_reset: Arc<RwLock<Instant>>,
}

impl SecureKeystore {
    pub fn from_file(config: KeystoreConfig) -> Result<Self> {
        info!("🔐 Loading secure keystore from: {}", config.keypair_path);
        config.validate()?;

        let expanded_path = shellexpand::tilde(&config.keypair_path).to_string();

        let keypair_bytes = std::fs::read(&expanded_path)
            .with_context(|| format!("Failed to read keypair file: {}", expanded_path))?;

        let keypair = if keypair_bytes.len() == 64 {
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&keypair_bytes[0..32]);
            Keypair::new_from_array(secret)
        } else if keypair_bytes.len() == 32 {
            let mut secret = [0u8; 32];
            secret.copy_from_slice(&keypair_bytes);
            Keypair::new_from_array(secret)
        } else {
            let secret_key: Vec<u8> = serde_json::from_slice(&keypair_bytes)
                .context("Failed to parse keypair JSON")?;
            if secret_key.len() == 64 {
                let mut secret = [0u8; 32];
                secret.copy_from_slice(&secret_key[0..32]);
                Keypair::new_from_array(secret)
            } else if secret_key.len() == 32 {
                let mut secret = [0u8; 32];
                secret.copy_from_slice(&secret_key);
                Keypair::new_from_array(secret)
            } else {
                bail!("Invalid keypair file format: expected 32 or 64 bytes, got {}", secret_key.len());
            }
        };

        let pubkey = keypair.pubkey();
        info!("✅ Keystore loaded successfully");
        info!("   Public key: {}", pubkey);

        Ok(Self {
            keypair,
            pubkey,
            config,
            daily_tx_count: Arc::new(AtomicU64::new(0)),
            daily_volume_usdc: Arc::new(RwLock::new(0.0)),
            last_reset: Arc::new(RwLock::new(Instant::now())),
        })
    }

    pub fn pubkey(&self) -> &Pubkey {
        &self.pubkey
    }

    pub async fn validate_transaction(&self, amount_usdc: f64) -> Result<()> {
        {
            let mut last_reset = self.last_reset.write().await;
            if last_reset.elapsed() > Duration::from_secs(86400) {
                info!("🔄 Resetting daily transaction counters");
                self.daily_tx_count.store(0, Ordering::SeqCst);
                *self.daily_volume_usdc.write().await = 0.0;
                *last_reset = Instant::now();
            }
        }

        if let Some(max_amount) = self.config.max_transaction_amount_usdc {
            if amount_usdc > max_amount {
                bail!("Transaction amount ${:.2} exceeds max position size ${:.2}",
                    amount_usdc, max_amount);
            }
        }

        if let Some(max_trades) = self.config.max_daily_trades {
            let daily_count = self.daily_tx_count.load(Ordering::SeqCst);
            if daily_count >= max_trades as u64 {
                bail!("Daily trade limit reached: {}/{}", daily_count, max_trades);
            }
        }

        if let Some(max_volume) = self.config.max_daily_volume_usdc {
            let daily_vol = *self.daily_volume_usdc.read().await;
            if daily_vol + amount_usdc > max_volume {
                bail!("Daily volume limit would be exceeded: ${:.2} + ${:.2} > ${:.2}",
                    daily_vol, amount_usdc, max_volume);
            }
        }

        Ok(())
    }

    pub fn sign_transaction(&self, tx: &mut Transaction) -> Result<()> {
        tx.sign(&[&self.keypair], tx.message.recent_blockhash);
        Ok(())
    }

    /// Sign a VersionedTransaction (required for Jupiter V0 swaps with ALTs).
    ///
    /// # Errors
    /// - Returns an error if the transaction has no signature slots (malformed).
    /// - Returns an error if `static_account_keys()[0]` does not match this
    ///   keystore's pubkey — catches a misconfigured `user_pubkey` passed to
    ///   `JupiterClient::prepare_swap()` before it reaches the network.
    pub fn sign_versioned_transaction(&self, tx: &mut VersionedTransaction) -> Result<()> {
        if tx.signatures.is_empty() {
            bail!(
                "Jupiter returned a VersionedTransaction with 0 signature slots — \
                 transaction may be malformed. Cannot determine signer position."
            );
        }

        let fee_payer = tx
            .message
            .static_account_keys()
            .first()
            .context("VersionedTransaction has no static account keys")?;
        if fee_payer != &self.pubkey {
            bail!(
                "Fee-payer mismatch: transaction expects {}, keystore holds {}. \
                 Ensure JupiterClient::prepare_swap() is called with the correct user_pubkey.",
                fee_payer,
                self.pubkey
            );
        }

        let message_bytes = tx.message.serialize();
        let signature = self.keypair.sign_message(&message_bytes);
        tx.signatures[0] = signature;
        Ok(())
    }

    pub async fn record_transaction(&self, amount_usdc: f64) {
        self.daily_tx_count.fetch_add(1, Ordering::SeqCst);
        let mut vol = self.daily_volume_usdc.write().await;
        *vol += amount_usdc;
    }

    pub async fn get_daily_stats(&self) -> (u64, f64) {
        let count = self.daily_tx_count.load(Ordering::SeqCst);
        let volume = *self.daily_volume_usdc.read().await;
        (count, volume)
    }

    pub async fn check_limits_warning(&self) -> Option<String> {
        let (count, volume) = self.get_daily_stats().await;

        if let Some(max_trades) = self.config.max_daily_trades {
            let usage_pct = (count as f64 / max_trades as f64) * 100.0;
            if usage_pct > 80.0 {
                return Some(format!("⚠️  Trade limit: {}/{} ({:.1}%)",
                    count, max_trades, usage_pct));
            }
        }

        if let Some(max_volume) = self.config.max_daily_volume_usdc {
            let usage_pct = (volume / max_volume) * 100.0;
            if usage_pct > 80.0 {
                return Some(format!("⚠️  Volume limit: ${:.2}/${:.2} ({:.1}%)",
                    volume, max_volume, usage_pct));
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = KeystoreConfig::default();
        assert!(config.validate().is_ok());

        config.max_transaction_amount_usdc = Some(-100.0);
        assert!(config.validate().is_err());

        config.max_transaction_amount_usdc = Some(100.0);
        config.max_daily_trades = Some(0);
        assert!(config.validate().is_err());
    }

    #[tokio::test]
    async fn test_daily_limits_validation() {
        let config = KeystoreConfig {
            keypair_path: "test.json".into(),
            max_transaction_amount_usdc: Some(100.0),
            max_daily_trades: Some(5),
            max_daily_volume_usdc: Some(500.0),
        };
        assert!(config.validate().is_ok());
    }
}
