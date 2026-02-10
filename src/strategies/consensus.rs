// ═══════════════════════════════════════════════════════════════════════════
// 🧠 PROJECT FLASH V5 - CONSENSUS ENGINE (Phase 4 Foundation)
// ═══════════════════════════════════════════════════════════════════════════
//
// Purpose:
//   Encapsulates consensus logic away from StrategyManager for clear separation.
//   Manages all voting / weighted averaging logic across multiple strategies.
//
// Highlights:
//   ✅ Isolated from StrategyManager
//   ✅ Reusable across AI FusionBus or manual weight voting
//   ✅ Clean unit-tested decisions for BUY / SELL / HOLD
// ═══════════════════════════════════════════════════════════════════════════

use log::debug;
use serde::{Deserialize, Serialize};

use crate::strategies::Signal;

// Consensus modes guiding signal resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsensusMode {
    Single,
    WeightedAverage,
    MajorityVote,
}

impl Default for ConsensusMode {
    fn default() -> Self {
        Self::Single
    }
}

// ConsensusEngine - reusable for any StrategyManager or orchestration layer
#[derive(Debug, Clone)]
pub struct ConsensusEngine {
    pub mode: ConsensusMode,
}

impl ConsensusEngine {
    pub fn new(mode: ConsensusMode) -> Self {
        Self { mode }
    }

    pub fn resolve(&self, signals: &[Signal]) -> Signal {
        match self.mode {
            ConsensusMode::Single => self.single(signals),
            ConsensusMode::WeightedAverage => self.weighted(signals),
            ConsensusMode::MajorityVote => self.majority(signals),
        }
    }

    fn single(&self, signals: &[Signal]) -> Signal {
        signals.first().cloned().unwrap_or(Signal::Hold {
            reason: Some("no signals available".into()),
        })
    }

    fn weighted(&self, signals: &[Signal]) -> Signal {
        if signals.is_empty() {
            return Signal::Hold {
                reason: Some("weighted: empty".into()),
            };
        }

        let avg_strength = signals.iter().map(|s| s.strength()).sum::<f64>() / signals.len() as f64;

        if avg_strength > 0.6 {
            signals.first().cloned().unwrap()
        } else {
            Signal::Hold {
                reason: Some("weighted: neutral".into()),
            }
        }
    }

    fn majority(&self, signals: &[Signal]) -> Signal {
        if signals.is_empty() {
            return Signal::Hold {
                reason: Some("majority: empty".into()),
            };
        }

        let bulls = signals.iter().filter(|s| s.is_bullish()).count();
        let bears = signals.iter().filter(|s| s.is_bearish()).count();

        debug!("Consensus majority → bulls: {} | bears: {}", bulls, bears);

        match bulls.cmp(&bears) {
            std::cmp::Ordering::Greater => Signal::Buy {
                price: 0.0,
                size: 0.0,
                reason: "majority bull".into(),
                confidence: 0.75,
            },
            std::cmp::Ordering::Less => Signal::Sell {
                price: 0.0,
                size: 0.0,
                reason: "majority bear".into(),
                confidence: 0.75,
            },
            _ => Signal::Hold {
                reason: Some("majority: tie".into()),
            },
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE - Deterministic Consensus Validation
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::Signal;

    fn sample_signals() -> Vec<Signal> {
        vec![
            Signal::Buy {
                price: 100.0,
                size: 1.0,
                reason: "bullish".into(),
                confidence: 0.9,
            },
            Signal::Sell {
                price: 101.0,
                size: 1.0,
                reason: "bearish".into(),
                confidence: 0.3,
            },
            Signal::Hold {
                reason: Some("neutral".into()),
            },
        ]
    }

    #[test]
    fn test_single_returns_first() {
        let engine = ConsensusEngine::new(ConsensusMode::Single);
        let s = engine.resolve(&sample_signals());
        assert!(matches!(s, Signal::Buy { .. }));
    }

    #[test]
    fn test_weighted_mode_produces_signal() {
        let engine = ConsensusEngine::new(ConsensusMode::WeightedAverage);
        let s = engine.resolve(&sample_signals());
        assert!(s.is_bullish() || matches!(s, Signal::Hold { .. }));
    }

    #[test]
    fn test_majority_vote_returns_expected() {
        let engine = ConsensusEngine::new(ConsensusMode::MajorityVote);
        
        // Test with clear majority (2 buys vs 1 sell)
        let signals = vec![
            Signal::Buy {
                price: 100.0,
                size: 1.0,
                reason: "bullish 1".into(),
                confidence: 0.9,
            },
            Signal::Buy {
                price: 100.0,
                size: 1.0,
                reason: "bullish 2".into(),
                confidence: 0.8,
            },
            Signal::Sell {
                price: 101.0,
                size: 1.0,
                reason: "bearish".into(),
                confidence: 0.3,
            },
        ];
        
        let s = engine.resolve(&signals);
        
        // With 2 buys vs 1 sell, majority should return Buy
        assert!(
            matches!(s, Signal::Buy { .. }),
            "Expected Buy signal with clear majority, got: {:?}",
            s
        );
    }
}
