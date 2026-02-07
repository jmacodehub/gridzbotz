//! ═══════════════════════════════════════════════════════════════════════════
//! Feed Consensus Algorithms and Validation Utilities
//! ═══════════════════════════════════════════════════════════════════════════

use crate::trading::redundant_feed::{ConsensusPrice, FeedSource, PriceSource};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct ConsensusAlgorithm;

#[derive(Debug, Clone)]
pub struct MedianConsensus;

#[derive(Debug, Clone)]
pub struct PriceValidator;

impl MedianConsensus {
    pub fn calculate(prices: &[PriceSource]) -> ConsensusPrice {
        if prices.is_empty() {
            return ConsensusPrice {
                price: 150.0,
                sources: vec![FeedSource::Mock],
                timestamp: Utc::now(),
                confidence: 0.0,
                latency_ms: 0.0,
            };
        }

        let mut sorted: Vec<f64> = prices.iter().map(|p| p.price).collect();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if sorted.len() % 2 == 0 {
            (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
        } else {
            sorted[sorted.len() / 2]
        };

        ConsensusPrice {
            price: median,
            sources: prices.iter().map(|p| p.source).collect(),
            timestamp: Utc::now(),
            confidence: prices.len() as f64 / 3.0,
            latency_ms: prices
                .iter()
                .map(|p| p.latency_us as f64 / 1000.0)
                .sum::<f64>()
                / prices.len() as f64,
        }
    }
}
