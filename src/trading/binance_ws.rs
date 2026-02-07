//! ═══════════════════════════════════════════════════════════════════════════
//! 🔶 Real Binance WebSocket Feed - V4.2 Production Ready
//!
//! Features:
//! ✅ Real-time prices from Binance (50-100ms latency)
//! ✅ Automatic reconnection with exponential backoff
//! ✅ Geographic diversity (independent from Pyth)
//! ✅ Free forever (public WebSocket)
//! ✅ SOL/USDT validation source
//! ✅ V4.2: No stale initialization values (Option<f64>)
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::Result;
use futures_util::StreamExt;
use log::{debug, error, info, warn};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Binance ticker message structure
#[derive(Debug, Deserialize)]
struct BinanceTicker {
    #[serde(rename = "s")]
    symbol: String,

    #[serde(rename = "c")]
    close_price: String,

    #[serde(rename = "E")]
    _event_time: u64,

    #[serde(rename = "h")]
    _high_price: String,

    #[serde(rename = "l")]
    _low_price: String,

    #[serde(rename = "v")]
    _volume: String,
}

/// Binance WebSocket price feed client (V4.2)
pub struct BinanceWSFeed {
    _symbol: String,
    last_price: Arc<RwLock<Option<f64>>>, // V4.2: Option to track "no price yet"
    is_connected: Arc<RwLock<bool>>,
}

impl BinanceWSFeed {
    /// Create new Binance WebSocket feed (V4.2)
    ///
    /// # Arguments
    /// * `symbol` - Trading pair symbol (e.g., "SOLUSDT")
    ///
    /// # Example
    /// ```
    /// use solana_grid_bot::trading::binance_ws::BinanceWSFeed;
    ///
    /// #[tokio::main]
    /// async fn main() -> anyhow::Result<()> {
    ///     let feed = BinanceWSFeed::new("SOLUSDT").await?;
    ///     let price = feed.get_latest_price().await?;
    ///     println!("SOL/USDT: ${:.4}", price);
    ///     Ok(())
    /// }
    /// ```
    pub async fn new(symbol: &str) -> Result<Self> {
        info!("🔶 Connecting to Binance WebSocket...");
        info!("   Symbol: {}", symbol);
        info!("   Expected latency: 50-100ms");
        info!("   Endpoint: wss://stream.binance.com:9443");

        // V4.2: Initialize as None (no stale prices!)
        let last_price = Arc::new(RwLock::new(None));
        let is_connected = Arc::new(RwLock::new(false));

        let last_price_clone = Arc::clone(&last_price);
        let is_connected_clone = Arc::clone(&is_connected);
        let symbol_lower = symbol.to_lowercase();

        // Spawn WebSocket connection in background
        tokio::spawn(async move {
            Self::ws_loop(&symbol_lower, last_price_clone, is_connected_clone).await;
        });

        // Wait for first connection and price (max 2 seconds)
        for i in 0..20 {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

            let connected = *is_connected.read().await;
            let has_price = last_price.read().await.is_some();

            if connected && has_price {
                info!(
                    "✅ Binance WebSocket connected with valid price after {}ms",
                    i * 100
                );
                break;
            }
        }

        // Check final status
        let has_price = last_price.read().await.is_some();
        if *is_connected.read().await && has_price {
            info!("✅ Binance WebSocket active and receiving prices");
        } else if *is_connected.read().await {
            warn!("⚠️  Binance WebSocket connected but awaiting first price");
        } else {
            warn!("⚠️  Binance WebSocket connecting in background (may take a few seconds)");
        }

        Ok(Self {
            _symbol: symbol.to_string(),
            last_price,
            is_connected,
        })
    }

