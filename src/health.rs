//! Health Check Endpoint
//!
//! Provides HTTP health check for monitoring and alerting.
//! Returns system status, trading engine health, and key metrics.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use warp::Filter;

/// Health status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

/// Component health
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}

/// System metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub uptime_secs: u64,
    pub memory_used_mb: u64,
    pub cpu_usage_pct: f64,
}

/// Trading metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingMetrics {
    pub total_trades: u64,
    pub successful_trades: u64,
    pub failed_trades: u64,
    pub success_rate_pct: f64,
    pub circuit_breaker_active: bool,
}

/// Health check response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub timestamp: i64,
    pub version: String,
    pub components: Vec<ComponentHealth>,
    pub system: SystemMetrics,
    pub trading: TradingMetrics,
}

impl HealthResponse {
    /// Check if overall status is healthy
    pub fn is_healthy(&self) -> bool {
        self.status == HealthStatus::Healthy
    }

    /// Get HTTP status code
    pub fn http_status(&self) -> u16 {
        match self.status {
            HealthStatus::Healthy => 200,
            HealthStatus::Degraded => 200,  // Still operational
            HealthStatus::Unhealthy => 503, // Service unavailable
        }
    }
}

/// Health check provider trait
pub trait HealthProvider: Send + Sync {
    fn get_health(&self) -> ComponentHealth;
}

/// Health check service
pub struct HealthService {
    start_time: std::time::Instant,
    providers: Vec<Arc<dyn HealthProvider>>,
    trading_metrics: Arc<RwLock<TradingMetrics>>,
}

impl HealthService {
    /// Create new health service
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            providers: Vec::new(),
            trading_metrics: Arc::new(RwLock::new(TradingMetrics {
                total_trades: 0,
                successful_trades: 0,
                failed_trades: 0,
                success_rate_pct: 0.0,
                circuit_breaker_active: false,
            })),
        }
    }

    /// Register a health provider
    pub fn register(&mut self, provider: Arc<dyn HealthProvider>) {
        self.providers.push(provider);
    }

    /// Update trading metrics
    pub async fn update_trading_metrics(&self, metrics: TradingMetrics) {
        *self.trading_metrics.write().await = metrics;
    }

    /// Get current health status
    pub async fn get_health(&self) -> HealthResponse {
        let components: Vec<ComponentHealth> = self.providers
            .iter()
            .map(|p| p.get_health())
            .collect();

        // Determine overall status
        let status = if components.iter().any(|c| c.status == HealthStatus::Unhealthy) {
            HealthStatus::Unhealthy
        } else if components.iter().any(|c| c.status == HealthStatus::Degraded) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        let system = Self::get_system_metrics(&self.start_time);
        let trading = self.trading_metrics.read().await.clone();

        HealthResponse {
            status,
            timestamp: chrono::Utc::now().timestamp(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            components,
            system,
            trading,
        }
    }

    fn get_system_metrics(start_time: &std::time::Instant) -> SystemMetrics {
        let uptime_secs = start_time.elapsed().as_secs();
        
        // Get memory usage (cross-platform via sysinfo crate would be better,
        // but keeping it simple for now)
        let memory_used_mb = Self::get_memory_usage_mb();
        
        // CPU usage (simplified - would need sysinfo crate for real CPU %)
        let cpu_usage_pct = 0.0;

        SystemMetrics {
            uptime_secs,
            memory_used_mb,
            cpu_usage_pct,
        }
    }

    #[cfg(target_os = "linux")]
    fn get_memory_usage_mb() -> u64 {
        // Read from /proc/self/statm on Linux
        if let Ok(contents) = std::fs::read_to_string("/proc/self/statm") {
            let parts: Vec<&str> = contents.split_whitespace().collect();
            if let Some(rss_pages) = parts.get(1) {
                if let Ok(pages) = rss_pages.parse::<u64>() {
                    // Each page is typically 4KB
                    return (pages * 4) / 1024;
                }
            }
        }
        0
    }

    #[cfg(not(target_os = "linux"))]
    fn get_memory_usage_mb() -> u64 {
        // Fallback for non-Linux systems
        0
    }

    /// Start HTTP health check server
    pub async fn serve(self: Arc<Self>, port: u16) -> Result<()> {
        log::info!("❤️  Health check server starting on port {}", port);

        let health_route = warp::path("health")
            .and(warp::get())
            .and_then({
                let service = self.clone();
                move || {
                    let service = service.clone();
                    async move {
                        let health = service.get_health().await;
                        let status_code = health.http_status();
                        
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&health),
                            warp::http::StatusCode::from_u16(status_code).unwrap(),
                        ))
                    }
                }
            });

        let routes = health_route;

        log::info!("✅ Health check server ready at http://0.0.0.0:{}/health", port);
        warp::serve(routes).run(([0, 0, 0, 0], port)).await;

        Ok(())
    }
}

