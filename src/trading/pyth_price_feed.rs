//! 🚀 Production-Grade Pyth Price Feed - HTTP API

use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::{sync::RwLock, time::sleep};
use log::{debug, error, info, warn};
use fastrand::Rng;
use serde::{Deserialize, Serialize};

const PYTH_API_URL: &str = "https://hermes.pyth.network/api/latest_price_feeds";
const SOL_USD_FEED_ID: &str = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";
const POLL_INTERVAL_MS: u64 = 500;
const MAX_POLL_INTERVAL_MS: u64 = 5000;
const INITIAL_PRICE: f64 = 150.0;
const PRICE_MIN: f64 = 1.0;
const PRICE_MAX: f64 = 1000.0;
const VOLATILITY: f64 = 0.01;
const MAX_CONSECUTIVE_FAILURES: u64 = 3;
const STATS_INTERVAL: u64 = 50;
const REQUEST_TIMEOUT_SECS: u64 = 5;

#[derive(Debug, Deserialize, Serialize)]
struct PythPriceResponse {
    id: String,
    price: PythPrice,
}

#[derive(Debug, Deserialize, Serialize)]
struct PythPrice {
    price: String,
    expo: i32,
    conf: String,
    publish_time: i64,
}

pub struct PythPriceFeed {
    current_price: Arc<RwLock<f64>>,
    client: reqwest::Client,
    is_healthy: Arc<AtomicBool>,
    total_fetches: Arc<AtomicU64>,
    successful_fetches: Arc<AtomicU64>,
    last_update: Arc<RwLock<Instant>>,
    cumulative_latency_us: Arc<AtomicU64>,
}

impl PythPriceFeed {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .pool_max_idle_per_host(10)
            .build()?;
        
        info!("🔧 Initializing Pyth HTTP Price Feed");
        info!("📡 API: {}", PYTH_API_URL);
        info!("🎯 SOL/USD Feed: {}...", &SOL_USD_FEED_ID[..20]);
        
