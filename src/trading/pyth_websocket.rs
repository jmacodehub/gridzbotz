//! ═══════════════════════════════════════════════════════════════════════════
//! PYTH HERMES V2 / WORLD-CLASS WEBSOCKET INTEGRATION (2025)
//! - Ultra-fast, robust, observable, and scalable.
//! - Designed for Solana trading bot + institutional infra
//! ═══════════════════════════════════════════════════════════════════════════

use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use futures_util::{StreamExt, SinkExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::time::{interval, Duration, sleep, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use dashmap::DashMap;
use log::{info, warn, error, debug};
use rand::Rng;
use std::collections::HashMap;

// Update to read feeds dynamically from config (TOML/YAML)
fn load_feed_ids() -> Vec<String> {
    vec![
        // SOL/USD, BTC/USD, ETH/USD
        "0xef0d8b6fda2ceba41da39a73436148de22aeb0b51deb47e5f6bdc5caf5bcb3d4".into(),
        "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43".into(),
        "0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace".into(),
        // Add more from config here
    ]
}

// Hermes websocket constants
const PYTH_HERMES_WS_URL: &str = "wss://hermes.pyth.network/ws";
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 30000;
const BACKOFF_MULTIPLIER: f64 = 1.5;
const MAX_CONSECUTIVE_ERRORS: u32 = 5;
const HEARTBEAT_INTERVAL_SECS: u64 = 30;
const CACHE_PRUNE_SECS: u64 = 180;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceUpdate {
    pub feed_id: String,
    pub price: f64,
    pub confidence: f64,
    pub publish_time: i64,
    pub received_at_micros: u64,
    #[serde(default)]
    pub num_publishers: u8,
}

// Streaming message formats (as above)
#[derive(Debug, Deserialize)]
struct HermesStreamResponse {
    #[serde(default)]
    parsed: Option<Vec<ParsedPriceFeed>>,
}
#[derive(Debug, Deserialize)]
struct ParsedPriceFeed {
    id: String,
    price: PriceInfo,
    #[serde(default)]
    ema_price: Option<PriceInfo>,
}
#[derive(Debug, Deserialize)]
struct PriceInfo {
    price: String,
    conf: String,
    expo: i32,
    publish_time: i64,
}

pub struct PythWebSocketFeed {
    price_cache: Arc<DashMap<String, PriceUpdate>>,
    price_tx: mpsc::Sender<PriceUpdate>,
    price_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<PriceUpdate>>>,
    is_connected: Arc<AtomicBool>,
    messages_received: Arc<AtomicU64>,
    last_update_micros: Arc<AtomicU64>,
    feed_ids: Vec<String>,
}

impl PythWebSocketFeed {
    pub fn new(feed_ids: Vec<String>) -> Self {
        let (price_tx, price_rx) = mpsc::channel(10000);
        Self {
            price_cache: Arc::new(DashMap::new()),
            price_tx,
            price_rx: Arc::new(tokio::sync::Mutex::new(price_rx)),
            is_connected: Arc::new(AtomicBool::new(false)),
            messages_received: Arc::new(AtomicU64::new(0)),
            last_update_micros: Arc::new(AtomicU64::new(0)),
            feed_ids,
        }
    }

    pub async fn start(&self) {
        let price_cache = self.price_cache.clone();
        let price_tx = self.price_tx.clone();
        let is_connected = self.is_connected.clone();
        let messages_received = self.messages_received.clone();
        let last_update_micros = self.last_update_micros.clone();
        let feed_ids = self.feed_ids.clone();

        tokio::spawn(async move {
            Self::connection_loop(
                price_cache, price_tx, is_connected, messages_received, last_update_micros, feed_ids
            ).await
        });

        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if self.is_connected.load(Ordering::Relaxed) {
                info!("✅ Hermes streaming connected successfully");
                return;
            }
            sleep(Duration::from_millis(100)).await;
        }
        warn!("⚠ Initial connection may take longer, reconnection loop is active.");
    }

    async fn connection_loop(
        price_cache: Arc<DashMap<String, PriceUpdate>>,
        price_tx: mpsc::Sender<PriceUpdate>,
        is_connected: Arc<AtomicBool>,
        messages_received: Arc<AtomicU64>,
        last_update_micros: Arc<AtomicU64>,
        feed_ids: Vec<String>,
    ) {
        let mut backoff_ms = INITIAL_BACKOFF_MS;
        let mut connection_attempts = 0u64;
        loop {
            connection_attempts += 1;
            let query_params: Vec<String> = feed_ids.iter().map(|id| format!("ids[]={}", id)).collect();
            let url = format!("{}?{}", PYTH_HERMES_WS_URL, query_params.join("&"));
            debug!("🔌 Connecting: {}", url);

            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("✅ Connected on attempt {}", connection_attempts);
                    is_connected.store(true, Ordering::Relaxed);
                    backoff_ms = INITIAL_BACKOFF_MS;

                    let (write, mut read) = ws_stream.split();
                    let write_shared = Arc::new(tokio::sync::Mutex::new(write));
                    let heartbeat_write = write_shared.clone();
                    let heartbeat_handle = tokio::spawn(async move {
                        Self::heartbeat_loop(heartbeat_write).await
                    });

                    // Background prune old prices
                    let prune_cache = price_cache.clone();
                    tokio::spawn(async move {
                        let mut prune_intvl = interval(Duration::from_secs(CACHE_PRUNE_SECS));
                        loop {
                            prune_intvl.tick().await;
                            let now = Self::get_timestamp_micros();
                            for entry in prune_cache.iter() {
                                if now - entry.value().received_at_micros > CACHE_PRUNE_SECS * 1_000_000 {
                                    prune_cache.remove(entry.key());
                                }
                            }
                        }
                    });

                    let mut consecutive_errors = 0;
                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(Message::Text(text)) => {
                                consecutive_errors = 0;
                                match Self::parse_hermes_message(&text) {
                                    Ok(updates) => {
                                        let now_micros = Self::get_timestamp_micros();
                                        for mut update in updates {
                                            update.received_at_micros = now_micros;
                                            price_cache.insert(update.feed_id.clone(), update.clone());
                                            let count = messages_received.fetch_add(1, Ordering::Relaxed) + 1;
                                            last_update_micros.store(now_micros, Ordering::Relaxed);
                                            let _ = price_tx.try_send(update.clone());
                                            if count == 1 { info!("🎉 1st update received."); }
                                            else if count % 1000 == 0 { info!("📊 {} updates.", count); }
                                        }
                                    },
                                    Err(e) => { debug!("Message parse failed: {}", e); }
                                }
                            },
                            Ok(Message::Ping(_)) | Ok(Message::Pong(_)) => {
                                debug!("💓 Heartbeat ping/pong");
                            },
                            Ok(Message::Close(frame)) => {
                                warn!("🔌 Stream closed by server: {:?}", frame);
                                break;
                            },
                            Err(e) => {
                                consecutive_errors += 1;
                                error!("❌ Stream error #{}: {}", consecutive_errors, e);
                                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                                    error!("💥 Too many consecutive errors, reconnecting...");
                                    break;
                                }
                            },
                            Ok(Message::Binary(_)) | Ok(Message::Frame(_)) => {}
                        }
                    }
                    heartbeat_handle.abort();
                    is_connected.store(false, Ordering::Relaxed);
                    warn!("⚠️ Disconnected from Hermes, reconnecting...");
                },
                Err(e) => {
                    error!(
                        "❌ Connection attempt {} failed: {:?}",
                        connection_attempts, e
                    );
                    is_connected.store(false, Ordering::Relaxed);
                }
            }
            // Backoff with jitter (randomize delay +/- 30%)
            let jitter = rand::thread_rng().gen_range(
                -(backoff_ms as i64 / 3)..(backoff_ms as i64 / 3)
            );
            let wait_ms = (backoff_ms as i64 + jitter).max(0) as u64;
            warn!("⏳ Waiting {}ms before reconnect attempt {}", wait_ms, connection_attempts + 1);
            sleep(Duration::from_millis(wait_ms)).await;
            backoff_ms = ((backoff_ms as f64) * BACKOFF_MULTIPLIER) as u64;
            backoff_ms = backoff_ms.min(MAX_BACKOFF_MS);
        }
    }

    async fn heartbeat_loop(
        write: Arc<tokio::sync::Mutex<futures_util::stream::SplitSink<
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
            >,
            Message
        >>>
    ) {
        let mut heartbeat_interval = interval(Duration::from_secs(HEARTBEAT_INTERVAL_SECS));
        heartbeat_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            heartbeat_interval.tick().await;
            let mut writer = write.lock().await;
            if let Err(e) = writer.send(Message::Ping(vec![].into())).await {
                error!("💔 Heartbeat failed: {}", e);
                break;
            }
            debug!("💓 Heartbeat ping sent");
        }
    }

    fn parse_hermes_message(text: &str) -> anyhow::Result<Vec<PriceUpdate>> {
        let response: HermesStreamResponse = serde_json::from_str(text)?;
        let mut updates = Vec::new();
        if let Some(parsed_feeds) = response.parsed {
            for feed in parsed_feeds {
                let price_val = feed.price.price.parse::<i64>()?;
                let conf_val = feed.price.conf.parse::<u64>()?;
                let expo = feed.price.expo;
                let price = (price_val as f64) * 10f64.powi(expo);
                let confidence = (conf_val as f64) * 10f64.powi(expo);
                updates.push(PriceUpdate {
                    feed_id: feed.id,
                    price,
                    confidence,
                    publish_time: feed.price.publish_time,
                    received_at_micros: 0,
                    num_publishers: 0,
                });
            }
        }
        if updates.is_empty() { anyhow::bail!("No price data in Hermes message"); }
        Ok(updates)
    }

    pub async fn get_price(&self, feed_id: &str) -> Option<f64> {
        self.price_cache.get(feed_id).map(|entry| entry.price)
    }
    pub async fn get_price_update(&self, feed_id: &str) -> Option<PriceUpdate> {
        self.price_cache.get(feed_id).map(|entry| entry.clone())
    }
    pub async fn get_all_prices(&self) -> HashMap<String, f64> {
        self.price_cache.iter().map(|entry| (entry.key().clone(), entry.value().price)).collect()
    }
    pub async fn is_healthy(&self) -> bool {
        self.is_connected.load(Ordering::Relaxed)
    }
    pub async fn stats(&self) -> (u64, bool) {
        let count = self.messages_received.load(Ordering::Relaxed);
        let connected = self.is_connected.load(Ordering::Relaxed);
        (count, connected)
    }
    pub async fn latency_micros(&self) -> Option<u64> {
        let last_update = self.last_update_micros.load(Ordering::Relaxed);
        if last_update == 0 { None }
        else {
            let now = Self::get_timestamp_micros();
            Some(now.saturating_sub(last_update))
        }
    }
    pub async fn display_stats(&self) {
        let (count, connected) = self.stats().await;
        let all_prices = self.get_all_prices().await;
        let latency = self.latency_micros().await;
        println!("\n📊 Pyth WebSocket Statistics\n═══════════════════");
        println!("Status:     {}", if connected { "✅ Connected" } else { "❌ Disconnected" });
        println!("Messages:   {}", count);
        println!("Active Feeds: {}", all_prices.len());
        if let Some(latency_us) = latency {
            println!("Last Update: {}ms ago", latency_us / 1000);
        }
        println!("\n💰 Current Prices:");
        for (feed_id, price) in all_prices {
            println!("  {}: ${:.4}", &feed_id[..10], price);
        }
    }
    fn get_timestamp_micros() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_micros() as u64
    }
}

impl Default for PythWebSocketFeed {
    fn default() -> Self {
        Self::new(load_feed_ids())
    }
}

// To run connect: PythWebSocketFeed::new(load_feed_ids()).start().await
// To query, use .get_price(), .stats(), .display_stats() from your dashboard/main loop