impl Default for HealthService {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple health provider for testing
pub struct SimpleHealthProvider {
    name: String,
    status: Arc<RwLock<HealthStatus>>,
}

impl SimpleHealthProvider {
    pub fn new(name: String) -> Self {
        Self {
            name,
            status: Arc::new(RwLock::new(HealthStatus::Healthy)),
        }
    }

    pub async fn set_status(&self, status: HealthStatus) {
        *self.status.write().await = status;
    }
}

impl HealthProvider for SimpleHealthProvider {
    fn get_health(&self) -> ComponentHealth {
        // Note: This uses blocking read which isn't ideal in async context,
        // but works for simple health checks
        let status = self.status.try_read()
            .map(|s| s.clone())
            .unwrap_or(HealthStatus::Degraded);

        ComponentHealth {
            name: self.name.clone(),
            status,
            message: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_service_creation() {
        let service = HealthService::new();
        let health = service.get_health().await;
        
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_health_with_healthy_component() {
        let mut service = HealthService::new();
        let provider = Arc::new(SimpleHealthProvider::new("test".to_string()));
        service.register(provider);

        let health = service.get_health().await;
        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.components.len(), 1);
    }

    #[tokio::test]
    async fn test_health_with_unhealthy_component() {
        let mut service = HealthService::new();
        let provider = Arc::new(SimpleHealthProvider::new("test".to_string()));
        provider.set_status(HealthStatus::Unhealthy).await;
        service.register(provider);

        let health = service.get_health().await;
        assert_eq!(health.status, HealthStatus::Unhealthy);
    }

    #[tokio::test]
    async fn test_health_with_degraded_component() {
        let mut service = HealthService::new();
        let provider = Arc::new(SimpleHealthProvider::new("test".to_string()));
        provider.set_status(HealthStatus::Degraded).await;
        service.register(provider);

        let health = service.get_health().await;
        assert_eq!(health.status, HealthStatus::Degraded);
    }

    #[tokio::test]
    async fn test_trading_metrics_update() {
        let service = HealthService::new();
        
        let metrics = TradingMetrics {
            total_trades: 100,
            successful_trades: 95,
            failed_trades: 5,
            success_rate_pct: 95.0,
            circuit_breaker_active: false,
        };

        service.update_trading_metrics(metrics.clone()).await;
        
        let health = service.get_health().await;
        assert_eq!(health.trading.total_trades, 100);
        assert_eq!(health.trading.success_rate_pct, 95.0);
    }

    #[test]
    fn test_http_status_codes() {
        let mut response = HealthResponse {
            status: HealthStatus::Healthy,
            timestamp: 0,
            version: "test".to_string(),
            components: vec![],
            system: SystemMetrics {
                uptime_secs: 0,
                memory_used_mb: 0,
                cpu_usage_pct: 0.0,
            },
            trading: TradingMetrics {
                total_trades: 0,
                successful_trades: 0,
                failed_trades: 0,
                success_rate_pct: 0.0,
                circuit_breaker_active: false,
            },
        };

        assert_eq!(response.http_status(), 200);

        response.status = HealthStatus::Degraded;
        assert_eq!(response.http_status(), 200);

        response.status = HealthStatus::Unhealthy;
        assert_eq!(response.http_status(), 503);
    }
}
