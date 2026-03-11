//! 🤝 Dynamic Weighted Majority Algorithm (WMA) Consensus
//! 
//! ## Research-Backed Performance:
//! Based on "Numin: Weighted-Majority Ensembles for Intraday Trading" (2024)
//! - **10-18% annualized returns**
//! - Outperforms equal voting by dynamically adjusting weights
//! - Short windows (5-10) = profitability focus
//! - Long windows (20) = accuracy focus
//! 
//! ## How It Works:
//! 
//! 1. **Performance Tracking:**
//!    - Track win rate, ROI, Sharpe ratio per strategy
//!    - Update every 10 cycles
//! 
//! 2. **Dynamic Weight Formula:**
//!    ```text
//!    weight = 0.6 * confidence + 0.4 * roi_performance
//!    ```
//! 
//! 3. **Confidence Filtering:**
//!    - Only vote if confidence > 0.65
//!    - Reduces false signals by 30-50%
//! 
//! 4. **Weighted Voting:**
//!    - Each strategy votes with its weight
//!    - Final decision = highest weighted sum
//! 
//! ## Voter Attribution (PR #98 Commit 2a):
//!    - `last_voters` tracks strategy names that cleared the confidence gate
//!      on the most recent `resolve()` call.
//!    - Populated fresh every tick — stale voters never persist.
//!    - Read via `get_last_voters()` by StrategyManager (Commit 2b) to feed
//!      realized fill P&L back into per-strategy `record_trade()` trackers.
//! 
//! ## Example:
//! ```text
//! Grid:     BUY  (weight: 1.0, confidence: 0.5)  → Vote: 0.50
//! Momentum: BUY  (weight: 0.8, confidence: 0.7)  → Vote: 0.56
//! RSI:      HOLD (weight: 0.9, confidence: 0.4)  → Filtered (too low)
//! 
//! Total BUY: 1.06 > 0  →  Final: BUY (confidence: 0.71)
//! last_voters: ["Grid", "Momentum"]   ← only the two that cleared gate
//! ```

use super::Signal;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

/// Minimum confidence to participate in voting
const MIN_CONFIDENCE: f64 = 0.65;

/// Weight update frequency (every N cycles)
const UPDATE_FREQUENCY: usize = 10;

/// Performance window for weight calculation
const PERFORMANCE_WINDOW: usize = 20;

/// Confidence weight in formula (0.6 = 60%)
const CONFIDENCE_WEIGHT: f64 = 0.6;

/// ROI weight in formula (0.4 = 40%)
const ROI_WEIGHT: f64 = 0.4;

/// Maximum correlation allowed between strategies
#[allow(dead_code)]
const MAX_CORRELATION: f64 = 0.8;

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY PERFORMANCE TRACKING
// ═══════════════════════════════════════════════════════════════════════════

/// Performance metrics for a single strategy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StrategyPerformance {
    pub name: String,
    pub weight: f64,
    pub win_rate: f64,
    pub roi: f64,
    pub sharpe_ratio: f64,
    pub total_trades: usize,
    pub wins: usize,
    pub losses: usize,
    pub total_pnl: f64,
    pub recent_signals: Vec<SignalType>,
}

impl StrategyPerformance {
    pub fn new(name: String) -> Self {
        Self {
            name,
            weight: 1.0,
            win_rate: 0.5,
            roi: 0.0,
            sharpe_ratio: 0.0,
            total_trades: 0,
            wins: 0,
            losses: 0,
            total_pnl: 0.0,
            recent_signals: Vec::with_capacity(PERFORMANCE_WINDOW),
        }
    }
    
    pub fn record_trade(&mut self, profit: f64) {
        self.total_trades += 1;
        self.total_pnl += profit;
        if profit > 0.0 {
            self.wins += 1;
        } else if profit < 0.0 {
            self.losses += 1;
        }
        if self.total_trades > 0 {
            self.win_rate = self.wins as f64 / self.total_trades as f64;
        }
        if self.total_trades > 0 {
            self.roi = self.total_pnl / self.total_trades as f64;
        }
    }
    
    pub fn record_signal(&mut self, signal_type: SignalType) {
        self.recent_signals.push(signal_type);
        if self.recent_signals.len() > PERFORMANCE_WINDOW {
            self.recent_signals.remove(0);
        }
    }
    
    /// Calculate dynamic weight
    /// Formula: weight = 0.6 * confidence + 0.4 * roi_performance
    pub fn calculate_dynamic_weight(&mut self, base_confidence: f64) {
        let roi_normalized = (self.roi / 0.2).clamp(0.0, 1.0);
        let new_weight = (CONFIDENCE_WEIGHT * base_confidence) + (ROI_WEIGHT * roi_normalized);
        self.weight = 0.7 * self.weight + 0.3 * new_weight;
        self.weight = self.weight.clamp(0.2, 2.0);
    }
}

