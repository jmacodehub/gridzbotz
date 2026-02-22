//! 💰 Arbitrage Trading Strategy
//!
//! ## What is Arbitrage?
//! Exploiting price differences across multiple DEXs for risk-free profit.
//!
//! ## How It Works:
//! 1. Monitor prices on multiple DEXs simultaneously
//! 2. Detect when price difference exceeds minimum threshold
//! 3. Calculate net profit after fees and slippage
//! 4. Execute simultaneous buy/sell when profitable
//!
//! ## Example:
//! ```text
//! Jupiter:  SOL = $150.00 (BUY)
//! Raydium:  SOL = $150.60 (SELL)
//!
//! Spread:   $0.60 (0.40%)
//! Fees:     -$0.15
//! Net:      $0.45 per SOL
//!
//! Signal: ARBITRAGE OPPORTUNITY! 💰
//! ```

use super::{Strategy, Signal, StrategyStats};
use async_trait::async_trait;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum spread threshold (%) to trigger arbitrage
const MIN_SPREAD_PERCENT: f64 = 0.25;

/// Estimated total fees (trading fees + gas)
const ESTIMATED_FEES_PERCENT: f64 = 0.30;

/// Minimum net profit threshold after fees
const MIN_NET_PROFIT_PERCENT: f64 = 0.10;

/// Maximum allowed slippage
const _MAX_SLIPPAGE_PERCENT: f64 = 0.50;

// ═══════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════

/// DEX identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DexId {
    Jupiter,
    Raydium,
    Orca,
    Phoenix,
    OpenBook,
}

impl DexId {
    fn name(&self) -> &str {
        match self {
            DexId::Jupiter => "Jupiter",
            DexId::Raydium => "Raydium",
            DexId::Orca => "Orca",
            DexId::Phoenix => "Phoenix",
            DexId::OpenBook => "OpenBook",
        }
    }
}

/// Price quote from a DEX
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DexQuote {
    pub dex: DexId,
    pub price: f64,
    pub liquidity: f64,
    pub timestamp: i64,
}

/// Arbitrage opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArbitrageOpportunity {
    pub buy_dex: DexId,
    pub sell_dex: DexId,
    pub buy_price: f64,
    pub sell_price: f64,
    pub spread_percent: f64,
    pub estimated_fees: f64,
    pub net_profit_percent: f64,
    pub confidence: f64,
}

impl ArbitrageOpportunity {
    /// Check if opportunity is profitable after fees
    fn is_profitable(&self) -> bool {
        self.net_profit_percent >= MIN_NET_PROFIT_PERCENT
    }

