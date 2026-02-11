//! Configuration Security Auditor
//!
//! Validates configuration for security issues and provides warnings
//! about potentially dangerous settings.

use crate::Config;
use anyhow::Result;

/// Audit severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuditLevel {
    Info,
    Warning,
    Error,
}

impl std::fmt::Display for AuditLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditLevel::Info => write!(f, "ℹ️  INFO"),
            AuditLevel::Warning => write!(f, "⚠️  WARNING"),
            AuditLevel::Error => write!(f, "❌ ERROR"),
        }
    }
}

/// Audit finding
#[derive(Debug, Clone)]
pub struct AuditFinding {
    pub level: AuditLevel,
    pub category: String,
    pub message: String,
    pub recommendation: Option<String>,
}

impl AuditFinding {
    fn new(level: AuditLevel, category: &str, message: String) -> Self {
        Self {
            level,
            category: category.to_string(),
            message,
            recommendation: None,
        }
    }

    fn with_recommendation(mut self, rec: String) -> Self {
        self.recommendation = Some(rec);
        self
    }
}

/// Audit report
#[derive(Debug, Clone)]
pub struct AuditReport {
    pub findings: Vec<AuditFinding>,
}

impl AuditReport {
    fn new() -> Self {
        Self { findings: Vec::new() }
    }

    fn add(&mut self, finding: AuditFinding) {
        self.findings.push(finding);
    }

    /// Check if report has any errors
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.level == AuditLevel::Error)
    }

    /// Check if report has warnings
    pub fn has_warnings(&self) -> bool {
        self.findings.iter().any(|f| f.level == AuditLevel::Warning)
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.findings.iter().filter(|f| f.level == AuditLevel::Error).count()
    }

    /// Get warning count
    pub fn warning_count(&self) -> usize {
        self.findings.iter().filter(|f| f.level == AuditLevel::Warning).count()
    }

    /// Print formatted report
    pub fn print(&self) {
        println!("\n═══════════════════════════════════════════════════════════════");
        println!("  CONFIGURATION SECURITY AUDIT REPORT");
        println!("═══════════════════════════════════════════════════════════════\n");

        if self.findings.is_empty() {
            println!("✅ No security issues found!\n");
            return;
        }

        println!("Summary:");
        println!("  Errors:   {}", self.error_count());
        println!("  Warnings: {}\n", self.warning_count());

        // Group by category
        let mut categories: std::collections::HashMap<String, Vec<&AuditFinding>> =
            std::collections::HashMap::new();
        
        for finding in &self.findings {
            categories.entry(finding.category.clone())
                .or_insert_with(Vec::new)
                .push(finding);
        }

        for (category, findings) in categories {
            println!("─── {} ───", category);
            for finding in findings {
                println!("  {} {}", finding.level, finding.message);
                if let Some(rec) = &finding.recommendation {
                    println!("     💡 {}", rec);
                }
            }
            println!();
        }

        if self.has_errors() {
            println!("❌ CRITICAL: Configuration has security errors!");
            println!("   Fix these before running in production.\n");
        } else if self.has_warnings() {
            println!("⚠️  Configuration has warnings.");
            println!("   Review recommendations above.\n");
        }

        println!("═══════════════════════════════════════════════════════════════\n");
    }
}

/// Configuration auditor
pub struct ConfigAuditor;

impl ConfigAuditor {
    /// Audit configuration for security issues
    pub fn audit(config: &Config) -> Result<AuditReport> {
        let mut report = AuditReport::new();

        Self::audit_risk_management(config, &mut report);
        Self::audit_trading_limits(config, &mut report);
        Self::audit_network_security(config, &mut report);
        Self::audit_api_keys(config, &mut report);
        Self::audit_logging(config, &mut report);
        Self::audit_operational(config, &mut report);

        Ok(report)
    }

