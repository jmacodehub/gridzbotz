//! Prometheus Metrics Exporter
//!
//! Exposes trading and system metrics in Prometheus format.
//! Enables monitoring, alerting, and observability.

use std::sync::Arc;
use tokio::sync::RwLock;

/// Metric type
#[derive(Debug, Clone)]
pub enum MetricType {
    Counter,
    Gauge,
    Histogram,
}

/// Individual metric
#[derive(Debug, Clone)]
pub struct Metric {
    pub name: String,
    pub metric_type: MetricType,
    pub help: String,
    pub value: f64,
    pub labels: Vec<(String, String)>,
}

impl Metric {
    /// Format as Prometheus exposition format
    pub fn format(&self) -> String {
        let type_name = match self.metric_type {
            MetricType::Counter => "counter",
            MetricType::Gauge => "gauge",
            MetricType::Histogram => "histogram",
        };

        let mut output = String::new();
        output.push_str(&format!("# HELP {} {}\n", self.name, self.help));
        output.push_str(&format!("# TYPE {} {}\n", self.name, type_name));

        if self.labels.is_empty() {
            output.push_str(&format!("{} {}\n", self.name, self.value));
        } else {
            let labels: Vec<String> = self.labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, v))
                .collect();
            output.push_str(&format!(
                "{}{{{}}} {}\n",
                self.name,
                labels.join(","),
                self.value
            ));
        }

        output
    }
}

/// Metrics registry
pub struct MetricsRegistry {
    metrics: Arc<RwLock<Vec<Metric>>>,
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Register a metric
    pub async fn register(&self, metric: Metric) {
        let mut metrics = self.metrics.write().await;
        
        // Update if exists, otherwise add
        if let Some(existing) = metrics.iter_mut().find(|m| {
            m.name == metric.name && m.labels == metric.labels
        }) {
            existing.value = metric.value;
        } else {
            metrics.push(metric);
        }
    }

    /// Increment a counter
    pub async fn inc_counter(&self, name: &str, labels: Vec<(String, String)>) {
        let mut metrics = self.metrics.write().await;
        
        if let Some(metric) = metrics.iter_mut().find(|m| {
            m.name == name && m.labels == labels
        }) {
            metric.value += 1.0;
        } else {
            metrics.push(Metric {
                name: name.to_string(),
                metric_type: MetricType::Counter,
                help: format!("Auto-generated counter for {}", name),
                value: 1.0,
                labels,
            });
        }
    }

    /// Set a gauge value
    pub async fn set_gauge(&self, name: &str, value: f64, labels: Vec<(String, String)>) {
        self.register(Metric {
            name: name.to_string(),
            metric_type: MetricType::Gauge,
            help: format!("Auto-generated gauge for {}", name),
            value,
            labels,
        }).await;
    }

    /// Export all metrics in Prometheus format
    pub async fn export(&self) -> String {
        let metrics = self.metrics.read().await;
        
        let mut output = String::new();
        for metric in metrics.iter() {
            output.push_str(&metric.format());
        }
        
        output
    }

    /// Clear all metrics (for testing)
    pub async fn clear(&self) {
        self.metrics.write().await.clear();
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Trading metrics collector
pub struct TradingMetrics {
    registry: Arc<MetricsRegistry>,
}

impl TradingMetrics {
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self { registry }
    }

    /// Record a trade
    pub async fn record_trade(&self, side: &str, success: bool) {
        self.registry
            .inc_counter(
                "gridbot_trades_total",
                vec![
                    ("side".to_string(), side.to_string()),
                    ("success".to_string(), success.to_string()),
                ],
            )
            .await;
    }

    /// Set current PnL
    pub async fn set_pnl(&self, pnl: f64) {
        self.registry
            .set_gauge("gridbot_pnl_usd", pnl, vec![])
            .await;
    }

    /// Set current balance
    pub async fn set_balance(&self, token: &str, amount: f64) {
        self.registry
            .set_gauge(
                "gridbot_balance",
                amount,
                vec![("token".to_string(), token.to_string())],
            )
            .await;
    }

    /// Record circuit breaker trip
    pub async fn record_circuit_breaker_trip(&self, reason: &str) {
        self.registry
            .inc_counter(
                "gridbot_circuit_breaker_trips_total",
                vec![("reason".to_string(), reason.to_string())],
            )
            .await;
    }

    /// Set circuit breaker state
    pub async fn set_circuit_breaker_active(&self, active: bool) {
        self.registry
            .set_gauge("gridbot_circuit_breaker_active", if active { 1.0 } else { 0.0 }, vec![])
            .await;
    }

    /// Record RPC request
    pub async fn record_rpc_request(&self, endpoint: &str, success: bool, latency_ms: f64) {
        self.registry
            .inc_counter(
                "gridbot_rpc_requests_total",
                vec![
                    ("endpoint".to_string(), endpoint.to_string()),
                    ("success".to_string(), success.to_string()),
                ],
            )
            .await;

        self.registry
            .register(Metric {
                name: "gridbot_rpc_latency_ms".to_string(),
                metric_type: MetricType::Histogram,
                help: "RPC request latency in milliseconds".to_string(),
                value: latency_ms,
                labels: vec![("endpoint".to_string(), endpoint.to_string())],
            })
            .await;
    }
}

/// System metrics collector
pub struct SystemMetrics {
    registry: Arc<MetricsRegistry>,
    start_time: std::time::Instant,
}

impl SystemMetrics {
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self {
            registry,
            start_time: std::time::Instant::now(),
        }
    }

