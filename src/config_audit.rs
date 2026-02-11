//! Configuration Security Auditor
//!
//! Validates configuration for security issues and provides warnings
//! about potentially dangerous settings.
//!
//! NOTE: Currently placeholder - needs config structure updates

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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
    ///
    /// TODO: Update after config structure supports security fields
    pub fn audit(_config: &Config) -> Result<AuditReport> {
        let report = AuditReport::new();
        
        // Placeholder - will implement after config structure is updated
        // with proper security fields (circuit_breaker_max_loss_pct, etc.)
        
        Ok(report)
    }
}