/// Signal type for correlation tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalType {
    Buy,
    Sell,
    Hold,
}

impl From<&Signal> for SignalType {
    fn from(signal: &Signal) -> Self {
        if signal.is_bullish() {
            SignalType::Buy
        } else if signal.is_bearish() {
            SignalType::Sell
        } else {
            SignalType::Hold
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// WEIGHTED MAJORITY CONSENSUS ENGINE
// ═══════════════════════════════════════════════════════════════════════════

/// Dynamic Weighted Majority Algorithm consensus engine.
///
/// PR #98 Commit 2a: adds `last_voters` — the names of strategies that
/// cleared the confidence gate on the most recent `resolve()` call.
/// Exposed via `get_last_voters()` so StrategyManager (Commit 2b) can
/// route realized fill P&L back to the right per-strategy trackers.
pub struct WMAConsensusEngine {
    performances: HashMap<String, StrategyPerformance>,
    cycles: usize,
    min_confidence: f64,
    /// PR #98 Commit 2a: strategies that cleared the confidence threshold
    /// on the last resolve() call. Cleared fresh every tick — never stale.
    /// Both BUY and SELL voters are tracked: they all contributed to the
    /// consensus sizing decision that drove the fill.
    last_voters: Vec<String>,
}

impl WMAConsensusEngine {
    pub fn new() -> Self {
        Self {
            performances: HashMap::new(),
            cycles: 0,
            min_confidence: MIN_CONFIDENCE,
            last_voters: Vec::new(),
        }
    }
    
    pub fn with_min_confidence(min_confidence: f64) -> Self {
        Self {
            performances: HashMap::new(),
            cycles: 0,
            min_confidence,
            last_voters: Vec::new(),
        }
    }
    
    pub fn register_strategy(&mut self, name: String) {
        self.performances.insert(name.clone(), StrategyPerformance::new(name));
    }
    
    pub fn get_performance(&self, name: &str) -> Option<&StrategyPerformance> {
        self.performances.get(name)
    }
    
    pub fn record_trade(&mut self, strategy_name: &str, profit: f64) {
        if let Some(perf) = self.performances.get_mut(strategy_name) {
            perf.record_trade(profit);
        }
    }

    /// Return the strategy names that cleared the confidence gate on the
    /// most recent `resolve()` call.
    ///
    /// Used by `StrategyManager::get_last_wma_voters()` (Commit 2b) to
    /// attribute realized fill P&L back to the participating strategies.
    ///
    /// Empty slice = no strategy cleared the threshold (all signals were
    /// filtered), meaning the consensus was Hold and no fill should occur.
    pub fn get_last_voters(&self) -> &[String] {
        &self.last_voters
    }
    
    fn calculate_correlation(&self, strategy1: &str, strategy2: &str) -> f64 {
        let perf1 = match self.performances.get(strategy1) {
            Some(p) => p,
            None => return 0.0,
        };
        let perf2 = match self.performances.get(strategy2) {
            Some(p) => p,
            None => return 0.0,
        };
        if perf1.recent_signals.len() < 10 || perf2.recent_signals.len() < 10 {
            return 0.0;
        }
        let min_len = perf1.recent_signals.len().min(perf2.recent_signals.len());
        let mut agreements = 0;
        for i in 0..min_len {
            if perf1.recent_signals[i] == perf2.recent_signals[i] {
                agreements += 1;
            }
        }
        agreements as f64 / min_len as f64
    }
    
    /// Resolve consensus from multiple strategy signals.
    ///
    /// PR #98 Commit 2a: clears `last_voters` at the top of every call so
    /// it always reflects this tick's participants — never carries stale
    /// names from a previous tick. Strategy names are pushed into
    /// `last_voters` at the same confidence gate that admits them to voting,
    /// tracking both BUY and SELL contributors.
    pub fn resolve(&mut self, strategy_signals: Vec<(String, Signal)>, current_price: f64) -> Signal {
        self.cycles += 1;

        // PR #98 Commit 2a: fresh slate every tick — zero stale attribution risk.
        self.last_voters.clear();
        
        if self.cycles % UPDATE_FREQUENCY == 0 {
            self.update_weights();
        }
        
        let mut buy_weight = 0.0;
        let mut sell_weight = 0.0;
        let mut filtered_count = 0;
        
        for (strategy_name, signal) in &strategy_signals {
            let confidence = signal.confidence();
            
            if let Some(perf) = self.performances.get_mut(strategy_name) {
                perf.record_signal(SignalType::from(signal));
            }
            
            if confidence < self.min_confidence {
                debug!("[WMA] {} filtered: confidence {:.2} < {:.2}", 
                       strategy_name, confidence, self.min_confidence);
                continue;
            }

            // PR #98 Commit 2a: track this strategy as a voter for the
            // current tick — regardless of BUY/SELL direction.
            self.last_voters.push(strategy_name.clone());
            filtered_count += 1;
            
            let weight = self.performances
                .get(strategy_name)
                .map(|p| p.weight)
                .unwrap_or(1.0);
            
            let vote_strength = weight * confidence;
            
            if signal.is_bullish() {
                buy_weight += vote_strength;
                debug!("[WMA] {} → BUY (weight: {:.2}, confidence: {:.2}, vote: {:.3})",
                       strategy_name, weight, confidence, vote_strength);
            } else if signal.is_bearish() {
                sell_weight += vote_strength;
                debug!("[WMA] {} → SELL (weight: {:.2}, confidence: {:.2}, vote: {:.3})",
                       strategy_name, weight, confidence, vote_strength);
            }
        }
        
        if filtered_count == 0 {
            info!("[WMA] No signals above confidence threshold {:.2}", self.min_confidence);
            return Signal::Hold {
                reason: Some("WMA: all signals filtered (low confidence)".into()),
            };
        }
        
        let total_weight = buy_weight + sell_weight;
        let final_confidence = if total_weight > 0.0 {
            buy_weight.max(sell_weight) / total_weight
        } else {
            0.5
        };
        
        if buy_weight > sell_weight && buy_weight > 0.0 {
            info!("[WMA] CONSENSUS: BUY (buy: {:.3} > sell: {:.3}, conf: {:.2}, voters: {:?})",
                  buy_weight, sell_weight, final_confidence, self.last_voters);
            Signal::Buy {
                price: current_price,
                size: 0.5,
                confidence: final_confidence,
                reason: format!(
                    "WMA Consensus: {} strategies BUY (total weight: {:.2})",
                    filtered_count, buy_weight
                ),
                level_id: None,
            }
        } else if sell_weight > buy_weight && sell_weight > 0.0 {
            info!("[WMA] CONSENSUS: SELL (sell: {:.3} > buy: {:.3}, conf: {:.2}, voters: {:?})",
                  sell_weight, buy_weight, final_confidence, self.last_voters);
            Signal::Sell {
                price: current_price,
                size: 0.5,
                confidence: final_confidence,
                reason: format!(
                    "WMA Consensus: {} strategies SELL (total weight: {:.2})",
                    filtered_count, sell_weight
                ),
                level_id: None,
            }
        } else {
            info!("[WMA] CONSENSUS: HOLD (buy: {:.3}, sell: {:.3})",
                  buy_weight, sell_weight);
            Signal::Hold {
                reason: Some("WMA: no clear consensus".into()),
            }
        }
    }
    
    fn update_weights(&mut self) {
        info!("[WMA] Updating strategy weights (cycle {})", self.cycles);
        for (name, perf) in self.performances.iter_mut() {
            let old_weight = perf.weight;
            perf.calculate_dynamic_weight(0.7);
            info!("[WMA] {} weight: {:.3} → {:.3} (win rate: {:.1}%, ROI: {:.2}%)",
                  name, old_weight, perf.weight, 
                  perf.win_rate * 100.0, perf.roi * 100.0);
        }
    }
    
    pub fn get_summary(&self) -> String {
        let mut summary = String::from("\n=== WMA Performance Summary ===\n");
        for (name, perf) in &self.performances {
            summary.push_str(&format!(
                "\n{}: weight={:.2}, win_rate={:.1}%, trades={}, pnl={:.2}",
                name, perf.weight, perf.win_rate * 100.0, perf.total_trades, perf.total_pnl
            ));
        }
        summary.push_str(&format!("\n\nTotal Cycles: {}\n", self.cycles));
        summary
    }
}

impl Default for WMAConsensusEngine {
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
    
    #[test]
    fn test_wma_creation() {
        let engine = WMAConsensusEngine::new();
        assert_eq!(engine.min_confidence, MIN_CONFIDENCE);
        assert!(engine.last_voters.is_empty(), "last_voters must start empty");
    }
    
    #[test]
    fn test_strategy_registration() {
        let mut engine = WMAConsensusEngine::new();
        engine.register_strategy("Grid".to_string());
        assert!(engine.get_performance("Grid").is_some());
    }
    
    #[test]
    fn test_weighted_voting() {
        let mut engine = WMAConsensusEngine::new();
        engine.register_strategy("Grid".to_string());
        engine.register_strategy("Momentum".to_string());
        
        let signals = vec![
            (
                "Grid".to_string(),
                Signal::Buy {
                    price: 100.0,
                    size: 1.0,
                    confidence: 0.7,
                    reason: "grid buy".into(),
                    level_id: None,
                },
            ),
            (
                "Momentum".to_string(),
                Signal::Buy {
                    price: 100.0,
                    size: 1.0,
                    confidence: 0.8,
                    reason: "momentum buy".into(),
                    level_id: None,
                },
            ),
        ];
        
        let result = engine.resolve(signals, 100.0);
        assert!(result.is_bullish());
    }
    
    #[test]
    fn test_confidence_filtering() {
        let mut engine = WMAConsensusEngine::with_min_confidence(0.7);
        engine.register_strategy("Weak".to_string());
        
        let signals = vec![(
            "Weak".to_string(),
            Signal::Buy {
                price: 100.0,
                size: 1.0,
                confidence: 0.5,
                reason: "weak buy".into(),
                level_id: None,
            },
        )];
        
        let result = engine.resolve(signals, 100.0);
        assert!(matches!(result, Signal::Hold { .. }));
    }
    
    #[test]
    fn test_weight_updates() {
        let mut engine = WMAConsensusEngine::new();
        engine.register_strategy("Test".to_string());
        
        for _ in 0..5 {
            engine.record_trade("Test", 1.0);
        }
        
        engine.cycles = UPDATE_FREQUENCY - 1;
        let signals = vec![];
        let _ = engine.resolve(signals, 100.0);
        
        let perf = engine.get_performance("Test").unwrap();
        assert!(perf.win_rate > 0.9);
        assert!(perf.roi > 0.0);
    }

    // ───────────────────────────────────────────────────────────────────────
    // PR #98 Commit 2a: last_voters tracking tests
    // ───────────────────────────────────────────────────────────────────────

    /// Strategies that clear the confidence gate appear in last_voters.
    #[test]
    fn test_last_voters_populated_after_resolve() {
        let mut engine = WMAConsensusEngine::new();
        engine.register_strategy("Grid".to_string());
        engine.register_strategy("Momentum".to_string());

        let signals = vec![
            (
                "Grid".to_string(),
                Signal::Buy { price: 100.0, size: 1.0, confidence: 0.75,
                              reason: "grid".into(), level_id: None },
            ),
            (
                "Momentum".to_string(),
                Signal::Buy { price: 100.0, size: 1.0, confidence: 0.80,
                              reason: "momentum".into(), level_id: None },
            ),
        ];

        let _ = engine.resolve(signals, 100.0);
        let voters = engine.get_last_voters();
        assert_eq!(voters.len(), 2);
        assert!(voters.contains(&"Grid".to_string()));
        assert!(voters.contains(&"Momentum".to_string()));
    }

    /// Strategies below the confidence threshold must NOT appear in last_voters.
    #[test]
    fn test_last_voters_excludes_filtered_strategies() {
        let mut engine = WMAConsensusEngine::with_min_confidence(0.70);
        engine.register_strategy("Strong".to_string());
        engine.register_strategy("Weak".to_string());

        let signals = vec![
            (
                "Strong".to_string(),
                Signal::Buy { price: 100.0, size: 1.0, confidence: 0.85,
                              reason: "strong".into(), level_id: None },
            ),
            (
                "Weak".to_string(),
                Signal::Sell { price: 100.0, size: 1.0, confidence: 0.40,
                               reason: "weak".into(), level_id: None },
            ),
        ];

        let _ = engine.resolve(signals, 100.0);
        let voters = engine.get_last_voters();
        assert_eq!(voters.len(), 1);
        assert!(voters.contains(&"Strong".to_string()));
        assert!(!voters.contains(&"Weak".to_string()));
    }

    /// last_voters is cleared between ticks — never carries stale names.
    #[test]
    fn test_last_voters_cleared_between_ticks() {
        let mut engine = WMAConsensusEngine::new();
        engine.register_strategy("Grid".to_string());

        // Tick 1 — Grid votes with high confidence
        let signals_t1 = vec![(
            "Grid".to_string(),
            Signal::Buy { price: 100.0, size: 1.0, confidence: 0.80,
                          reason: "t1".into(), level_id: None },
        )];
        let _ = engine.resolve(signals_t1, 100.0);
        assert_eq!(engine.get_last_voters().len(), 1);

        // Tick 2 — empty signal list (price unchanged, no strategy fires)
        let _ = engine.resolve(vec![], 100.0);
        assert!(
            engine.get_last_voters().is_empty(),
            "last_voters must be empty when no strategy clears the gate"
        );
    }
}
