// ═══════════════════════════════════════════════════════════════════════════
// SHARED SIGNALS MODULE - PROJECT FLASH V5+ (Phase 3/4 Modular Edition)
// ═══════════════════════════════════════════════════════════════════════════

use async_trait::async_trait;

// ═══════════════════════════════════════════════════════════════════════════
// CORE TRAIT - SIGNAL MODULE INTERFACE
// ═══════════════════════════════════════════════════════════════════════════

/// Core trait for all signal modules
#[async_trait]
pub trait SignalModule: Send + Sync {
    fn name(&self) -> &str;
    async fn compute(&mut self, price: f64) -> f64;
    fn last_value(&self) -> Option<f64>;
    fn reset(&mut self);
}

// ═══════════════════════════════════════════════════════════════════════════
// SIGNAL MODULE DECLARATIONS
// ═══════════════════════════════════════════════════════════════════════════

pub mod mean_signal;
pub mod momentum_signal;
pub mod rsi_signal;

// ═══════════════════════════════════════════════════════════════════════════
// PUBLIC RE-EXPORTS - V5+ CLEAN IMPORT LAYER
// ═══════════════════════════════════════════════════════════════════════════

// ✅ Re-export signal structs
pub use mean_signal::MeanSignal;
pub use momentum_signal::MomentumSignal;
pub use rsi_signal::RsiSignal;

// ✅ NOTE: No need to re-export SignalModule - it's already public in this module!
// External code can import it as:
//   use crate::strategies::shared::signals::SignalModule;

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::runtime::Runtime;

    #[test]
    fn test_signal_trait_basics() {
        struct DummySignal {
            last: Option<f64>,
        }

        #[async_trait]
        impl SignalModule for DummySignal {
            fn name(&self) -> &str {
                "Dummy"
            }

            async fn compute(&mut self, price: f64) -> f64 {
                self.last = Some(price);
                price
            }

            fn last_value(&self) -> Option<f64> {
                self.last
            }

            fn reset(&mut self) {
                self.last = None;
            }
        }

        let rt = Runtime::new().unwrap();
        rt.block_on(async {
            let mut s = DummySignal { last: None };
            let out = s.compute(100.0).await;
            assert_eq!(out, 100.0);
            assert_eq!(s.last_value().unwrap(), 100.0);

            s.reset();
            assert!(s.last_value().is_none());
        });
    }

    #[tokio::test]
    async fn test_all_signals_exist() {
        // Just verify we can create instances
        let _rsi = RsiSignal::default();
        let _mom = MomentumSignal::default();
        let _mean = MeanSignal::default();
    }
}
