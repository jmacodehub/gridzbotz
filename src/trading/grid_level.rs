//! ═══════════════════════════════════════════════════════════════════════════
//! 📊 GRID LEVEL STATE TRACKER V4.3 - GRIDZBOTZ
//!
//! V4.3 ADDITIONS (PR #107 — fix/fee-reconciliation, Commit 3):
//! ✅ GridLevel: fees_paid: f64 field added (default 0.0)
//!    - on_buy_filled(fee_usdc) — accumulate buy-side fee
//!    - on_sell_filled(fee_usdc) — accumulate sell-side fee; compute NET P&L:
//!      net = (sell_price - buy_price) * quantity - total_fees_paid
//!    - Caller (grid_bot.rs Commit 4) sources fee_usdc from FeesConfig:
//!      fee_usdc = fill_price * quantity * (taker_fee_pct / 100.0)
//! ✅ GridStateTracker: mark_buy_filled(id, fee_usdc) / mark_sell_filled(id, fee_usdc)
//! ✅ GridStateTracker: total_fees_paid() -> f64 (sum across all levels)
//! ✅ display(): shows fees alongside P&L
//! ✅ All existing call sites updated (0.0 arg — zero behaviour change for non-fee paths)
//! ✅ 4 new unit tests + existing suite updated
//!
//! V4.2 — GridStateTracker: Added reposition safety, get_all_levels(), tests.
//! V4.1 — Initial GridLevel + GridStateTracker impl.
//! ═══════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

// ═══════════════════════════════════════════════════════════════════════════
// GRID LEVEL STATUS
// ═══════════════════════════════════════════════════════════════════════════

/// Lifecycle states of a single grid level.
///
/// State machine:
///   Pending → BuyFilled → BothFilled  (complete cycle)
///   Pending → BuyFilled → Cancelled   (partial fill cancelled)
///   Pending → Cancelled               (unfilled cancel)
///   Any state → Repositioning          (grid rebalance in progress)
#[derive(Debug, Clone, PartialEq)]
pub enum GridLevelStatus {
    /// Buy order placed, not yet filled.
    Pending,
    /// Buy order confirmed filled. Waiting for sell.
    BuyFilled,
    /// Both buy and sell filled. Cycle complete.
    BothFilled,
    /// One or both orders cancelled before completion.
    Cancelled,
    /// Level is being repositioned (grid rebalance).
    Repositioning,
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID LEVEL - V4.3 (fees_paid added)
// ═══════════════════════════════════════════════════════════════════════════

/// A single grid price level with order tracking and fee-aware P&L.
///
/// `realized_pnl` is always **net**: gross spread minus all fees paid
/// on this level. Use `fees_paid` to inspect the cost separately.
///
/// # P&L formula (on sell fill)
/// ```text
/// gross   = (sell_price - buy_price) * quantity
/// net_pnl = gross - fees_paid          // fees_paid already includes buy-side fee
/// ```
#[derive(Debug, Clone)]
pub struct GridLevel {
    pub id:           u64,
    pub buy_price:    f64,
    pub sell_price:   f64,
    pub quantity:     f64,
    pub status:       GridLevelStatus,
    pub buy_order_id:  Option<String>,
    pub sell_order_id: Option<String>,
    pub created_at:   Instant,
    pub filled_at:    Option<Instant>,
    /// Net realized P&L (gross spread minus total fees_paid).
    pub realized_pnl: f64,
    /// Total fees paid on this level (buy fee + sell fee), in USDC.
    /// Populated by callers who source the rate from FeesConfig.
    pub fees_paid:    f64,
}

impl GridLevel {
    /// Create a new Pending grid level.
    pub fn new(id: u64, buy_price: f64, sell_price: f64, quantity: f64) -> Self {
        Self {
            id,
            buy_price,
            sell_price,
            quantity,
            status:        GridLevelStatus::Pending,
            buy_order_id:  None,
            sell_order_id: None,
            created_at:    Instant::now(),
            filled_at:     None,
            realized_pnl:  0.0,
            fees_paid:     0.0,
        }
    }

    /// Mark the buy side as filled. Accumulates `fee_usdc` into `fees_paid`.
    /// Call with fee_usdc = fill_price * quantity * (taker_fee_pct / 100.0).
    /// Pass 0.0 when no fee info is available (paper trading).
    pub fn on_buy_filled(&mut self, fee_usdc: f64) {
        self.status = GridLevelStatus::BuyFilled;
        self.fees_paid += fee_usdc;
    }

    /// Mark the sell side as filled. Accumulates `fee_usdc` and records
    /// NET realized P&L = gross spread - total fees_paid on this level.
    pub fn on_sell_filled(&mut self, fee_usdc: f64) {
        self.fees_paid += fee_usdc;
        let gross = (self.sell_price - self.buy_price) * self.quantity;
        self.realized_pnl = gross - self.fees_paid;
        self.status    = GridLevelStatus::BothFilled;
        self.filled_at = Some(Instant::now());
    }

