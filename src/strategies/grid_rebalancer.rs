//! Grid Rebalancing Strategy — Mean-Reversion + ATR Sizing
//!
//! V3.8 MODULAR STRATEGY:
//! ✅ Independent strategy module — not directly coupled to GridState or GridBot
//! ✅ Integrates with GridBot via process_price_update() in grid_bot.rs
//! ✅ GridState passed by mutable reference — strategy never constructs or owns the grid
//! ✅ All strategy behavior via public functions with &mut GridState + config params
//!
//! HOW GridRebalancer WORKS WITH GridState:
//! 1. GridBot creates GridState (src/bots/grid_state.rs) during initialization
//! 2. GridBot process_price_update() calls GridRebalancer::update_grid_positions()
//! 3. GridRebalancer mutates GridState via &mut reference (no construction, no ownership)
//! 4. GridRebalancer::should_reposition() reads GridState fields to make decisions
//! 5. GridRebalancer::reposition_grid() applies all changes via GridState's public methods
//! 6. GridState's place_grid()/update_levels() handle all volatility-adjusted positioning
//!
//! Flow: GridBot → GridRebalancer::update_grid_positions() → GridState modifications
//!
//! ARCHITECTURE:
//!   - GridState (grid_state.rs) owns all grid data + volatility management
//!   - GridRebalancer (this file) provides the decision logic + applies updates
//!   - GridBot (grid_bot.rs) orchestrates the entire flow
//!
//! Strategy logic:
//!   1. Calculates mid-price from best bid/ask
//!   2. Buys when price crosses DOWN through grid levels (mean reversion)
//!   3. Sells when price crosses UP through grid levels
//!   4. Position sizing scales with ATR (Average True Range volatility)
//!   5. Grid spacing adapts dynamically based on short-term volatility
//!
//! Created: Feb 2026 | Updated: Feb 2026 (V3.8 Modular Architecture)

use crate::bots::grid_state::GridState;
use crate::config::Config;
use crate::trading::{TradingEngine, OrderSide};
use anyhow::Result;
use log::{debug, info, warn};

/// Grid Rebalancing Strategy implementation.
/// All methods are stateless — strategy doesn't own GridState, only modifies it.
pub struct GridRebalancer;