    fn audit_risk_management(config: &Config, report: &mut AuditReport) {
        // Circuit breaker
        if let Some(max_loss) = config.circuit_breaker_max_loss_pct {
            if max_loss > 20.0 {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "Risk Management",
                        format!("Circuit breaker at {:.1}% is very high", max_loss),
                    )
                    .with_recommendation("Consider 10% or lower for production".to_string()),
                );
            }
            if max_loss < 2.0 {
                report.add(AuditFinding::new(
                    AuditLevel::Info,
                    "Risk Management",
                    format!("Circuit breaker at {:.1}% is conservative (good!)", max_loss),
                ));
            }
        } else {
            report.add(
                AuditFinding::new(
                    AuditLevel::Error,
                    "Risk Management",
                    "Circuit breaker is disabled!".to_string(),
                )
                .with_recommendation("Enable with max_loss_pct = 10.0".to_string()),
            );
        }

        // Stop loss
        if config.stop_loss_pct.is_none() {
            report.add(
                AuditFinding::new(
                    AuditLevel::Warning,
                    "Risk Management",
                    "Stop loss not configured".to_string(),
                )
                .with_recommendation("Set stop_loss_pct to limit downside".to_string()),
            );
        }
    }

    fn audit_trading_limits(config: &Config, report: &mut AuditReport) {
        // Grid size
        if let Some(grid_size) = config.grid_size {
            if grid_size > 50 {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "Trading Limits",
                        format!("Large grid size: {} levels", grid_size),
                    )
                    .with_recommendation("May cause excessive trading fees".to_string()),
                );
            }
        }

        // Position size
        if let Some(pos_size) = config.position_size_sol {
            if pos_size > 100.0 {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "Trading Limits",
                        format!("Large position size: {} SOL", pos_size),
                    )
                    .with_recommendation("Ensure this matches your risk tolerance".to_string()),
                );
            }
        }
    }

    fn audit_network_security(config: &Config, report: &mut AuditReport) {
        // RPC URL
        if let Some(ref rpc_url) = config.rpc_url {
            if !rpc_url.starts_with("https://") {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Error,
                        "Network Security",
                        "RPC URL is not using HTTPS!".to_string(),
                    )
                    .with_recommendation("Use https:// for secure connections".to_string()),
                );
            }

            // Warn about public endpoints
            if rpc_url.contains("api.mainnet-beta.solana.com") {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "Network Security",
                        "Using public RPC endpoint".to_string(),
                    )
                    .with_recommendation(
                        "Consider private RPC (Triton, Helius, QuickNode) for production".to_string(),
                    ),
                );
            }
        }

        // WebSocket
        if let Some(ref ws_url) = config.ws_url {
            if !ws_url.starts_with("wss://") && !ws_url.starts_with("ws://localhost") {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Error,
                        "Network Security",
                        "WebSocket URL is not using WSS!".to_string(),
                    )
                    .with_recommendation("Use wss:// for secure WebSocket connections".to_string()),
                );
            }
        }
    }

    fn audit_api_keys(config: &Config, report: &mut AuditReport) {
        // Jupiter API key
        if let Some(ref key) = config.jupiter_api_key {
            if key == "your_api_key_here" || key == "test" || key.is_empty() {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Error,
                        "API Keys",
                        "Jupiter API key appears to be a placeholder".to_string(),
                    )
                    .with_recommendation("Set real API key in environment".to_string()),
                );
            }
        }

        // Jito API key (if using MEV protection)
        if let Some(ref key) = config.jito_api_key {
            if key == "your_api_key_here" || key == "test" || key.is_empty() {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "API Keys",
                        "Jito API key appears to be a placeholder".to_string(),
                    )
                    .with_recommendation("Set real key if using MEV protection".to_string()),
                );
            }
        }
    }

    fn audit_logging(config: &Config, report: &mut AuditReport) {
        // Check if logging is configured
        if let Some(ref log_level) = config.log_level {
            if log_level == "trace" || log_level == "debug" {
                report.add(
                    AuditFinding::new(
                        AuditLevel::Warning,
                        "Logging",
                        format!("Log level set to '{}' (verbose)", log_level),
                    )
                    .with_recommendation(
                        "Use 'info' or 'warn' in production to reduce noise".to_string(),
                    ),
                );
            }
        }
    }

    fn audit_operational(config: &Config, report: &mut AuditReport) {
        // Paper trading check
        if config.paper_trading.unwrap_or(true) {
            report.add(
                AuditFinding::new(
                    AuditLevel::Info,
                    "Operational",
                    "Paper trading mode ENABLED (safe for testing)".to_string(),
                )
            );
        } else {
            report.add(
                AuditFinding::new(
                    AuditLevel::Warning,
                    "Operational",
                    "REAL TRADING MODE - Using actual funds!".to_string(),
                )
                .with_recommendation("Double-check all settings before starting".to_string()),
            );
        }

        // Auto-start check
        if config.auto_start.unwrap_or(false) {
            report.add(
                AuditFinding::new(
                    AuditLevel::Warning,
                    "Operational",
                    "Auto-start is ENABLED".to_string(),
                )
                .with_recommendation(
                    "Bot will start trading immediately on launch".to_string(),
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_safe_config() {
        let mut config = Config::default();
        config.circuit_breaker_max_loss_pct = Some(10.0);
        config.stop_loss_pct = Some(5.0);
        config.rpc_url = Some("https://api.devnet.solana.com".to_string());
        config.paper_trading = Some(true);

        let report = ConfigAuditor::audit(&config).unwrap();
        assert!(!report.has_errors());
    }

    #[test]
    fn test_audit_insecure_rpc() {
        let mut config = Config::default();
        config.rpc_url = Some("http://insecure.example.com".to_string());

        let report = ConfigAuditor::audit(&config).unwrap();
        assert!(report.has_errors());
        assert!(report.findings.iter().any(|f| 
            f.message.contains("HTTPS")
        ));
    }

    #[test]
    fn test_audit_no_circuit_breaker() {
        let mut config = Config::default();
        config.circuit_breaker_max_loss_pct = None;

        let report = ConfigAuditor::audit(&config).unwrap();
        assert!(report.has_errors());
    }

    #[test]
    fn test_audit_placeholder_api_key() {
        let mut config = Config::default();
        config.jupiter_api_key = Some("test".to_string());

        let report = ConfigAuditor::audit(&config).unwrap();
        assert!(report.has_errors());
    }

    #[test]
    fn test_report_counts() {
        let mut report = AuditReport::new();
        
        report.add(AuditFinding::new(
            AuditLevel::Error,
            "Test",
            "Error 1".to_string(),
        ));
        report.add(AuditFinding::new(
            AuditLevel::Warning,
            "Test",
            "Warning 1".to_string(),
        ));
        report.add(AuditFinding::new(
            AuditLevel::Warning,
            "Test",
            "Warning 2".to_string(),
        ));

        assert_eq!(report.error_count(), 1);
        assert_eq!(report.warning_count(), 2);
        assert!(report.has_errors());
        assert!(report.has_warnings());
    }
}