    /// Mark the level as cancelled.
    pub fn cancel(&mut self) {
        self.status = GridLevelStatus::Cancelled;
    }

    /// Mark the level as being repositioned.
    pub fn start_repositioning(&mut self) {
        self.status = GridLevelStatus::Repositioning;
    }

    /// Seconds elapsed since this level was created.
    pub fn age_seconds(&self) -> u64 {
        self.created_at.elapsed().as_secs()
    }

    /// True if the level has been open longer than `max_age`.
    pub fn is_stale(&self, max_age: Duration) -> bool {
        self.created_at.elapsed() > max_age
    }

    /// True if the level completed a full buy→sell cycle.
    pub fn is_complete(&self) -> bool {
        self.status == GridLevelStatus::BothFilled
    }

    /// True if no action has been taken yet.
    pub fn is_pending(&self) -> bool {
        self.status == GridLevelStatus::Pending
    }

    /// True if buy is filled and waiting for sell.
    pub fn is_buy_filled(&self) -> bool {
        self.status == GridLevelStatus::BuyFilled
    }

    /// Print a single-line summary to stdout.
    pub fn display(&self) {
        println!(
            "  Level {} | Buy ${:.4} → Sell ${:.4} | Status: {:?} | Age: {}s",
            self.id, self.buy_price, self.sell_price,
            self.status, self.age_seconds()
        );
        if let Some(buy_id) = &self.buy_order_id {
            println!("    Buy Order:  {}", buy_id);
        }
        if let Some(sell_id) = &self.sell_order_id {
            println!("    Sell Order: {}", sell_id);
        }
        if self.realized_pnl != 0.0 || self.fees_paid != 0.0 {
            println!("    Net P&L: ${:.4} (fees paid: ${:.4})",
                self.realized_pnl, self.fees_paid);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID STATE TRACKER - V4.3
// ═══════════════════════════════════════════════════════════════════════════

/// Thread-safe tracker for all active grid levels.
///
/// Wraps a `HashMap<u64, GridLevel>` under an async `RwLock`.
/// Each method acquires the lock independently — callers MUST NOT hold
/// a read guard while calling a write method (deadlock).
pub struct GridStateTracker {
    levels:     RwLock<HashMap<u64, GridLevel>>,
    next_id:    RwLock<u64>,
}

impl GridStateTracker {
    /// Construct an empty tracker.
    pub fn new() -> Self {
        Self {
            levels:  RwLock::new(HashMap::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Allocate the next sequential level ID.
    async fn next_id(&self) -> u64 {
        let mut id = self.next_id.write().await;
        let v = *id;
        *id += 1;
        v
    }

    /// Create a new Pending level and insert it.
    pub async fn create_level(&self, buy_price: f64, sell_price: f64, quantity: f64) -> u64 {
        let id = self.next_id().await;
        let level = GridLevel::new(id, buy_price, sell_price, quantity);
        self.levels.write().await.insert(id, level);
        id
    }

    /// Mark the buy side filled, accumulating `fee_usdc` into the level's fees_paid.
    /// No-op (with warning) if the level ID is unknown.
    pub async fn mark_buy_filled(&self, level_id: u64, fee_usdc: f64) {
        let mut levels = self.levels.write().await;
        match levels.get_mut(&level_id) {
            Some(level) => level.on_buy_filled(fee_usdc),
            None => log::warn!("mark_buy_filled: unknown level_id={}", level_id),
        }
    }

    /// Mark the sell side filled, accumulating `fee_usdc` and computing net P&L.
    /// No-op (with warning) if the level ID is unknown.
    pub async fn mark_sell_filled(&self, level_id: u64, fee_usdc: f64) {
        let mut levels = self.levels.write().await;
        match levels.get_mut(&level_id) {
            Some(level) => level.on_sell_filled(fee_usdc),
            None => log::warn!("mark_sell_filled: unknown level_id={}", level_id),
        }
    }

    /// Mark a level as cancelled. No-op if unknown.
    pub async fn cancel_level(&self, level_id: u64) {
        if let Some(level) = self.levels.write().await.get_mut(&level_id) {
            level.cancel();
        }
    }

    /// Mark a level as repositioning. No-op if unknown.
    pub async fn start_repositioning(&self, level_id: u64) {
        if let Some(level) = self.levels.write().await.get_mut(&level_id) {
            level.start_repositioning();
        }
    }

    /// Remove all levels in Cancelled or BothFilled state.
    pub async fn cleanup_completed_levels(&self) {
        self.levels.write().await
            .retain(|_, v| {
                v.status != GridLevelStatus::Cancelled
                    && v.status != GridLevelStatus::BothFilled
            });
    }

    /// Count of levels in each status category.
    pub async fn level_counts(&self) -> (usize, usize, usize) {
        let levels = self.levels.read().await;
        let pending    = levels.values().filter(|l| l.status == GridLevelStatus::Pending).count();
        let buy_filled = levels.values().filter(|l| l.status == GridLevelStatus::BuyFilled).count();
        let completed  = levels.values().filter(|l| l.status == GridLevelStatus::BothFilled).count();
        (pending, buy_filled, completed)
    }

    /// Sum of realized_pnl (net of fees) across all BothFilled levels.
    pub async fn total_realized_pnl(&self) -> f64 {
        self.levels.read().await
            .values()
            .filter(|l| l.status == GridLevelStatus::BothFilled)
            .map(|l| l.realized_pnl)
            .sum()
    }

    /// Sum of all fees_paid across every level (all statuses).
    /// Useful for total cost tracking and reconciliation with FeesConfig.
    pub async fn total_fees_paid(&self) -> f64 {
        self.levels.read().await
            .values()
            .map(|l| l.fees_paid)
            .sum()
    }

    /// Returns a snapshot of all levels (cloned).
    pub async fn get_all_levels(&self) -> Vec<GridLevel> {
        self.levels.read().await.values().cloned().collect()
    }

    /// Count of stale pending levels older than `max_age`.
    pub async fn stale_level_count(&self, max_age: Duration) -> usize {
        self.levels.read().await
            .values()
            .filter(|l| l.status == GridLevelStatus::Pending && l.is_stale(max_age))
            .count()
    }

    /// Print a summary of all levels to stdout.
    pub async fn display_all(&self) {
        let levels = self.levels.read().await;
        println!("══════════════════════════════════════════════════");
        println!("📊 Grid Levels ({} total)", levels.len());
        let mut sorted: Vec<&GridLevel> = levels.values().collect();
        sorted.sort_by_key(|l| l.id);
        for level in sorted { level.display(); }
        println!("══════════════════════════════════════════════════");
    }
}

impl Default for GridStateTracker {
    fn default() -> Self { Self::new() }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── GridLevel unit tests ──────────────────────────────────────────────────

    #[test]
    fn test_grid_level_initial_state() {
        let level = GridLevel::new(1, 100.0, 101.0, 1.0);
        assert_eq!(level.id, 1);
        assert_eq!(level.status, GridLevelStatus::Pending);
        assert!(level.buy_order_id.is_none());
        assert!(level.sell_order_id.is_none());
        assert_eq!(level.realized_pnl, 0.0);
        assert_eq!(level.fees_paid, 0.0);
        assert!(level.is_pending());
        assert!(!level.is_complete());
    }

    #[test]
    fn test_grid_level_status_transitions() {
        let mut level = GridLevel::new(1, 100.0, 101.0, 1.0);
        assert_eq!(level.status, GridLevelStatus::Pending);

        level.on_buy_filled(0.0);
        assert_eq!(level.status, GridLevelStatus::BuyFilled);
        assert!(level.is_buy_filled());
        assert!(!level.is_complete());

        level.on_sell_filled(0.0);
        assert_eq!(level.status, GridLevelStatus::BothFilled);
        assert!(level.is_complete());
        assert!(level.filled_at.is_some());
    }

    #[test]
    fn test_grid_level_cancel() {
        let mut level = GridLevel::new(2, 99.0, 100.0, 0.5);
        level.cancel();
        assert_eq!(level.status, GridLevelStatus::Cancelled);
    }

    #[test]
    fn test_grid_level_repositioning() {
        let mut level = GridLevel::new(3, 98.0, 99.0, 1.0);
        level.start_repositioning();
        assert_eq!(level.status, GridLevelStatus::Repositioning);
    }

    #[test]
    fn test_grid_level_age() {
        let level = GridLevel::new(4, 100.0, 101.0, 1.0);
        assert!(level.age_seconds() < 2, "new level must be < 2s old");
        assert!(!level.is_stale(Duration::from_secs(10)));
    }

    // ── V4.3: fee tracking unit tests ──────────────────────────────────────────

    #[test]
    fn test_fees_paid_accumulates_both_fills() {
        let mut level = GridLevel::new(1, 100.0, 101.0, 1.0);
        level.on_buy_filled(0.05);
        assert!(
            (level.fees_paid - 0.05).abs() < 1e-9,
            "after buy fill fees_paid must be 0.05, got {:.6}", level.fees_paid
        );
        level.on_sell_filled(0.05);
        assert!(
            (level.fees_paid - 0.10).abs() < 1e-9,
            "after sell fill total fees must be 0.10, got {:.6}", level.fees_paid
        );
    }

    #[test]
    fn test_realized_pnl_is_net_of_fees() {
        let mut level = GridLevel::new(1, 100.0, 101.0, 1.0);
        // gross = (101 - 100) * 1.0 = 1.0
        level.on_buy_filled(0.05);
        level.on_sell_filled(0.05);
        // net = 1.0 - 0.10 = 0.90
        let expected = 1.0_f64 - 0.10;
        assert!(
            (level.realized_pnl - expected).abs() < 1e-9,
            "net P&L must be gross minus total fees; expected {:.4}, got {:.4}",
            expected, level.realized_pnl
        );
    }

    #[test]
    fn test_realized_pnl_zero_fees_unchanged() {
        let mut level = GridLevel::new(1, 100.0, 101.0, 1.0);
        level.on_buy_filled(0.0);
        level.on_sell_filled(0.0);
        let expected = (101.0_f64 - 100.0) * 1.0;
        assert!(
            (level.realized_pnl - expected).abs() < 1e-9,
            "zero fees must not change gross P&L; expected {:.4}, got {:.4}",
            expected, level.realized_pnl
        );
    }

    // ── GridStateTracker async tests ────────────────────────────────────────

    #[tokio::test]
    async fn test_grid_state_tracker() {
        let tracker = GridStateTracker::new();

        let id1 = tracker.create_level(100.0, 101.0, 1.0).await;
        let id2 = tracker.create_level(99.0, 100.0, 1.0).await;
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        let (pending, buy_filled, completed) = tracker.level_counts().await;
        assert_eq!(pending, 2);
        assert_eq!(buy_filled, 0);
        assert_eq!(completed, 0);

        tracker.mark_buy_filled(id1, 0.0).await;
        let (pending, buy_filled, _) = tracker.level_counts().await;
        assert_eq!(pending, 1);
        assert_eq!(buy_filled, 1);

        tracker.mark_sell_filled(id1, 0.0).await;
        let (_, _, completed) = tracker.level_counts().await;
        assert_eq!(completed, 1);

        let pnl = tracker.total_realized_pnl().await;
        assert!(pnl > 0.0, "completed level must have positive P&L, got {:.4}", pnl);
    }

    #[tokio::test]
    async fn test_cleanup_removes_completed_and_cancelled() {
        let tracker = GridStateTracker::new();

        let id1 = tracker.create_level(100.0, 101.0, 1.0).await;
        let id2 = tracker.create_level(99.0, 100.0, 1.0).await;
        let id3 = tracker.create_level(98.0, 99.0, 1.0).await;

        tracker.mark_buy_filled(id1, 0.0).await;
        tracker.mark_sell_filled(id1, 0.0).await;
        tracker.cancel_level(id2).await;

        tracker.cleanup_completed_levels().await;

        let levels = tracker.get_all_levels().await;
        assert_eq!(levels.len(), 1, "only pending level id3 must remain");
        assert_eq!(levels[0].id, id3);
    }

    #[tokio::test]
    async fn test_reposition_safety() {
        let tracker = GridStateTracker::new();

        tracker.create_level(100.0, 101.0, 1.0).await;
        let id2 = tracker.create_level(99.0, 100.0, 1.0).await;

        tracker.mark_buy_filled(id2, 0.0).await;
        tracker.start_repositioning(id2).await;

        let levels = tracker.get_all_levels().await;
        let repo_levels: Vec<_> = levels.iter()
            .filter(|l| l.status == GridLevelStatus::Repositioning)
            .collect();
        assert_eq!(repo_levels.len(), 1);
        assert_eq!(repo_levels[0].id, id2);
    }

    #[tokio::test]
    async fn test_get_all_levels_includes_all_statuses() {
        let tracker = GridStateTracker::new();

        let id1 = tracker.create_level(100.0, 101.0, 1.0).await;
        let id2 = tracker.create_level(99.0, 100.0, 1.0).await;
        let id3 = tracker.create_level(98.0, 99.0,  1.0).await;

        tracker.mark_buy_filled(id1, 0.0).await;
        tracker.cancel_level(id2).await;
        let _ = id3;

        let levels = tracker.get_all_levels().await;
        assert_eq!(levels.len(), 3, "get_all_levels must return every level regardless of status");
    }

    #[tokio::test]
    async fn test_total_fees_paid() {
        let tracker = GridStateTracker::new();
        let id1 = tracker.create_level(100.0, 101.0, 1.0).await;
        let id2 = tracker.create_level(99.0, 100.0, 1.0).await;

        tracker.mark_buy_filled(id1, 0.05).await;
        tracker.mark_sell_filled(id1, 0.05).await;
        tracker.mark_buy_filled(id2, 0.04).await;

        let total = tracker.total_fees_paid().await;
        assert!(
            (total - 0.14).abs() < 1e-9,
            "total fees must sum all level fees; expected 0.14, got {:.6}", total
        );
    }
}
