//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥 REAL TRADER ENGINE V2.0 - MODULAR & BULLETPROOF
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Context, Result};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

// ─────────────────────────────────────────────────────────────────────────────
// 📦 MODULAR IMPORTS
// ─────────────────────────────────────────────────────────────────────────────
use crate::security::keystore::{SecureKeystore, KeystoreConfig};
use crate::risk::circuit_breaker::{CircuitBreaker, TripReason};
use crate::Config;
use super::executor::{TransactionExecutor, ExecutorConfig};
use super::trade::Trade;
use super::paper_trader::{Order, OrderSide};
use super::jupiter_client::{JupiterClient, JupiterConfig, SOL_MINT, USDC_MINT};
use solana_sdk::transaction::VersionedTransaction;

// ─────────────────────────────────────────────────────────────────────────────
// ⚙️ CONFIGURATION
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTradingConfig {
    pub keystore: KeystoreConfig,
    pub executor: ExecutorConfig,
    pub slippage_bps: Option<u16>,
    pub circuit_breaker_loss_pct: Option<f64>,
    pub stop_loss_pct: Option<f64>,
    pub profit_take_threshold: Option<f64>,
    pub profit_take_ratio: Option<f64>,
    pub maker_fee_bps: Option<f64>,
    pub taker_fee_bps: Option<f64>,
    pub reconcile_balances_every_n_trades: Option<u32>,
}

impl Default for RealTradingConfig {
    fn default() -> Self {
        Self {
            keystore: KeystoreConfig::default(),
            executor: ExecutorConfig::default(),
            slippage_bps: Some(50),
            circuit_breaker_loss_pct: Some(5.0),
            stop_loss_pct: Some(10.0),
            profit_take_threshold: Some(3.0),
            profit_take_ratio: Some(0.4),
            maker_fee_bps: Some(2.0),
            taker_fee_bps: Some(4.0),
            reconcile_balances_every_n_trades: Some(10),
        }
    }
}

impl RealTradingConfig {
    pub fn validate(&self) -> Result<()> {
        self.keystore.validate()?;
        self.executor.validate()?;

        if let Some(loss_pct) = self.circuit_breaker_loss_pct {
            if loss_pct <= 0.0 {
                bail!("circuit_breaker_loss_pct must be positive");
            }
        }

        if let Some(slippage) = self.slippage_bps {
            if slippage > 1000 {
                bail!("slippage_bps too high: {} (max 1000 = 10%)", slippage);
            }
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 💰 BALANCE TRACKER
// ─────────────────────────────────────────────────────────────────────────────
struct BalanceTracker {
    expected_usdc: Arc<RwLock<f64>>,
    expected_sol: Arc<RwLock<f64>>,
    /// Initial portfolio value in USD, calculated at boot using the live
    /// SOL price from the Pyth feed — never a hardcoded estimate.
    initial_balance_usd: f64,
}

impl BalanceTracker {
    /// Create a new balance tracker.
    ///
    /// `sol_price_usd` must be the live SOL/USD price fetched from the
    /// price feed at engine initialisation — do NOT pass a hardcoded value.
    fn new(initial_usdc: f64, initial_sol: f64, sol_price_usd: f64) -> Self {
        Self {
            expected_usdc: Arc::new(RwLock::new(initial_usdc)),
            expected_sol: Arc::new(RwLock::new(initial_sol)),
            initial_balance_usd: initial_usdc + (initial_sol * sol_price_usd),
        }
    }

    async fn get_balances(&self) -> (f64, f64) {
        let usdc = *self.expected_usdc.read().await;
        let sol = *self.expected_sol.read().await;
        (usdc, sol)
    }

    #[allow(dead_code)]
    async fn update(&self, usdc: f64, sol: f64) {
        *self.expected_usdc.write().await = usdc;
        *self.expected_sol.write().await = sol;
    }

    fn initial_balance_usd(&self) -> f64 {
        self.initial_balance_usd
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 📊 PERFORMANCE STATS
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub total_trades: usize,
    pub winning_trades: usize,
    pub losing_trades: usize,
    pub total_pnl: f64,
    pub total_fees: f64,
    pub win_rate: f64,
    pub avg_win: f64,
    pub avg_loss: f64,
    pub largest_win: f64,
    pub largest_loss: f64,
    pub profit_factor: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// 🔥 REAL TRADING ENGINE
// ─────────────────────────────────────────────────────────────────────────────
pub struct RealTradingEngine {
    keystore: Arc<SecureKeystore>,
    executor: Arc<RwLock<TransactionExecutor>>,
    circuit_breaker: Arc<RwLock<CircuitBreaker>>,
    balance_tracker: Arc<BalanceTracker>,
    config: RealTradingConfig,
    trades: Arc<RwLock<Vec<Trade>>>,
    _open_orders: Arc<RwLock<HashMap<String, Order>>>,
    next_id: Arc<AtomicU64>,
    total_executions: Arc<AtomicU64>,
    successful_executions: Arc<AtomicU64>,
    failed_executions: Arc<AtomicU64>,
    emergency_shutdown: Arc<AtomicBool>,
}

impl RealTradingEngine {
    /// Construct the real trading engine.
    ///
    /// `initial_sol_price_usd` must be the live SOL/USD price from the
    /// Pyth price feed at the time of construction — e.g.
    /// `feed.latest_price().await`.  This is used to compute the initial
    /// portfolio NAV and, from it, the accurate ROI throughout the session.
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
        initial_sol_price_usd: f64,
    ) -> Result<Self> {
        info!("🚀 Initializing Real Trading Engine V2.0");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        let executor = Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?));

        // Use Config for CircuitBreaker
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_balance_usdc)
        ));

