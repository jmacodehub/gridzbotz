//! Security configuration — wallet path, permissions, authorized programs.

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail, Context};
use std::fs;
use log::{info, warn};
use super::default_wallet_path;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityConfig {
    #[serde(default = "default_wallet_path")]
    pub wallet_path: String,
    #[serde(default)]
    pub require_password: bool,
    #[serde(default)]
    pub authorized_programs: Option<Vec<String>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            wallet_path: default_wallet_path(),
            require_password: false,
            authorized_programs: None,
        }
    }
}

impl SecurityConfig {
    pub fn validate(&self) -> Result<()> {
        if self.wallet_path.is_empty() {
            bail!("security.wallet_path cannot be empty");
        }
        Ok(())
    }

    pub fn validate_for_live_mode(&self) -> Result<()> {
        use std::path::PathBuf;
        let expanded_path = if self.wallet_path.starts_with('~') {
            let home = std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .context("Cannot determine home directory for ~ expansion")?;
            PathBuf::from(self.wallet_path.replacen('~', &home, 1))
        } else {
            PathBuf::from(&self.wallet_path)
        };
        if !expanded_path.exists() {
            bail!(
                "Wallet file not found: {}\n\
                 Ensure security.wallet_path in your config points to a valid keypair file.",
                expanded_path.display()
            );
        }
        if let Err(e) = fs::File::open(&expanded_path) {
            bail!(
                "Wallet file exists but cannot be read: {}\n\
                 Error: {}\n\
                 Check file permissions and ownership.",
                expanded_path.display(), e
            );
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = fs::metadata(&expanded_path) {
                let mode = metadata.permissions().mode();
                if mode & 0o004 != 0 {
                    warn!(
                        "⚠️ SECURITY: Wallet file is world-readable: {}\n\
                         Fix with: chmod 600 {}",
                        expanded_path.display(), expanded_path.display()
                    );
                }
            }
        }
        info!("✅ Wallet file validated: {}", expanded_path.display());
        Ok(())
    }
}