    /// Display opportunity
    fn display(&self) -> String {
        format!(
            "BUY {} @ ${:.2} | SELL {} @ ${:.2} | Spread: {:.2}% | Net: {:.2}%",
            self.buy_dex.name(),
            self.buy_price,
            self.sell_dex.name(),
            self.sell_price,
            self.spread_percent,
            self.net_profit_percent
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ARBITRAGE STRATEGY
// ═══════════════════════════════════════════════════════════════════════════

/// Arbitrage strategy - exploits cross-DEX price differences
pub struct ArbitrageStrategy {
    /// Strategy name
    name: String,

    /// Current price quotes from each DEX
    dex_prices: HashMap<DexId, DexQuote>,

    /// Current best opportunity (if any)
    best_opportunity: Option<ArbitrageOpportunity>,

    /// Historical opportunities (for analysis)
    opportunities_found: Vec<ArbitrageOpportunity>,

    /// Strategy statistics
    stats: StrategyStats,
}

impl ArbitrageStrategy {
    /// Create new arbitrage strategy
    pub fn new() -> Self {
        Self {
            name: "Arbitrage (Cross-DEX)".to_string(),
            dex_prices: HashMap::new(),
            best_opportunity: None,
            opportunities_found: Vec::new(),
            stats: StrategyStats::default(),
        }
    }

    /// Update price for a specific DEX
    pub fn update_dex_price(&mut self, dex: DexId, price: f64, liquidity: f64, timestamp: i64) {
        let quote = DexQuote {
            dex,
            price,
            liquidity,
            timestamp,
        };

        self.dex_prices.insert(dex, quote);
    }

    /// Find best arbitrage opportunity across all DEXs
    fn find_best_opportunity(&mut self) -> Option<ArbitrageOpportunity> {
        if self.dex_prices.len() < 2 {
            return None;
        }

        let mut best_opp: Option<ArbitrageOpportunity> = None;
        let mut best_net_profit = 0.0;

        // Compare all DEX pairs
        let dexes: Vec<_> = self.dex_prices.keys().copied().collect();

        for i in 0..dexes.len() {
            for j in 0..dexes.len() {
                if i == j {
                    continue;
                }

                let dex_a = dexes[i];
                let dex_b = dexes[j];

                if let (Some(quote_a), Some(quote_b)) =
                    (self.dex_prices.get(&dex_a), self.dex_prices.get(&dex_b))
                {
                    // Calculate spread (buy low, sell high)
                    if quote_a.price < quote_b.price {
                        let spread = quote_b.price - quote_a.price;
                        let spread_percent = (spread / quote_a.price) * 100.0;

                        // Check if spread exceeds minimum
                        if spread_percent >= MIN_SPREAD_PERCENT {
                            // Calculate net profit after fees
                            let estimated_fees = ESTIMATED_FEES_PERCENT;
                            let net_profit_percent = spread_percent - estimated_fees;

                            // Check if profitable
                            if net_profit_percent > best_net_profit {
                                let confidence = self.calculate_confidence(spread_percent, quote_a, quote_b);

                                best_opp = Some(ArbitrageOpportunity {
                                    buy_dex: dex_a,
                                    sell_dex: dex_b,
                                    buy_price: quote_a.price,
                                    sell_price: quote_b.price,
                                    spread_percent,
                                    estimated_fees,
                                    net_profit_percent,
                                    confidence,
                                });

                                best_net_profit = net_profit_percent;
                            }
                        }
                    }
                }
            }
        }

        // Store opportunity if found
        if let Some(ref opp) = best_opp {
            if opp.is_profitable() {
                self.opportunities_found.push(opp.clone());
            }
        }

        best_opp
    }

    /// Calculate confidence in arbitrage opportunity
    fn calculate_confidence(&self, spread_percent: f64, quote_a: &DexQuote, quote_b: &DexQuote) -> f64 {
        let mut confidence = 0.0;

        // Larger spread = higher confidence
        confidence += (spread_percent / 2.0).min(0.4);

        // More liquidity = higher confidence
        let min_liquidity = quote_a.liquidity.min(quote_b.liquidity);
        if min_liquidity > 100_000.0 {
            confidence += 0.3;
        } else if min_liquidity > 50_000.0 {
            confidence += 0.2;
        } else {
            confidence += 0.1;
        }

        // Recent prices = higher confidence
        let now = chrono::Utc::now().timestamp();
        let max_age = (now - quote_a.timestamp).max(now - quote_b.timestamp);
        if max_age < 5 {
            confidence += 0.3;
        } else if max_age < 10 {
            confidence += 0.2;
        } else {
            confidence += 0.1;
        }

        confidence.min(1.0)
    }

    /// Get statistics about opportunities
    pub fn opportunity_stats(&self) -> (usize, f64) {
        let count = self.opportunities_found.len();
        let avg_profit = if count > 0 {
            self.opportunities_found.iter()
                .map(|o| o.net_profit_percent)
                .sum::<f64>() / count as f64
        } else {
            0.0
        };

        (count, avg_profit)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Strategy for ArbitrageStrategy {
    fn name(&self) -> &str {
        &self.name
    }

    async fn analyze(&mut self, price: f64, timestamp: i64) -> Result<Signal> {
        // Update simulated DEX prices (in real implementation, fetch from APIs)
        // For demo, we simulate small price differences
        self.update_dex_price(DexId::Jupiter, price, 1_000_000.0, timestamp);
        self.update_dex_price(DexId::Raydium, price * 1.003, 800_000.0, timestamp); // 0.3% higher
        self.update_dex_price(DexId::Orca, price * 1.001, 500_000.0, timestamp);    // 0.1% higher

        // Find best arbitrage opportunity
        self.best_opportunity = self.find_best_opportunity();

        // Generate signal based on opportunity
        let signal = if let Some(ref opp) = self.best_opportunity {
            if opp.is_profitable() {
                self.stats.buy_signals += 1;
                Signal::StrongBuy {
                    price: opp.buy_price,
                    size: 1.0,
                    confidence: opp.confidence,
                    reason: format!(
                        "Arbitrage: {} | Net: {:.2}%",
                        opp.display(),
                        opp.net_profit_percent
                    ),
                }
            } else {
                self.stats.hold_signals += 1;
                Signal::Hold { reason: Some("Spread too small for profit".to_string()) }
            }
        } else {
            self.stats.hold_signals += 1;
            Signal::Hold { reason: Some("No arbitrage opportunity".to_string()) }
        };

        self.stats.signals_generated += 1;

        Ok(signal)
    }

    fn stats(&self) -> StrategyStats {
        self.stats.clone()
    }

    fn reset(&mut self) {
        self.dex_prices.clear();
        self.best_opportunity = None;
        self.opportunities_found.clear();
        self.stats = StrategyStats::default();
    }
}

impl Default for ArbitrageStrategy {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_arbitrage_creation() {
        let strategy = ArbitrageStrategy::new();
        assert_eq!(strategy.name(), "Arbitrage (Cross-DEX)");
    }

    #[tokio::test]
    async fn test_opportunity_detection() {
        let mut strategy = ArbitrageStrategy::new();

        // Manually add DEX prices with PROFITABLE spread
        let timestamp = chrono::Utc::now().timestamp();

        // Jupiter: $150.00
        strategy.update_dex_price(DexId::Jupiter, 150.0, 1_000_000.0, timestamp);

        // Raydium: $151.00 (0.67% spread - definitely profitable!)
        strategy.update_dex_price(DexId::Raydium, 151.00, 800_000.0, timestamp);

        // Find opportunities
        let opp = strategy.find_best_opportunity();

        // Should detect arbitrage opportunity
        assert!(opp.is_some(), "Should find arbitrage opportunity with 0.67% spread");

        let opportunity = opp.unwrap();
        assert_eq!(opportunity.buy_dex, DexId::Jupiter);
        assert_eq!(opportunity.sell_dex, DexId::Raydium);

        // With 0.67% spread and 0.3% fees, net should be 0.37% (profitable!)
        assert!(opportunity.net_profit_percent > 0.10,
                "Net profit {:.2}% should exceed 0.10%",
                opportunity.net_profit_percent);
        assert!(opportunity.is_profitable(), "Opportunity should be profitable");
    }

    #[tokio::test]
    async fn test_profitability_check() {
        let opp = ArbitrageOpportunity {
            buy_dex: DexId::Jupiter,
            sell_dex: DexId::Raydium,
            buy_price: 150.0,
            sell_price: 151.0,
            spread_percent: 0.67,
            estimated_fees: 0.30,
            net_profit_percent: 0.37,
            confidence: 0.8,
        };

        assert!(opp.is_profitable());
    }

    #[tokio::test]
    async fn test_no_opportunity_when_spread_too_small() {
        let mut strategy = ArbitrageStrategy::new();
        let timestamp = chrono::Utc::now().timestamp();

        // Very small spread (0.1% - not profitable)
        strategy.update_dex_price(DexId::Jupiter, 150.0, 1_000_000.0, timestamp);
        strategy.update_dex_price(DexId::Orca, 150.15, 500_000.0, timestamp);

        let opp = strategy.find_best_opportunity();

        // Should NOT find opportunity (spread too small)
        assert!(opp.is_none(), "Should not find opportunity with tiny spread");
    }
}
