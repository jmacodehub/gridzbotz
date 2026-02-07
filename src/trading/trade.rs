//! ═══════════════════════════════════════════════════════════════════════════
//! Trade Tracking Module V3.5
//!
//! Comprehensive trade record with full P&L calculation, timing, and analytics.
//! This is the production-grade trade tracking system for Project Flash.
//!
//! Features:
//! - Complete P&L tracking (gross, net, fees)
//! - Trade timing and duration
//! - Market context (volatility, price at entry)
//! - Performance metrics (win/loss, ROI)
//! - Serialization support for logging and analysis
//! ═══════════════════════════════════════════════════════════════════════════

use super::OrderSide;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Comprehensive trade record with full P&L and analytics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trade {
    /// Unique trade identifier
    pub id: String,

    /// Reference to the order that created this trade
    pub order_id: String,

    /// Trade direction (Buy or Sell)
    pub side: OrderSide,

    // ═══════════════════════════════════════════════════════════════════════
    // PRICE INFORMATION
    // ═══════════════════════════════════════════════════════════════════════
    /// Entry price (price when trade opened)
    pub entry_price: f64,

    /// Exit price (price when trade closed)
    pub exit_price: f64,

    /// Average price (for partial fills)
    pub avg_price: f64,

    /// Market price when trade was entered (for context)
    pub price_at_entry: f64,

    // ═══════════════════════════════════════════════════════════════════════
    // POSITION SIZE
    // ═══════════════════════════════════════════════════════════════════════
    /// Size in SOL
    pub size: f64,

    /// USD value of the trade
    pub value_usd: f64,

    // ═══════════════════════════════════════════════════════════════════════
    // PROFIT & LOSS
    // ═══════════════════════════════════════════════════════════════════════
    /// Gross P&L (before fees)
    pub gross_pnl: f64,

    /// Total fees paid (maker + taker)
    pub fees_paid: f64,

    /// Net P&L (after fees) - THE KEY METRIC
    pub net_pnl: f64,

    /// P&L as percentage of trade value
    pub pnl_percent: f64,

    // ═══════════════════════════════════════════════════════════════════════
    // TIMING
    // ═══════════════════════════════════════════════════════════════════════
    /// When the trade was opened
    pub entry_time: DateTime<Utc>,

    /// When the trade was closed
    pub exit_time: DateTime<Utc>,

    /// Duration in seconds
    pub duration_secs: u64,

    // ═══════════════════════════════════════════════════════════════════════
    // MARKET CONTEXT
    // ═══════════════════════════════════════════════════════════════════════
    /// Market volatility at time of trade
    pub volatility: f64,
}