        let balance_tracker = Arc::new(BalanceTracker::new(
            initial_balance_usdc,
            initial_balance_sol,
            initial_sol_price_usd,
        ));

        info!("✅ Real Trading Engine initialized");
        info!("   Wallet:        {}", keystore.pubkey());
        info!("   Initial NAV:   ${:.2} (SOL @ ${:.4})",
            balance_tracker.initial_balance_usd(), initial_sol_price_usd);

        Ok(Self {
            keystore,
            executor,
            circuit_breaker,
            balance_tracker,
            config,
            trades: Arc::new(RwLock::new(Vec::new())),
            _open_orders: Arc::new(RwLock::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(1)),
            total_executions: Arc::new(AtomicU64::new(0)),
            successful_executions: Arc::new(AtomicU64::new(0)),
            failed_executions: Arc::new(AtomicU64::new(0)),
            emergency_shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn buy(&self, price: f64, size: f64) -> Result<String> {
        self.execute_trade(OrderSide::Buy, price, size).await
    }

    pub async fn sell(&self, price: f64, size: f64) -> Result<String> {
        self.execute_trade(OrderSide::Sell, price, size).await
    }

    async fn execute_trade(&self, side: OrderSide, price: f64, size: f64) -> Result<String> {
        if self.emergency_shutdown.load(Ordering::SeqCst) {
            bail!("🚨 EMERGENCY SHUTDOWN ACTIVE");
        }

        if !self.circuit_breaker.write().await.is_trading_allowed() {
            bail!("🚨 CIRCUIT BREAKER ACTIVE");
        }

        let amount_usdc = price * size;
        self.keystore.validate_transaction(amount_usdc).await?;

        let order_id = format!("REAL-{:06}", self.next_id.fetch_add(1, Ordering::SeqCst));
        info!("📝 {:?} order: {:.4} SOL @ ${:.2}", side, size, price);

        self.total_executions.fetch_add(1, Ordering::SeqCst);

        // 🔥 BUILD ACTUAL JUPITER SWAP — full VersionedTransaction (ALTs preserved)
        let (versioned_tx, _last_valid) = self.build_jupiter_swap(side, price, size).await?;

        let executor = self.executor.write().await;
        let signature = executor.execute_versioned(
            versioned_tx,
            |tx| self.keystore.sign_versioned_transaction(tx),
        ).await;

        match signature {
            Ok(sig) => {
                self.successful_executions.fetch_add(1, Ordering::SeqCst);

                let trade = Trade::new(
                    order_id.clone(),
                    side,
                    price,
                    size,
                    chrono::Utc::now(),
                );

                self.keystore.record_transaction(amount_usdc).await;
                self.trades.write().await.push(trade);

                info!("✅ {:?} order confirmed: {}", side, sig);

                let count = self.total_executions.load(Ordering::SeqCst);
                if count % self.config.reconcile_balances_every_n_trades.unwrap_or(10) as u64 == 0 {
                    let _ = self.reconcile_balances(price).await;
                }

                Ok(order_id)
            }
            Err(e) => {
                self.failed_executions.fetch_add(1, Ordering::SeqCst);
                error!("❌ Transaction failed: {}", e);
                Err(e)
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // 🪐 JUPITER SWAP INTEGRATION — VersionedTransaction (ALTs preserved)
    // ─────────────────────────────────────────────────────────────────────────
    /// Build a Jupiter swap as a VersionedTransaction.
    ///
    /// Uses JupiterClient::prepare_swap() which calls Jupiter's V6 API and
    /// returns a complete V0 VersionedTransaction — Address Lookup Tables
    /// (ALTs) are fully preserved.  Sign via keystore.sign_versioned_transaction()
    /// and submit via executor.execute_versioned().
    ///
    /// ⚠️  Do NOT decompose into Vec<Instruction> — that silently drops ALTs
    ///    and causes on-chain failures.
    async fn build_jupiter_swap(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
    ) -> Result<(VersionedTransaction, u64)> {
        info!("🪐 Building Jupiter VersionedTransaction...");

        let jupiter = JupiterClient::new(JupiterConfig {
            slippage_bps: self.config.slippage_bps.unwrap_or(50),
            ..Default::default()
        })?
        .with_priority_fee(10_000);

        let (input_mint, output_mint, amount) = match side {
            OrderSide::Buy => {
                // Buy SOL with USDC — USDC has 6 decimals
                let usdc_micro = (price * size * 1_000_000.0) as u64;
                info!("   BUY:  {:.2} USDC → SOL", price * size);
                (USDC_MINT, SOL_MINT, usdc_micro)
            }
            OrderSide::Sell => {
                // Sell SOL for USDC — SOL has 9 decimals (lamports)
                let sol_lamports = (size * 1_000_000_000.0) as u64;
                info!("   SELL: {:.4} SOL → USDC", size);
                (SOL_MINT, USDC_MINT, sol_lamports)
            }
        };

        let user_pubkey = *self.keystore.pubkey();
        let (tx, last_valid, _quote) = jupiter
            .prepare_swap(input_mint, output_mint, amount, user_pubkey)
            .await
            .context("Failed to prepare Jupiter swap")?;

        info!("✅ Jupiter swap tx built (last valid block: {})", last_valid);
        Ok((tx, last_valid))
    }

    async fn reconcile_balances(&self, current_price: f64) -> Result<()> {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        let total_value = usdc + (sol * current_price);

        let initial = self.balance_tracker.initial_balance_usd();
        let pnl = total_value - initial;

        let mut breaker = self.circuit_breaker.write().await;
        breaker.record_trade(pnl, total_value);

        Ok(())
    }

    pub async fn auto_take_profit(&self, current_price: f64) -> Result<()> {
        let roi = self.get_roi(current_price).await;
        let threshold = self.config.profit_take_threshold.unwrap_or(3.0);

        if roi >= threshold {
            let (_, sol) = self.balance_tracker.get_balances().await;
            let ratio = self.config.profit_take_ratio.unwrap_or(0.4);
            let sell_amount = sol * ratio;

            if sell_amount > 0.01 {
                info!("🎯 Auto profit-take: ROI {:.2}% >= {:.2}%", roi, threshold);
                self.sell(current_price, sell_amount).await?;
            }
        }

        Ok(())
    }

    pub async fn get_performance_stats(&self) -> PerformanceStats {
        let trades = self.trades.read().await;

        if trades.is_empty() {
            return PerformanceStats::default();
        }

        let mut stats = PerformanceStats {
            total_trades: trades.len(),
            ..Default::default()
        };

        for trade in trades.iter() {
            stats.total_fees += trade.fees_paid;

            let pnl = trade.net_pnl;
            stats.total_pnl += pnl;

            if pnl > 0.0 {
                stats.winning_trades += 1;
                stats.largest_win = stats.largest_win.max(pnl);
            } else if pnl < 0.0 {
                stats.losing_trades += 1;
                stats.largest_loss = stats.largest_loss.min(pnl);
            }
        }

        let pair_trades = stats.winning_trades + stats.losing_trades;
        if pair_trades > 0 {
            stats.win_rate = (stats.winning_trades as f64 / pair_trades as f64) * 100.0;
        }

        stats
    }

    pub async fn get_roi(&self, current_price: f64) -> f64 {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        let current_value = usdc + (sol * current_price);
        let initial_value = self.balance_tracker.initial_balance_usd();

        if initial_value > 0.0 {
            ((current_value - initial_value) / initial_value) * 100.0
        } else {
            0.0
        }
    }

    pub async fn get_trades(&self) -> Vec<Trade> {
        self.trades.read().await.clone()
    }

    pub async fn get_balances(&self) -> (f64, f64) {
        self.balance_tracker.get_balances().await
    }

    pub async fn display_status(&self, current_price: f64) {
        let (usdc, sol) = self.get_balances().await;
        let roi = self.get_roi(current_price).await;
        let stats = self.get_performance_stats().await;
        let (daily_trades, daily_volume) = self.keystore.get_daily_stats().await;
        let executor_stats = self.executor.read().await.get_stats();

        println!();
        println!("═══════════════════════════════════════════════════════");
        println!("  REAL TRADING ENGINE V2.0 - STATUS");
        println!("═══════════════════════════════════════════════════════");
        println!();
        println!("💰 Balances:");
        println!("   USDC:  ${:.2}", usdc);
        println!("   SOL:   {:.4} SOL (${:.2})", sol, sol * current_price);
        println!("   Total: ${:.2}", usdc + (sol * current_price));
        println!();
        println!("📊 Performance:");
        println!("   ROI:         {:.2}%", roi);
        println!("   Total P&L:   ${:.2}", stats.total_pnl);
        println!("   Total Fees:  ${:.2}", stats.total_fees);
        println!("   Win Rate:    {:.1}%", stats.win_rate);
        println!("   Trades:      {} ({} wins, {} losses)",
            stats.total_trades, stats.winning_trades, stats.losing_trades);
        println!();
        println!("📈 Today:");
        println!("   Trades:  {}", daily_trades);
        println!("   Volume:  ${:.2}", daily_volume);
        println!();
        println!("⚡ Executor:");
        println!("   Success Rate: {:.1}%", executor_stats.success_rate);
        println!("   Total Executions: {}", executor_stats.total_executions);
        println!();

        println!("🚦 Circuit Breaker:");
        let breaker = self.circuit_breaker.read().await;
        let status = breaker.status();

        if status.is_tripped {
            println!("   Status:  🚨 TRIPPED");
            if let Some(reason) = status.trip_reason {
                println!("   Reason:  {:?}", reason);
            }
            if let Some(cooldown) = status.cooldown_remaining {
                println!("   Cooldown: {}s", cooldown.as_secs());
            }
        } else {
            println!("   Status:  ✅ OK");
        }

        println!();
        println!("💻 Current SOL Price: ${:.4}", current_price);
        println!("═══════════════════════════════════════════════════════");
        println!();
    }

    pub async fn emergency_shutdown(&self, reason: &str) -> Result<()> {
        error!("🚨 EMERGENCY SHUTDOWN: {}", reason);
        self.emergency_shutdown.store(true, Ordering::SeqCst);

        let mut breaker = self.circuit_breaker.write().await;
        breaker.force_trip(TripReason::MaxDrawdown);

        self.display_status(0.0).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let config = RealTradingConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_slippage_validation() {
        let mut config = RealTradingConfig::default();

        // Valid slippage
        config.slippage_bps = Some(50);
        assert!(config.validate().is_ok());

        // Invalid slippage (too high)
        config.slippage_bps = Some(2000);
        assert!(config.validate().is_err());
    }
}