    /// Update system metrics
    pub async fn update(&self) {
        // Uptime
        let uptime_secs = self.start_time.elapsed().as_secs() as f64;
        self.registry
            .set_gauge("gridbot_uptime_seconds", uptime_secs, vec![])
            .await;

        // Memory (simplified)
        #[cfg(target_os = "linux")]
        {
            if let Ok(contents) = std::fs::read_to_string("/proc/self/statm") {
                let parts: Vec<&str> = contents.split_whitespace().collect();
                if let Some(rss_pages) = parts.get(1) {
                    if let Ok(pages) = rss_pages.parse::<u64>() {
                        let memory_mb = (pages * 4) as f64 / 1024.0;
                        self.registry
                            .set_gauge("gridbot_memory_usage_mb", memory_mb, vec![])
                            .await;
                    }
                }
            }
        }

        // Thread count
        let thread_count = std::thread::available_parallelism()
            .map(|n| n.get() as f64)
            .unwrap_or(0.0);
        self.registry
            .set_gauge("gridbot_threads_available", thread_count, vec![])
            .await;
    }
}

/// Metrics HTTP server
pub struct MetricsServer {
    registry: Arc<MetricsRegistry>,
}

impl MetricsServer {
    pub fn new(registry: Arc<MetricsRegistry>) -> Self {
        Self { registry }
    }

    /// Start metrics server on given port
    pub async fn serve(self, port: u16) -> Result<(), Box<dyn std::error::Error>> {
        use warp::Filter;

        log::info!("📊 Metrics server starting on port {}", port);

        let registry = self.registry.clone();
        let metrics_route = warp::path("metrics")
            .and(warp::get())
            .and_then(move || {
                let registry = registry.clone();
                async move {
                    let metrics = registry.export().await;
                    Ok::<_, warp::Rejection>(warp::reply::with_header(
                        metrics,
                        "Content-Type",
                        "text/plain; version=0.0.4",
                    ))
                }
            });

        log::info!("✅ Metrics server ready at http://0.0.0.0:{}/metrics", port);
        warp::serve(metrics_route).run(([0, 0, 0, 0], port)).await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metric_formatting() {
        let metric = Metric {
            name: "test_counter".to_string(),
            metric_type: MetricType::Counter,
            help: "Test counter".to_string(),
            value: 42.0,
            labels: vec![],
        };

        let formatted = metric.format();
        assert!(formatted.contains("# HELP test_counter Test counter"));
        assert!(formatted.contains("# TYPE test_counter counter"));
        assert!(formatted.contains("test_counter 42"));
    }

    #[tokio::test]
    async fn test_metric_with_labels() {
        let metric = Metric {
            name: "test_gauge".to_string(),
            metric_type: MetricType::Gauge,
            help: "Test gauge".to_string(),
            value: 100.5,
            labels: vec![
                ("env".to_string(), "prod".to_string()),
                ("region".to_string(), "us-east".to_string()),
            ],
        };

        let formatted = metric.format();
        assert!(formatted.contains("test_gauge{env=\"prod\",region=\"us-east\"} 100.5"));
    }

    #[tokio::test]
    async fn test_registry_increment() {
        let registry = MetricsRegistry::new();

        registry.inc_counter("test_counter", vec![]).await;
        registry.inc_counter("test_counter", vec![]).await;
        registry.inc_counter("test_counter", vec![]).await;

        let exported = registry.export().await;
        assert!(exported.contains("test_counter 3"));
    }

    #[tokio::test]
    async fn test_registry_gauge() {
        let registry = MetricsRegistry::new();

        registry.set_gauge("test_gauge", 42.5, vec![]).await;
        registry.set_gauge("test_gauge", 100.0, vec![]).await; // Update

        let exported = registry.export().await;
        assert!(exported.contains("test_gauge 100"));
        assert!(!exported.contains("42.5"));
    }

    #[tokio::test]
    async fn test_trading_metrics() {
        let registry = Arc::new(MetricsRegistry::new());
        let metrics = TradingMetrics::new(registry.clone());

        metrics.record_trade("buy", true).await;
        metrics.record_trade("buy", true).await;
        metrics.record_trade("sell", false).await;
        metrics.set_pnl(1234.56).await;

        let exported = registry.export().await;
        assert!(exported.contains("gridbot_trades_total{side=\"buy\",success=\"true\"} 2"));
        assert!(exported.contains("gridbot_trades_total{side=\"sell\",success=\"false\"} 1"));
        assert!(exported.contains("gridbot_pnl_usd 1234.56"));
    }

    #[tokio::test]
    async fn test_circuit_breaker_metrics() {
        let registry = Arc::new(MetricsRegistry::new());
        let metrics = TradingMetrics::new(registry.clone());

        metrics.set_circuit_breaker_active(true).await;
        metrics.record_circuit_breaker_trip("max_loss").await;

        let exported = registry.export().await;
        assert!(exported.contains("gridbot_circuit_breaker_active 1"));
        assert!(exported.contains("gridbot_circuit_breaker_trips_total"));
    }

    #[tokio::test]
    async fn test_system_metrics() {
        let registry = Arc::new(MetricsRegistry::new());
        let metrics = SystemMetrics::new(registry.clone());

        metrics.update().await;

        let exported = registry.export().await;
        assert!(exported.contains("gridbot_uptime_seconds"));
    }
}
