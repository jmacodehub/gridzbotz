//! ═══════════════════════════════════════════════════════════════════════════
//! Grid Level State Machine - V4.0 Production Ready
//!
//! Ensures buy/sell pairing survives grid repositioning.
//! Prevents orphaned positions by tracking order lifecycle per level.
//!
//! Architecture:
//! - GridLevel: Single price level with paired buy/sell orders
//! - GridLevelStatus: State machine (Pending → Active → Filled)
//! - GridStateTracker: Thread-safe HashMap of all active levels
//!
//! Key Feature: `can_cancel()` prevents cancelling levels with filled buys!
//! ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};
use log::{info, warn, debug};

/// Grid level lifecycle status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GridLevelStatus {
    /// Initial state - orders not yet placed
    Pending,
    /// Buy order placed, waiting for fill
    BuyPending,
    /// Buy filled, sell order placed
    BuyFilled,
    /// Both orders active on book
    Active,
    /// Sell filled - level complete
    SellFilled,
    /// Level cancelled (safe to remove)
    Cancelled,
    /// Error state
    Failed,
}

impl GridLevelStatus {
    /// Check if level is in terminal state (no more changes)
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::SellFilled | Self::Cancelled | Self::Failed)
    }

    /// Check if level has filled buy (must preserve sell!)
    pub fn has_filled_buy(&self) -> bool {
        matches!(self, Self::BuyFilled | Self::SellFilled)
    }

    /// Check if level is safe to cancel during reposition
    pub fn can_cancel(&self) -> bool {
        !self.has_filled_buy() && !self.is_terminal()
    }
}

/// Single grid level with paired buy/sell orders
#[derive(Debug, Clone)]
pub struct GridLevel {
    /// Unique level ID
    pub id: u64,

    /// Order IDs
    pub buy_order_id: Option<String>,
    pub sell_order_id: Option<String>,

    /// Price levels
    pub buy_price: f64,
    pub sell_price: f64,

    /// Order size
    pub size: f64,

    /// Current status
    pub status: GridLevelStatus,

    /// Timestamps (runtime only - not serialized)
    pub created_at: std::time::Instant,
    pub buy_filled_at: Option<std::time::Instant>,
    pub sell_filled_at: Option<std::time::Instant>,

    /// P&L tracking
    pub realized_pnl: f64,
}

impl GridLevel {
    /// Create new grid level
    pub fn new(id: u64, buy_price: f64, sell_price: f64, size: f64) -> Self {
        Self {
            id,
            buy_order_id: None,
            sell_order_id: None,
            buy_price,
            sell_price,
            size,
            status: GridLevelStatus::Pending,
            created_at: std::time::Instant::now(),
            buy_filled_at: None,
            sell_filled_at: None,
            realized_pnl: 0.0,
        }
    }

    /// Mark buy order as placed
    pub fn set_buy_order(&mut self, order_id: String) {
        self.buy_order_id = Some(order_id);
        self.status = GridLevelStatus::BuyPending;
        debug!("Level {} buy order placed: {}", self.id, self.buy_order_id.as_ref().unwrap());
    }

    /// Mark sell order as placed
    pub fn set_sell_order(&mut self, order_id: String) {
        self.sell_order_id = Some(order_id);
        if self.status == GridLevelStatus::BuyFilled {
            // Sell placed after buy filled
            self.status = GridLevelStatus::Active;
        }
        debug!("Level {} sell order placed: {}", self.id, self.sell_order_id.as_ref().unwrap());
    }

    /// Mark buy as filled - CRITICAL STATE TRANSITION
    pub fn on_buy_filled(&mut self) {
        self.buy_filled_at = Some(std::time::Instant::now());
        self.status = GridLevelStatus::BuyFilled;
        info!("🟢 Level {} BUY FILLED @ ${:.4} - sell must survive reposition!",
              self.id, self.buy_price);
    }

    /// Mark sell as filled - calculate P&L
    pub fn on_sell_filled(&mut self) {
        self.sell_filled_at = Some(std::time::Instant::now());
        self.realized_pnl = (self.sell_price - self.buy_price) * self.size;
        self.status = GridLevelStatus::SellFilled;
        info!("💰 Level {} COMPLETE - P&L: ${:.2} ({:.2}%)",
              self.id,
              self.realized_pnl,
              (self.realized_pnl / (self.buy_price * self.size)) * 100.0);
    }

    /// Mark level as cancelled
    pub fn cancel(&mut self) {
        if self.status.can_cancel() {
            self.status = GridLevelStatus::Cancelled;
            debug!("Level {} cancelled (safe)", self.id);
        } else {
            warn!("⚠️  Cannot cancel level {} - status: {:?}", self.id, self.status);
        }
    }

    /// Check if this level can be safely cancelled
    pub fn can_cancel(&self) -> bool {
        self.status.can_cancel()
    }

    /// Get level age in seconds
    pub fn age_seconds(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    /// Display level status
    pub fn display(&self) {
        println!("  Level {} | Buy ${:.4} → Sell ${:.4} | Status: {:?} | Age: {}s",
                 self.id, self.buy_price, self.sell_price, self.status, self.age_seconds());
        if let Some(buy_id) = &self.buy_order_id {
            println!("    Buy Order:  {}", buy_id);
        }
        if let Some(sell_id) = &self.sell_order_id {
            println!("    Sell Order: {}", sell_id);
        }
        if self.realized_pnl != 0.0 {
            println!("    P&L: ${:.2}", self.realized_pnl);
        }
    }
}

/// Thread-safe grid state tracker
#[derive(Clone)]
pub struct GridStateTracker {
    levels: Arc<RwLock<HashMap<u64, GridLevel>>>,
    next_id: Arc<RwLock<u64>>,
}

impl GridStateTracker {
    /// Create new state tracker
    pub fn new() -> Self {
        info!("🎯 Grid State Tracker V4.0 initialized");
        Self {
            levels: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(RwLock::new(1)),
        }
    }