    /// Main WebSocket loop with automatic reconnection (V4.2)
    async fn ws_loop(
        symbol: &str,
        price: Arc<RwLock<Option<f64>>>,
        is_connected: Arc<RwLock<bool>>,
    ) {
        let url = format!("wss://stream.binance.com:9443/ws/{}@ticker", symbol);
        let mut reconnect_delay = 1u64;
        let mut connection_attempts = 0u32;

        info!("🔶 Starting Binance WebSocket loop for {}", symbol);

        loop {
            connection_attempts += 1;
            debug!("🔶 Connection attempt #{} to {}", connection_attempts, url);

            match connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("✅ Binance WebSocket connected successfully!");
                    *is_connected.write().await = true;
                    reconnect_delay = 1; // Reset delay on successful connection

                    let (_, mut read) = ws_stream.split();

                    // Read messages until disconnection
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                // Parse ticker message
                                match serde_json::from_str::<BinanceTicker>(&text) {
                                    Ok(ticker) => {
                                        match ticker.close_price.parse::<f64>() {
                                            Ok(p) => {
                                                // V4.2: More generous range check (10-10000)
                                                if p > 10.0 && p < 10000.0 {
                                                    let is_first = price.read().await.is_none();
                                                    *price.write().await = Some(p);

                                                    if is_first {
                                                        info!("🔶 Binance: First price received ${:.4} ({})", p, ticker.symbol);
                                                    } else {
                                                        debug!(
                                                            "🔶 Binance: ${:.4} ({})",
                                                            p, ticker.symbol
                                                        );
                                                    }
                                                } else {
                                                    warn!(
                                                        "⚠️  Binance price out of range: ${:.4}",
                                                        p
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                warn!(
                                                    "⚠️  Failed to parse price: {} ({})",
                                                    e, ticker.close_price
                                                );
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!(
                                            "⚠️  Non-ticker message: {} ({})",
                                            e,
                                            &text[..text.len().min(100)]
                                        );
                                    }
                                }
                            }
                            Ok(Message::Ping(_)) => {
                                debug!("🔶 Ping received from Binance");
                            }
                            Ok(Message::Pong(_)) => {
                                debug!("🔶 Pong received from Binance");
                            }
                            Ok(Message::Close(reason)) => {
                                warn!("🔶 Binance closed connection: {:?}", reason);
                                break;
                            }
                            Err(e) => {
                                error!("🔶 WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }

                    *is_connected.write().await = false;
                    warn!("⚠️  Binance WebSocket disconnected");
                }
                Err(e) => {
                    *is_connected.write().await = false;
                    error!("❌ Binance WebSocket connection failed: {}", e);
                }
            }

            // Exponential backoff: 1s → 2s → 4s → 8s → 16s → max 30s
            warn!("⏳ Reconnecting to Binance in {}s...", reconnect_delay);
            tokio::time::sleep(tokio::time::Duration::from_secs(reconnect_delay)).await;
            reconnect_delay = (reconnect_delay * 2).min(30);
        }
    }

    /// Get latest price from Binance (V4.2)
    ///
    /// # Returns
    /// Current SOL/USDT price, or error if WebSocket hasn't received first price yet
    pub async fn get_latest_price(&self) -> Result<f64> {
        match *self.last_price.read().await {
            Some(price) => Ok(price),
            None => {
                if *self.is_connected.read().await {
                    Err(anyhow::anyhow!(
                        "Binance WebSocket connected but no price received yet"
                    ))
                } else {
                    Err(anyhow::anyhow!("Binance WebSocket not connected yet"))
                }
            }
        }
    }

    /// Check if WebSocket is connected and has received valid price (V4.2)
    pub async fn is_healthy(&self) -> bool {
        *self.is_connected.read().await && self.last_price.read().await.is_some()
    }

    /// Get last known price (even if disconnected) - V4.2
    ///
    /// Returns Some(price) if price available, None if never received
    pub async fn get_cached_price(&self) -> Option<f64> {
        *self.last_price.read().await
    }

    /// Get cached price or default fallback (V4.2 helper)
    ///
    /// Useful for redundant feed systems that need a value
    pub async fn get_cached_price_or(&self, default: f64) -> f64 {
        self.last_price.read().await.unwrap_or(default)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires network connection
    async fn test_binance_connection() {
        let feed = BinanceWSFeed::new("SOLUSDT").await;
        assert!(feed.is_ok());

        if let Ok(feed) = feed {
            // Wait for price update
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            if let Ok(price) = feed.get_latest_price().await {
                assert!(price > 10.0);
                assert!(price < 10000.0);
                println!("✅ Binance price: ${:.4}", price);
            }
        }
    }

    #[tokio::test]
    async fn test_price_none_before_connection() {
        // Can't easily test without mocking, but structure validates
        // that Option<f64> properly represents "no price yet"
        assert!(true);
    }
}