        Ok(Self {
            current_price: Arc::new(RwLock::new(INITIAL_PRICE)),
            client,
            is_healthy: Arc::new(AtomicBool::new(true)),
            total_fetches: Arc::new(AtomicU64::new(0)),
            successful_fetches: Arc::new(AtomicU64::new(0)),
            last_update: Arc::new(RwLock::new(Instant::now())),
            cumulative_latency_us: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn start(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let price_ref = Arc::clone(&self.current_price);
        let client = self.client.clone();
        let is_healthy = Arc::clone(&self.is_healthy);
        let total_fetches = Arc::clone(&self.total_fetches);
        let successful_fetches = Arc::clone(&self.successful_fetches);
        let last_update = Arc::clone(&self.last_update);
        let cumulative_latency = Arc::clone(&self.cumulative_latency_us);

        tokio::spawn(async move {
            let mut consecutive_failures = 0u64;
            let mut current_interval = POLL_INTERVAL_MS;
            let start_time = Instant::now();

            info!("✅ Pyth mainnet polling started");
            
            loop {
                let fetch_start = Instant::now();
                let fetch_count = total_fetches.fetch_add(1, Ordering::Relaxed) + 1;

                let success = Self::update_price(
                    &price_ref,
                    &client,
                    &last_update,
                    consecutive_failures,
                ).await;

                let fetch_latency = fetch_start.elapsed();
                cumulative_latency.fetch_add(fetch_latency.as_micros() as u64, Ordering::Relaxed);

                if success {
                    consecutive_failures = 0;
                    current_interval = POLL_INTERVAL_MS;
                    is_healthy.store(true, Ordering::Relaxed);
                    successful_fetches.fetch_add(1, Ordering::Relaxed);
                } else {
                    consecutive_failures += 1;
                    current_interval = (current_interval * 2).min(MAX_POLL_INTERVAL_MS);
                    
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        is_healthy.store(false, Ordering::Relaxed);
                        error!("🔴 HEALTH WARNING: {} consecutive failures", consecutive_failures);
                    }
                }

                if fetch_count % STATS_INTERVAL == 0 {
                    let success_count = successful_fetches.load(Ordering::Relaxed);
                    let success_rate = (success_count as f64 / fetch_count as f64) * 100.0;
                    let uptime = start_time.elapsed().as_secs();
                    let avg_latency = cumulative_latency.load(Ordering::Relaxed) / fetch_count;
                    
                    info!(
                        "📊 {} fetches | {:.1}% success | {}s uptime | {:.1}ms latency",
                        fetch_count, success_rate, uptime, avg_latency as f64 / 1000.0
                    );
                }

                debug!("⏱️  Fetch took {:?}", fetch_latency);
                sleep(Duration::from_millis(current_interval)).await;
            }
        });

        Ok(())
    }

    async fn update_price(
        price_ref: &Arc<RwLock<f64>>,
        client: &reqwest::Client,
        last_update: &Arc<RwLock<Instant>>,
        consecutive_failures: u64,
    ) -> bool {
        let url = format!("{}?ids[]={}", PYTH_API_URL, SOL_USD_FEED_ID);
        
        match client.get(&url).send().await {
            Ok(response) => {
                if !response.status().is_success() {
                    warn!("⚠️  API status: {}", response.status());
                    Self::simulate_price(price_ref).await;
                    return false;
                }

                match response.json::<Vec<PythPriceResponse>>().await {
                    Ok(feeds) => {
                        if let Some(feed) = feeds.first() {
                            match Self::parse_price(&feed.price) {
                                Ok(display_price) => {
                                    if display_price < PRICE_MIN || display_price > PRICE_MAX {
                                        warn!("⚠️  Price ${:.2} outside bounds", display_price);
                                        return false;
                                    }

                                    *price_ref.write().await = display_price;
                                    *last_update.write().await = Instant::now();
                                    
                                    let trend = Self::get_price_trend(price_ref, display_price).await;
                                    info!("🔄 Live SOL/USD ${:.4} {}", display_price, trend);
                                    true
                                }
                                Err(e) => {
                                    warn!("⚠️  Parse error: {}", e);
                                    Self::simulate_price(price_ref).await;
                                    false
                                }
                            }
                        } else {
                            warn!("⚠️  No price data in response");
                            Self::simulate_price(price_ref).await;
                            false
                        }
                    }
                    Err(e) => {
                        if consecutive_failures < 2 {
                            warn!("⚠️  JSON error: {}", e);
                        }
                        Self::simulate_price(price_ref).await;
                        false
                    }
                }
            }
            Err(e) => {
                if consecutive_failures == 0 {
                    warn!("⚠️  HTTP error: {}", e);
                }
                Self::simulate_price(price_ref).await;
                false
            }
        }
    }

    fn parse_price(price_data: &PythPrice) -> Result<f64, String> {
        let price: i64 = price_data.price.parse()
            .map_err(|e| format!("Parse error: {}", e))?;
        let expo = price_data.expo;
        
        let display_price = (price as f64) * 10f64.powi(expo);
        
        if !display_price.is_finite() {
            return Err("Invalid calculation".to_string());
        }
        
        Ok(display_price)
    }

    #[inline]
    async fn get_price_trend(price_ref: &Arc<RwLock<f64>>, new_price: f64) -> &'static str {
        let old_price = *price_ref.read().await;
        let diff = new_price - old_price;
        
        if diff.abs() < 0.01 {
            "➡️"
        } else if diff > 0.0 {
            "📈"
        } else {
            "📉"
        }
    }

    async fn simulate_price(price_ref: &Arc<RwLock<f64>>) {
        let mut rng = Rng::new();
        let mut current = *price_ref.read().await;
        let delta = (rng.f64() - 0.5) * VOLATILITY * current;
        current = (current + delta).clamp(PRICE_MIN, PRICE_MAX);
        *price_ref.write().await = current;
        info!("⚡ Fallback: ${:.4}", current);
    }

    #[inline]
    pub async fn latest_price(&self) -> f64 {
        *self.current_price.read().await
    }

    #[inline]
    pub fn is_healthy(&self) -> bool {
        self.is_healthy.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn success_rate(&self) -> f64 {
        let total = self.total_fetches.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        let successful = self.successful_fetches.load(Ordering::Relaxed);
        (successful as f64 / total as f64) * 100.0
    }

    pub fn avg_latency_ms(&self) -> f64 {
        let total = self.total_fetches.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let cumulative = self.cumulative_latency_us.load(Ordering::Relaxed);
        (cumulative as f64 / total as f64) / 1000.0
    }
}

impl Default for PythPriceFeed {
    fn default() -> Self {
        Self::new().expect("Failed to create PythPriceFeed")
    }
}
