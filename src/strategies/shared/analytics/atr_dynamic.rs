// src/strategies/shared/atr_dynamic.rs
// ═══════════════════════════════════════════════════════════════════════════
// ATR-DYNAMIC ANALYZER - SHARED MODULE V2.0
// ═══════════════════════════════════════════════════════════════════════════
//
// Purpose:
//   - Generic ATR-based analyzer usable by any strategy
//   - Provides dynamic spacing, volatility thresholds, or adaptive parameters
//
// Improvements in V2.0:
//   ✅ Modular configuration struct (no hardcoded values)
//   ✅ Supports both OHLC and fallback single-price updates
//   ✅ Works with shared config loader and async-compatible design
//   ✅ Clean API: calculate_spacing(), current_atr(), as_percent()
//   ✅ Logging-safe and backtest-friendly
//   ✅ Fully strategy-agnostic (usable in grid, momentum, or RSI)
//
// ═══════════════════════════════════════════════════════════════════════════

use log::{debug, info};
use std::collections::VecDeque;

/// ATR calculation configuration (used for initialization)
#[derive(Debug, Clone)]
pub struct ATRConfig {
    pub atr_period: usize,
    pub atr_multiplier: f64,
    pub min_spacing: f64,
    pub max_spacing: f64,
}

impl Default for ATRConfig {
    fn default() -> Self {
        Self {
            atr_period: 14,
            atr_multiplier: 3.0,
            min_spacing: 0.50,
            max_spacing: 5.0,
        }
    }
}

/// Core ATR calculator (mathematical foundation)
#[derive(Debug, Clone)]
pub struct ATRCalculator {
    period: usize,
    true_ranges: VecDeque<f64>,
    prev_close: Option<f64>,
    current_atr: Option<f64>,
}

impl ATRCalculator {
    pub fn new(period: usize) -> Self {
        Self {
            period,
            true_ranges: VecDeque::with_capacity(period),
            prev_close: None,
            current_atr: None,
        }
    }

    /// Accepts standard OHLC data.
    /// Returns Some(ATR) once enough samples collected.
    pub fn update_ohlc(&mut self, high: f64, low: f64, close: f64) -> Option<f64> {
        let tr = if let Some(prev) = self.prev_close {
            (high - low)
                .max((high - prev).abs())
                .max((low - prev).abs())
        } else {
            high - low
        };

        self.prev_close = Some(close);
        self.true_ranges.push_back(tr);
        if self.true_ranges.len() > self.period {
            self.true_ranges.pop_front();
        }

        if self.true_ranges.len() == self.period {
            let atr = self.true_ranges.iter().sum::<f64>() / self.period as f64;
            self.current_atr = Some(atr);
            debug!("ATR updated: {:.6}", atr);
            Some(atr)
        } else {
            None
        }
    }

    /// Convenience method: derive ATR from single price (synthetic OHLC)
    pub fn update_with_price(&mut self, price: f64) -> Option<f64> {
        let spread = price * 0.001;
        self.update_ohlc(price + spread, price - spread, price)
    }

    pub fn value(&self) -> Option<f64> {
        self.current_atr
    }

    pub fn as_percent(&self, current_price: f64) -> Option<f64> {
        self.current_atr.map(|atr| (atr / current_price) * 100.0)
    }

    pub fn is_ready(&self) -> bool {
        self.current_atr.is_some()
    }
}

/// ATR Dynamic Analyzer — shared across strategies
#[derive(Debug, Clone)]
pub struct ATRDynamic {
    atr_calc: ATRCalculator,
    pub atr_multiplier: f64,
    pub min_spacing: f64,
    pub max_spacing: f64,
    pub updates: u64,
}

impl ATRDynamic {
    /// Initialize directly from config struct
    pub fn from_config(cfg: &ATRConfig) -> Self {
        info!(
            "🚀 Initializing Shared ATRDynamic Analyzer (period={} multiplier={}× range={:.2}%-{:.2}%)",
            cfg.atr_period, cfg.atr_multiplier, cfg.min_spacing, cfg.max_spacing
        );
        Self {
            atr_calc: ATRCalculator::new(cfg.atr_period),
            atr_multiplier: cfg.atr_multiplier,
            min_spacing: cfg.min_spacing,
            max_spacing: cfg.max_spacing,
            updates: 0,
        }
    }

    /// Update internal ATR using raw price data or OHLC when available
    pub fn update(&mut self, price: f64) -> Option<f64> {
        self.updates += 1;
        self.atr_calc.update_with_price(price)
    }

    pub fn update_ohlc(&mut self, h: f64, l: f64, c: f64) -> Option<f64> {
        self.updates += 1;
        self.atr_calc.update_ohlc(h, l, c)
    }

    /// Dynamic spacing in %: ATR% × multiplier (clamped)
    pub fn calculate_spacing(&self, current_price: f64) -> Option<f64> {
        self.atr_calc.as_percent(current_price).map(|atr_pct| {
            let raw = atr_pct * self.atr_multiplier;
            let spacing = raw.max(self.min_spacing).min(self.max_spacing);
            debug!(
                "📈 ATRDynamic: ATR={:.4}% × {:.2}x = {:.4}% → clamped {:.4}%",
                atr_pct, self.atr_multiplier, raw, spacing
            );
            spacing
        })
    }

    pub fn atr_value(&self) -> Option<f64> {
        self.atr_calc.value()
    }

    pub fn atr_percent(&self, price: f64) -> Option<f64> {
        self.atr_calc.as_percent(price)
    }

    pub fn ready(&self) -> bool {
        self.atr_calc.is_ready()
    }

    pub fn updates(&self) -> u64 {
        self.updates
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Unit Tests
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atr_warmup_and_value() {
        let mut calc = ATRCalculator::new(3);
        assert!(calc.update_with_price(100.0).is_none());
        assert!(calc.update_with_price(101.0).is_none());
        assert!(calc.update_with_price(102.0).is_some());
        assert!(calc.value().unwrap() > 0.0);
    }

    #[test]
    fn test_dynamic_spacing_clamping() {
        let cfg = ATRConfig {
            atr_period: 3,
            atr_multiplier: 5.0,
            min_spacing: 1.0,
            max_spacing: 5.0,
        };
        let mut atr = ATRDynamic::from_config(&cfg);
        atr.update_ohlc(105.0, 100.0, 103.0);
        atr.update_ohlc(106.0, 102.0, 104.0);
        atr.update_ohlc(105.0, 101.0, 103.0);
        let spacing = atr.calculate_spacing(104.0).unwrap();
        assert!(spacing >= 1.0 && spacing <= 5.0);
    }

    #[test]
    fn test_percentage_conversion() {
        let mut calc = ATRCalculator::new(2);
        calc.update_ohlc(110.0, 100.0, 105.0);
        calc.update_ohlc(120.0, 108.0, 115.0);
        assert!(calc.as_percent(110.0).unwrap() > 0.5);
    }
}