    /// Create new grid level
    pub async fn create_level(&self, buy_price: f64, sell_price: f64, size: f64) -> GridLevel {
        let mut next_id = self.next_id.write().await;
        let id = *next_id;
        *next_id += 1;

        let level = GridLevel::new(id, buy_price, sell_price, size);
        self.levels.write().await.insert(id, level.clone());
        debug!("Created level {} @ buy ${:.4} / sell ${:.4}", id, buy_price, sell_price);
        level
    }

    /// Update level (after order placement or fill)
    pub async fn update_level(&self, level: GridLevel) {
        self.levels.write().await.insert(level.id, level);
    }

    /// Mark buy as filled
    pub async fn mark_buy_filled(&self, level_id: u64) {
        if let Some(level) = self.levels.write().await.get_mut(&level_id) {
            level.on_buy_filled();
        }
    }

    /// Mark sell as filled
    pub async fn mark_sell_filled(&self, level_id: u64) {
        if let Some(level) = self.levels.write().await.get_mut(&level_id) {
            level.on_sell_filled();
        }
    }

    /// Get level by ID
    pub async fn get_level(&self, level_id: u64) -> Option<GridLevel> {
        self.levels.read().await.get(&level_id).cloned()
    }

    /// Get all cancellable level IDs (safe during reposition)
    pub async fn get_cancellable_levels(&self) -> Vec<u64> {
        self.levels.read().await
            .iter()
            .filter(|(_, level)| level.can_cancel())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all levels with filled buys (must preserve their sells!)
    pub async fn get_levels_with_filled_buys(&self) -> Vec<GridLevel> {
        self.levels.read().await
            .values()
            .filter(|level| level.status.has_filled_buy())
            .cloned()
            .collect()
    }

    /// Get all active levels
    pub async fn get_active_levels(&self) -> Vec<GridLevel> {
        self.levels.read().await
            .values()
            .filter(|level| !level.status.is_terminal())
            .cloned()
            .collect()
    }

    /// Clear completed/cancelled levels
    pub async fn cleanup_terminal_levels(&self) {
        let mut levels = self.levels.write().await;
        levels.retain(|_, level| !level.status.is_terminal());
    }

    /// Get total count of levels
    pub async fn count(&self) -> usize {
        self.levels.read().await.len()
    }

    /// Get total realized P&L
    pub async fn total_realized_pnl(&self) -> f64 {
        self.levels.read().await
            .values()
            .map(|level| level.realized_pnl)
            .sum()
    }

    /// Display all levels
    pub async fn display_all(&self) {
        let levels = self.levels.read().await;
        println!("\n📊 Grid Levels ({} total):", levels.len());
        for level in levels.values() {
            level.display();
        }
        println!();
    }

    /// Reset all levels (for testing)
    pub async fn reset(&self) {
        self.levels.write().await.clear();
        *self.next_id.write().await = 1;
        info!("Grid state tracker reset");
    }
}

impl Default for GridStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grid_level_status_transitions() {
        let mut level = GridLevel::new(1, 100.0, 102.0, 1.0);

        assert_eq!(level.status, GridLevelStatus::Pending);
        assert!(level.can_cancel());

        level.set_buy_order("BUY-1".to_string());
        assert_eq!(level.status, GridLevelStatus::BuyPending);
        assert!(level.can_cancel());

        level.on_buy_filled();
        assert_eq!(level.status, GridLevelStatus::BuyFilled);
        assert!(!level.can_cancel(), "Cannot cancel after buy filled!");

        level.set_sell_order("SELL-1".to_string());
        assert_eq!(level.status, GridLevelStatus::Active);

        level.on_sell_filled();
        assert_eq!(level.status, GridLevelStatus::SellFilled);
        assert!(level.status.is_terminal());
    }

    #[tokio::test]
    async fn test_grid_state_tracker() {
        let tracker = GridStateTracker::new();

        let level = tracker.create_level(100.0, 102.0, 1.0).await;
        assert_eq!(level.id, 1);

        tracker.mark_buy_filled(1).await;

        let cancellable = tracker.get_cancellable_levels().await;
        assert!(cancellable.is_empty(), "Level 1 has filled buy - should not be cancellable!");

        let filled_buys = tracker.get_levels_with_filled_buys().await;
        assert_eq!(filled_buys.len(), 1);
    }

    #[tokio::test]
    async fn test_reposition_safety() {
        let tracker = GridStateTracker::new();

        // Create 3 levels
        for i in 0..3 {
            let level = tracker.create_level(100.0 + i as f64, 102.0 + i as f64, 1.0).await;
            let mut level_clone = level.clone();
            level_clone.set_buy_order(format!("BUY-{}", i));
            tracker.update_level(level_clone).await;
        }

        // Fill buy for level 2
        tracker.mark_buy_filled(2).await;

        // Get cancellable levels (should exclude level 2!)
        let cancellable = tracker.get_cancellable_levels().await;
        assert_eq!(cancellable.len(), 2);
        assert!(!cancellable.contains(&2), "Level 2 should NOT be cancellable!");
    }
}