impl Trade {
    /// Create a new trade from an order execution
    ///
    /// # Arguments
    /// * `order_id` - ID of the order that created this trade
    /// * `side` - Buy or Sell
    /// * `entry_price` - Price at entry
    /// * `size` - Trade size in SOL
    /// * `entry_time` - Timestamp of entry
    ///
    /// # Returns
    /// A new Trade instance with entry data populated
    pub fn new(
        order_id: String,
        side: OrderSide,
        entry_price: f64,
        size: f64,
        entry_time: DateTime<Utc>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            order_id,
            side,
            entry_price,
            exit_price: 0.0,
            avg_price: entry_price,
            price_at_entry: entry_price,
            size,
            value_usd: entry_price * size,
            gross_pnl: 0.0,
            fees_paid: 0.0,
            net_pnl: 0.0,
            pnl_percent: 0.0,
            entry_time,
            exit_time: entry_time,
            duration_secs: 0,
            volatility: 0.0,
        }
    }

    /// Create a trade with market context
    pub fn new_with_context(
        order_id: String,
        side: OrderSide,
        entry_price: f64,
        size: f64,
        entry_time: DateTime<Utc>,
        market_price: f64,
        volatility: f64,
    ) -> Self {
        let mut trade = Self::new(order_id, side, entry_price, size, entry_time);
        trade.price_at_entry = market_price;
        trade.volatility = volatility;
        trade
    }

    /// Close the trade with exit information
    ///
    /// # Arguments
    /// * `exit_price` - Price at which trade was closed
    /// * `exit_time` - Timestamp of exit
    /// * `maker_fee` - Maker fee rate (e.g., 0.0002 for 0.02%)
    /// * `taker_fee` - Taker fee rate (e.g., 0.0004 for 0.04%)
    pub fn close(
        &mut self,
        exit_price: f64,
        exit_time: DateTime<Utc>,
        maker_fee: f64,
        taker_fee: f64,
    ) {
        self.exit_price = exit_price;
        self.exit_time = exit_time;
        self.duration_secs = (exit_time - self.entry_time).num_seconds().max(0) as u64;
        self.calculate_pnl(maker_fee, taker_fee);
    }

    /// Calculate profit and loss for this trade
    ///
    /// # Arguments
    /// * `maker_fee` - Maker fee rate (decimal, e.g., 0.0002 = 0.02%)
    /// * `taker_fee` - Taker fee rate (decimal, e.g., 0.0004 = 0.04%)
    ///
    /// # Formula
    /// - For Buy: Profit = (Exit Price - Entry Price) * Size
    /// - For Sell: Profit = (Entry Price - Exit Price) * Size
    /// - Net P&L = Gross P&L - Fees
    pub fn calculate_pnl(&mut self, maker_fee: f64, taker_fee: f64) {
        // Calculate gross P&L based on side
        let price_diff = match self.side {
            OrderSide::Buy => self.exit_price - self.entry_price,
            OrderSide::Sell => self.entry_price - self.exit_price,
        };

        self.gross_pnl = price_diff * self.size;

        // Calculate fees
        // Assume maker fee on entry, taker fee on exit (conservative estimate)
        let entry_fee = self.value_usd * maker_fee;
        let exit_value = self.exit_price * self.size;
        let exit_fee = exit_value * taker_fee;
        self.fees_paid = entry_fee + exit_fee;

        // Net P&L after fees
        self.net_pnl = self.gross_pnl - self.fees_paid;

        // Percentage return
        if self.value_usd > 0.0 {
            self.pnl_percent = (self.net_pnl / self.value_usd) * 100.0;
        } else {
            self.pnl_percent = 0.0;
        }
    }

    /// Check if trade was profitable
    #[inline]
    pub fn is_winner(&self) -> bool {
        self.net_pnl > 0.0
    }

    /// Check if trade was a loss
    #[inline]
    pub fn is_loser(&self) -> bool {
        self.net_pnl < 0.0
    }

    /// Check if trade broke even
    #[inline]
    pub fn is_breakeven(&self) -> bool {
        self.net_pnl.abs() < 0.001 // Within $0.001
    }

    /// Calculate return per hour
    ///
    /// Useful for comparing trades of different durations
    pub fn return_per_hour(&self) -> f64 {
        let hours = self.duration_secs as f64 / 3600.0;
        if hours > 0.0 {
            self.net_pnl / hours
        } else {
            0.0
        }
    }

    /// Calculate return per day (annualized rate)
    pub fn return_per_day(&self) -> f64 {
        self.return_per_hour() * 24.0
    }

    /// Get trade efficiency (net P&L / fees)
    ///
    /// Higher is better. Values < 1 mean fees ate into profits.
    pub fn efficiency(&self) -> f64 {
        if self.fees_paid > 0.0 {
            self.net_pnl / self.fees_paid
        } else {
            0.0
        }
    }

    /// Get risk/reward ratio
    ///
    /// For closed trades, this is actual profit vs maximum potential loss
    pub fn risk_reward_ratio(&self) -> f64 {
        let max_loss = self.value_usd;
        if max_loss > 0.0 {
            self.net_pnl.abs() / max_loss
        } else {
            0.0
        }
    }

    /// Generate a human-readable summary
    pub fn summary(&self) -> String {
        let side_emoji = match self.side {
            OrderSide::Buy => "🟢",
            OrderSide::Sell => "🔴",
        };

        let result_emoji = if self.is_winner() {
            "✅"
        } else if self.is_loser() {
            "❌"
        } else {
            "➖"
        };

        format!(
            "{} {} Trade: {} SOL @ ${:.2} → ${:.2} | P&L: ${:+.2} ({:+.2}%) | Duration: {}s {}",
            side_emoji,
            match self.side {
                OrderSide::Buy => "BUY",
                OrderSide::Sell => "SELL",
            },
            self.size,
            self.entry_price,
            self.exit_price,
            self.net_pnl,
            self.pnl_percent,
            self.duration_secs,
            result_emoji
        )
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DISPLAY IMPLEMENTATIONS
// ═══════════════════════════════════════════════════════════════════════════

impl std::fmt::Display for Trade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.summary())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_creation() {
        let trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            Utc::now(),
        );

        assert_eq!(trade.entry_price, 100.0);
        assert_eq!(trade.size, 1.0);
        assert_eq!(trade.value_usd, 100.0);
    }

    #[test]
    fn test_winning_trade() {
        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            Utc::now(),
        );

        // Exit at higher price (profit)
        trade.close(105.0, Utc::now(), 0.0002, 0.0004);

        assert!(trade.gross_pnl > 0.0);
        assert!(trade.net_pnl > 0.0);
        assert!(trade.is_winner());
        assert!(!trade.is_loser());
        assert_eq!(trade.gross_pnl, 5.0); // $5 profit
    }

    #[test]
    fn test_losing_trade() {
        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            Utc::now(),
        );

        // Exit at lower price (loss)
        trade.close(95.0, Utc::now(), 0.0002, 0.0004);

        assert!(trade.gross_pnl < 0.0);
        assert!(trade.is_loser());
        assert!(!trade.is_winner());
    }

    #[test]
    fn test_sell_trade() {
        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Sell,
            100.0,
            1.0,
            Utc::now(),
        );

        // Exit at lower price (profit for sell)
        trade.close(95.0, Utc::now(), 0.0002, 0.0004);

        assert!(trade.gross_pnl > 0.0);
        assert!(trade.is_winner());
        assert_eq!(trade.gross_pnl, 5.0);
    }

    #[test]
    fn test_fee_calculation() {
        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            Utc::now(),
        );

        trade.close(105.0, Utc::now(), 0.0010, 0.0020); // 0.1% and 0.2% fees

        // Entry fee: $100 * 0.001 = $0.10
        // Exit fee: $105 * 0.002 = $0.21
        // Total fees: ~$0.31
        assert!(trade.fees_paid > 0.30 && trade.fees_paid < 0.32);

        // Net P&L should be gross minus fees
        let expected_net = trade.gross_pnl - trade.fees_paid;
        assert!((trade.net_pnl - expected_net).abs() < 0.001);
    }

    #[test]
    fn test_return_per_hour() {
        let entry_time = Utc::now();
        let exit_time = entry_time + chrono::Duration::hours(2);

        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            entry_time,
        );

        trade.close(110.0, exit_time, 0.0, 0.0); // No fees for simplicity

        // $10 profit in 2 hours = $5/hour
        let hourly = trade.return_per_hour();
        assert!((hourly - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_efficiency() {
        let mut trade = Trade::new(
            "order123".to_string(),
            OrderSide::Buy,
            100.0,
            1.0,
            Utc::now(),
        );

        trade.close(105.0, Utc::now(), 0.001, 0.001);

        // Efficiency = net_pnl / fees_paid
        let expected_efficiency = trade.net_pnl / trade.fees_paid;
        assert!((trade.efficiency() - expected_efficiency).abs() < 0.001);
    }
}