impl GridRebalancer {
    /// Update grid positions and execute fills based on price crossings.
    /// Called from GridBot::process_price_update() every tick.
    ///
    /// Logic:
    /// 1. Check if grid repositioning is needed (volatility change or price drift)
    /// 2. If yes, reposition_grid() — grid spacing and levels adjusted via GridState
    /// 3. Detect level crossings (price moving through grid levels)
    /// 4. Execute trades for crossed levels via TradingEngine
    /// 5. Update GridState to mark levels as filled
    ///
    /// GridState mutation happens entirely via its public methods —
    /// GridRebalancer never directly modifies internal state fields.
    pub async fn update_grid_positions(
        grid: &mut GridState,
        config: &Config,
        trading_engine: &dyn TradingEngine,
        current_price: f64,
        timestamp: i64,
    ) -> Result<()> {
        // Step 1: Check if repositioning is needed
        let should_rebalance = Self::should_reposition(grid, current_price, config);

        if should_rebalance {
            info!(
                "[GridRebalancer] Repositioning grid at price ${:.4} (reason: {})",
                current_price,
                Self::reposition_reason(grid, current_price, config)
            );
            Self::reposition_grid(grid, config, current_price, timestamp).await?;
        }

        // Step 2: Check for level crossings and execute trades
        let crossings = grid.detect_level_crossings(current_price);

        for crossing in crossings {
            let side = if crossing.direction == "down" {
                OrderSide::Buy
            } else {
                OrderSide::Sell
            };

            let quantity = Self::calculate_position_size(
                config,
                grid.get_volatility(),
                current_price,
            );

            // Execute via TradingEngine
            match trading_engine
                .execute_order(side, crossing.level_price, quantity, timestamp)
                .await
            {
                Ok(fill_price) => {
                    info!(
                        "[GridRebalancer] {:?} fill @ ${:.4} | level {} | qty {:.4}",
                        side, fill_price, crossing.level_index, quantity
                    );
                    grid.mark_level_filled(crossing.level_index, fill_price, side)?;
                }
                Err(e) => {
                    warn!(
                        "[GridRebalancer] {:?} order failed at level {}: {}",
                        side, crossing.level_index, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Determine if grid repositioning is needed.
    ///
    /// Triggers:
    /// 1. Significant volatility change (ATR shift > 20%)
    /// 2. Price drifted beyond grid bounds
    /// 3. Minimum time since last reposition (avoid thrashing)
    fn should_reposition(grid: &GridState, current_price: f64, config: &Config) -> bool {
        let time_since_last = grid.time_since_last_reposition();
        let min_reposition_interval = config.strategy.min_reposition_interval_seconds;

        if time_since_last < min_reposition_interval {
            return false; // Too soon
        }

        // Check 1: Price out of bounds?
        let (lower, upper) = grid.get_grid_bounds();
        if current_price < lower || current_price > upper {
            debug!(
                "[GridRebalancer] Price ${:.4} outside bounds [${:.4}, ${:.4}]",
                current_price, lower, upper
            );
            return true;
        }

        // Check 2: Volatility shift > 20%?
        let current_volatility = grid.get_volatility();
        let initial_volatility = grid.get_initial_volatility();

        if initial_volatility > 0.0 {
            let vol_change_pct = ((current_volatility - initial_volatility) / initial_volatility).abs();
            if vol_change_pct > 0.20 {
                debug!(
                    "[GridRebalancer] Volatility shift: {:.2}% → {:.2}% (change: {:.1}%)",
                    initial_volatility * 100.0,
                    current_volatility * 100.0,
                    vol_change_pct * 100.0
                );
                return true;
            }
        }

        false
    }

    /// Human-readable reason for repositioning (for logs).
    fn reposition_reason(grid: &GridState, current_price: f64, config: &Config) -> String {
        let (lower, upper) = grid.get_grid_bounds();
        if current_price < lower {
            return format!("price ${:.4} below lower bound ${:.4}", current_price, lower);
        }
        if current_price > upper {
            return format!("price ${:.4} above upper bound ${:.4}", current_price, upper);
        }

        let current_volatility = grid.get_volatility();
        let initial_volatility = grid.get_initial_volatility();
        if initial_volatility > 0.0 {
            let vol_change_pct = ((current_volatility - initial_volatility) / initial_volatility).abs();
            if vol_change_pct > 0.20 {
                return format!(
                    "volatility shift {:.1}% (was {:.2}%, now {:.2}%)",
                    vol_change_pct * 100.0,
                    initial_volatility * 100.0,
                    current_volatility * 100.0
                );
            }
        }

        "unknown".to_string()
    }

    /// Reposition the grid around current price with updated spacing.
    /// All grid mutations happen via GridState's public methods.
    #[allow(clippy::too_many_arguments)]
    async fn reposition_grid(
        grid: &mut GridState,
        config: &Config,
        current_price: f64,
        timestamp: i64,
    ) -> Result<()> {
        let num_levels = config.grid.levels;
        let spacing_pct = grid.calculate_adaptive_spacing(config);

        // GridState handles all volatility-adjusted level placement
        grid.place_grid(
            current_price,
            num_levels,
            spacing_pct,
            timestamp,
        )?;

        info!(
            "[GridRebalancer] Grid repositioned: {} levels | spacing {:.2}% | center ${:.4}",
            num_levels,
            spacing_pct * 100.0,
            current_price
        );

        Ok(())
    }

    /// Calculate position size based on ATR and config parameters.
    ///
    /// Formula:
    ///   base_size = config.grid.order_size_usd
    ///   atr_multiplier = 1.0 + (volatility * config.strategy.volatility_scaling_factor)
    ///   position_size = base_size * atr_multiplier
    ///
    /// Higher volatility → larger positions (capture bigger moves)
    fn calculate_position_size(
        config: &Config,
        volatility: f64,
        _current_price: f64,
    ) -> f64 {
        let base_size = config.grid.order_size_usd;
        let scaling_factor = config.strategy.volatility_scaling_factor;

        let atr_multiplier = 1.0 + (volatility * scaling_factor);
        let position_size = base_size * atr_multiplier;

        // Clamp to reasonable bounds
        let min_size = base_size * 0.5;
        let max_size = base_size * 2.0;

        position_size.max(min_size).min(max_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GridConfig;
    use crate::bots::grid_state::GridState;

    fn mock_config() -> Config {
        let mut config = Config::default();
        config.grid = GridConfig {
            levels: 10,
            spacing_pct: 0.01,
            order_size_usd: 100.0,
            enable_dynamic_spacing: true,
            ..Default::default()
        };
        config
    }

    #[test]
    fn test_should_reposition_price_out_of_bounds() {
        let config = mock_config();
        let mut grid = GridState::new(100.0, 200.0, 10);
        grid.place_grid(150.0, 10, 0.01, 0).unwrap();

        // Price way below lower bound → should reposition
        assert!(GridRebalancer::should_reposition(&grid, 80.0, &config));

        // Price way above upper bound → should reposition
        assert!(GridRebalancer::should_reposition(&grid, 220.0, &config));
    }

    #[test]
    fn test_should_not_reposition_within_bounds() {
        let config = mock_config();
        let mut grid = GridState::new(100.0, 200.0, 10);
        grid.place_grid(150.0, 10, 0.01, 0).unwrap();

        // Price within bounds → no reposition
        assert!(!GridRebalancer::should_reposition(&grid, 150.0, &config));
        assert!(!GridRebalancer::should_reposition(&grid, 175.0, &config));
    }

    #[test]
    fn test_calculate_position_size_scales_with_volatility() {
        let config = mock_config();

        let low_vol_size = GridRebalancer::calculate_position_size(&config, 0.01, 100.0);
        let high_vol_size = GridRebalancer::calculate_position_size(&config, 0.10, 100.0);

        // Higher volatility → larger position
        assert!(high_vol_size > low_vol_size);

        // Should be clamped to max 2x base size
        assert!(high_vol_size <= config.grid.order_size_usd * 2.0);
    }

    #[test]
    fn test_calculate_position_size_clamps_to_min() {
        let config = mock_config();

        // Zero volatility should still give at least 0.5x base size
        let size = GridRebalancer::calculate_position_size(&config, 0.0, 100.0);
        assert!(size >= config.grid.order_size_usd * 0.5);
    }
}
