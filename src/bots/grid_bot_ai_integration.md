# 🤖 GridBot AI Integration - Complete Wiring Documentation

## Overview

This document explains how the Adaptive Optimizer (AI) is fully integrated with GridBot through the modular config system.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    USER TOML CONFIG                         │
│  [adaptive_optimizer]                                       │
│  enabled = true                                              │
│  optimization_interval_cycles = 10                          │
│  low_drawdown_threshold = 2.0                               │
│  ...                                                         │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│              Config::from_file("v5.toml")                   │
│  Parses TOML → AdaptiveOptimizerConfig struct              │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                   GridBot::new(config)                      │
│  1. Reads config.adaptive_optimizer                         │
│  2. Creates: AdaptiveOptimizer::new_with_config(...)       │
│  3. Stores in self.adaptive_optimizer                       │
└──────────────────────┬──────────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────────┐
│         bot.process_price_update(price, timestamp)          │
│  Every N cycles (from config):                              │
│  1. optimizer.optimize(&enhanced_metrics)                   │
│  2. If changes: log new spacing & size                      │
│  3. place_grid_orders() uses optimizer.current_* values    │
└─────────────────────────────────────────────────────────────┘
```

## Key Files Modified

### 1. `src/trading/adaptive_optimizer.rs` (V2.0)

**Changes:**
- Added `new_with_config()` constructor
- All thresholds/multipliers now from config
- `enabled` flag respected throughout
- Backwards-compatible `new()` kept for tests

**Key Method:**
```rust
pub fn new_with_config(
    config: AdaptiveOptimizerConfig,  // User's AI settings
    base_spacing_percent: f64,         // From trading config
    base_position_size: f64,           // From trading config  
) -> Self
```

### 2. `src/bots/grid_bot.rs` (Updated)

**Changes:**
- Constructor reads `config.adaptive_optimizer`
- Passes to `AdaptiveOptimizer::new_with_config()`
- Uses `config.adaptive_optimizer.optimization_interval_cycles`
- Grid orders use `optimizer.current_spacing_percent` & `optimizer.current_position_size`

**Key Code:**
```rust
// In GridBot::new()
let adaptive_optimizer = if let Some(ai_config) = &config.adaptive_optimizer {
    AdaptiveOptimizer::new_with_config(
        ai_config.clone(),
        base_spacing,
        base_size,
    )
} else {
    AdaptiveOptimizer::new(base_spacing, base_size) // Fallback
};
```

### 3. `src/config/adaptive_optimizer.rs` (Exists)

**Defines:**
```rust
pub struct AdaptiveOptimizerConfig {
    pub enabled: bool,
    pub optimization_interval_cycles: u32,
    pub low_drawdown_threshold: f64,
    pub spacing_tighten_multiplier: f64,
    // ... 20+ configurable parameters
}
```

## Config Example (V5.0)

```toml
[adaptive_optimizer]
enabled = true
optimization_interval_cycles = 10

# Spacing AI
low_drawdown_threshold = 2.0
moderate_drawdown_threshold = 5.0
high_drawdown_threshold = 8.0
emergency_drawdown_threshold = 12.0

spacing_tighten_multiplier = 0.80
spacing_widen_multiplier = 1.30
spacing_emergency_multiplier = 1.80

# Position Sizing AI
high_efficiency_threshold = 0.70
low_efficiency_threshold = 0.30

size_high_efficiency_multiplier = 1.30
size_low_efficiency_multiplier = 0.70

# Streaks
win_streak_bonus_max = 1.50
loss_streak_penalty_max = 0.60
streak_threshold = 3

# Safety
min_spacing_absolute = 0.15
max_spacing_absolute = 0.50
min_position_absolute = 0.05
max_position_absolute = 0.30
```

## How It Works at Runtime

### Initialization (Once)
1. User edits `multi_strategy_v5_ai_ultimate.toml`
2. `Config::from_file()` parses `[adaptive_optimizer]` section
3. `GridBot::new()` receives full config
4. Creates optimizer with config: `AdaptiveOptimizer::new_with_config(...)`

### Every Trading Cycle
1. `bot.process_price_update(price, ts)` called
2. Updates `enhanced_metrics` (drawdown, efficiency, trades)
3. Every N cycles (from config):
   - Calls `optimizer.optimize(&metrics)`
   - Optimizer checks drawdown vs thresholds
   - Calculates new spacing/size using config multipliers
   - Clamps to config safety limits
   - Returns result with changes

### On Grid Reposition
1. `place_grid_orders()` called
2. Uses `optimizer.current_spacing_percent` (not static config!)
3. Uses `optimizer.current_position_size` (not static config!)
4. AI values applied to new orders

## Modular Benefits

✅ **Separation of Concerns:**
- Config = User intent
- Optimizer = Intelligence logic
- GridBot = Orchestration

✅ **Testability:**
- Can test optimizer with mock configs
- Can test GridBot with disabled AI
- Each module independent

✅ **Flexibility:**
- Disable AI entirely: `enabled = false`
- Tweak thresholds without code changes
- A/B test different AI strategies

✅ **Backwards Compatible:**
- Old configs work (uses defaults)
- Tests use `new()` (no config needed)
- Gradual migration path

## Future Enhancements

1. **Per-Strategy AI:** Different AI configs for grid vs momentum
2. **ML Models:** Load TensorFlow Lite models from config
3. **Live Tuning:** Update config via API without restart
4. **Audit Trail:** Log all AI decisions to database

---

**Version:** 2.0  
**Date:** February 11, 2026  
**Status:** ✅ COMPLETE INTEGRATION
