//! =============================================================================
//! REAL TRADER ENGINE V2.5 - MODULAR & BULLETPROOF
//!
//! V2.2 CHANGES (fix/live-mode-circuit-breaker-wallet-noise):
//! ✅ CircuitBreaker::with_balance() now receives full portfolio NAV
//!    (USDC + SOL*price) instead of only initial_usdc (was 0.0 for SOL wallets).
//!    Peak-balance drawdown and daily-loss maths are now correct.
//! ✅ process_price_update() ticks is_trading_allowed() before reconcile
//!    so the cooldown reset fires even when fills == 0 (i.e. no execute_trade
//!    calls are made).  Previously the breaker could stay tripped forever.
//! ✅ get_wallet() uses VirtualWallet::new_silent() — stops the double
//!    "[WALLET] Initialized" log line firing on every price cycle.
//!
//! V2.3 CHANGES (fix/jupiter-client-wiring — Mar 2026):
//! ✅ Import path changed: super::jupiter_client → crate::dex::jupiter_client
//!    Now uses production JupiterClient V4.0 with full API key support.
//!    Old stub that caused all swap failures has been obliterated.
//!
//! V2.4 CHANGES (fix/real-trader-api-mismatch):
//! ✅ build_jupiter_swap() now uses JupiterClient::simple_swap() API.
//!    Constructor changed from JupiterConfig struct to 6 explicit args.
//!    with_priority_fee() now takes (lamports, level) instead of just lamports.
//!    Stub that prevented real swaps from compiling has been eradicated.
//!
//! V2.5 CHANGES (fix/dex-module-exports — Mar 2026 SECURITY):
//! ✅ JupiterClient::new() now accepts Pubkey instead of Keypair (security!).
//!    Removed broken line: Keypair::from_bytes(keystore.export_keypair()).
//!    Signing remains in keystore — keypair never leaves SecureKeystore.
//!    Root cause: export_keypair() doesn't exist and SHOULD NOT exist.
//! =============================================================================

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::str::FromStr;
use tokio::sync::RwLock;

// -----------------------------------------------------------------------------
// MODULAR IMPORTS
// -----------------------------------------------------------------------------
use crate::security::keystore::{SecureKeystore, KeystoreConfig};
use crate::risk::circuit_breaker::{CircuitBreaker, TripReason};
use crate::Config;
use super::executor::{TransactionExecutor, ExecutorConfig};
use super::trade::Trade;
use super::paper_trader::{Order, OrderSide, VirtualWallet, PerformanceStats as PaperPerformanceStats};
use crate::dex::{JupiterClient, SOL_MINT, USDC_MINT};
use super::{TradingEngine, TradingResult, FillEvent};
use solana_sdk::{
    transaction::VersionedTransaction,
    pubkey::Pubkey,
};

// -----------------------------------------------------------------------------
// CONFIGURATION
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTradingConfig {
    pub keystore: KeystoreConfig,
    pub executor: ExecutorConfig,
    pub slippage_bps: Option<u16>,
    pub max_trade_size_usdc: Option<f64>,  // 🆕 V5.4 (PR #44): Dual safety cap
    pub circuit_breaker_loss_pct: Option<f64>,
    pub stop_loss_pct: Option<f64>,
    pub profit_take_threshold: Option<f64>,
    pub profit_take_ratio: Option<f64>,
    pub maker_fee_bps: Option<f64>,
    pub taker_fee_bps: Option<f64>,
    pub reconcile_balances_every_n_trades: Option<u32>,
    pub jupiter_api_key: Option<String>,  // 🆕 V2.4: Jupiter API key
    pub rpc_url: Option<String>,           // 🆕 V2.4: RPC URL
}

impl Default for RealTradingConfig {
    fn default() -> Self {
        Self {
            keystore: KeystoreConfig::default(),
            executor: ExecutorConfig::default(),
            slippage_bps: Some(50),
            max_trade_size_usdc: Some(250.0),  // 🆕 V5.4 (PR #44)
            circuit_breaker_loss_pct: Some(5.0),
            stop_loss_pct: Some(10.0),
            profit_take_threshold: Some(3.0),
            profit_take_ratio: Some(0.4),
            maker_fee_bps: Some(2.0),
            taker_fee_bps: Some(4.0),
            reconcile_balances_every_n_trades: Some(10),
            jupiter_api_key: None,
            rpc_url: Some("https://api.mainnet-beta.solana.com".to_string()),
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

    /// Bridge the master TOML [execution] block into this engine config.
    /// Called by gridz_bot.rs when execution_mode = "live" to ensure the
    /// master TOML's slippage setting reaches the real trading engine.
    /// Remaining fields use sensible defaults from Default::default().
    pub fn from_execution_config(exec: &crate::config::ExecutionConfig) -> Self {
        Self {
            slippage_bps: Some(exec.max_slippage_bps),
            max_trade_size_usdc: Some(exec.max_trade_size_usdc),  // 🆕 V5.4 (PR #44)
            ..Default::default()
        }
    }
}

// -----------------------------------------------------------------------------
// BALANCE TRACKER
// -----------------------------------------------------------------------------
struct BalanceTracker {
    expected_usdc: Arc<RwLock<f64>>,
    expected_sol: Arc<RwLock<f64>>,
    /// Initial portfolio value in USD, calculated at boot using the live
    /// SOL price from the Pyth feed -- never a hardcoded estimate.
    initial_balance_usd: f64,
}

impl BalanceTracker {
    /// Create a new balance tracker.
    ///
    /// `sol_price_usd` must be the live SOL/USD price fetched from the
    /// price feed at engine initialisation -- e.g. `feed.latest_price().await`.
    /// Do NOT pass a hardcoded value.
    fn new(initial_usdc: f64, initial_sol: f64, sol_price_usd: f64) -> Self {
        Self {
            expected_usdc: Arc::new(RwLock::new(initial_usdc)),
            expected_sol: Arc::new(RwLock::new(initial_sol)),
            initial_balance_usd: initial_usdc + (initial_sol * sol_price_usd),
        }
    }

    async fn get_balances(&self) -> (f64, f64) {
        let usdc = *self.expected_usdc.read().await;
        let sol  = *self.expected_sol.read().await;
        (usdc, sol)
    }

    #[allow(dead_code)]
    async fn update(&self, usdc: f64, sol: f64) {
        *self.expected_usdc.write().await = usdc;
        *self.expected_sol.write().await  = sol;
    }

    fn initial_balance_usd(&self) -> f64 {
        self.initial_balance_usd
    }
}

// -----------------------------------------------------------------------------
// PERFORMANCE STATS
// -----------------------------------------------------------------------------
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PerformanceStats {
    pub total_trades:   usize,
    pub winning_trades: usize,
    pub losing_trades:  usize,
    pub total_pnl:      f64,
    pub total_fees:     f64,
    pub win_rate:       f64,
    pub avg_win:        f64,
    pub avg_loss:       f64,
    pub largest_win:    f64,
    pub largest_loss:   f64,
    pub profit_factor:  f64,
}

// -----------------------------------------------------------------------------
// REAL TRADING ENGINE
// -----------------------------------------------------------------------------
pub struct RealTradingEngine {
    keystore:              Arc<SecureKeystore>,
    executor:              Arc<RwLock<TransactionExecutor>>,
    circuit_breaker:       Arc<RwLock<CircuitBreaker>>,
    balance_tracker:       Arc<BalanceTracker>,
    config:                RealTradingConfig,
    trades:                Arc<RwLock<Vec<Trade>>>,
    _open_orders:          Arc<RwLock<HashMap<String, Order>>>,
    next_id:               Arc<AtomicU64>,
    total_executions:      Arc<AtomicU64>,
    successful_executions: Arc<AtomicU64>,
    failed_executions:     Arc<AtomicU64>,
    emergency_shutdown:    Arc<AtomicBool>,
}

impl RealTradingEngine {
    /// Construct the real trading engine.
    ///
    /// `initial_sol_price_usd` must be the live SOL/USD price from the
    /// Pyth price feed at the time of construction -- e.g.
    /// `feed.latest_price().await`.  This is used to compute the initial
    /// portfolio NAV and, from it, the accurate ROI throughout the session.
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
        initial_sol_price_usd: f64,
    ) -> Result<Self> {
        info!("[RealEngine] Initializing V2.5");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        let executor = Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?));

        // Pass full portfolio NAV so peak_balance, drawdown, and daily-loss
        // calculations are correct even when USDC balance is zero.
        let initial_nav = initial_balance_usdc + (initial_balance_sol * initial_sol_price_usd);
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_nav)
        ));

        let balance_tracker = Arc::new(BalanceTracker::new(
            initial_balance_usdc,
            initial_balance_sol,
            initial_sol_price_usd,
        ));

        info!("[RealEngine] Initialized V2.5");
        info!("  Wallet : {}",        keystore.pubkey());
        info!("  NAV    : ${:.2} (SOL @ ${:.4})",
            balance_tracker.initial_balance_usd(), initial_sol_price_usd);

        Ok(Self {
            keystore,
            executor,
            circuit_breaker,
            balance_tracker,
            config,
            trades:                Arc::new(RwLock::new(Vec::new())),
            _open_orders:          Arc::new(RwLock::new(HashMap::new())),
            next_id:               Arc::new(AtomicU64::new(1)),
            total_executions:      Arc::new(AtomicU64::new(0)),
            successful_executions: Arc::new(AtomicU64::new(0)),
            failed_executions:     Arc::new(AtomicU64::new(0)),
            emergency_shutdown:    Arc::new(AtomicBool::new(false)),
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
            bail!("[RealEngine] EMERGENCY SHUTDOWN ACTIVE");
        }

        if !self.circuit_breaker.write().await.is_trading_allowed() {
            bail!("[RealEngine] CIRCUIT BREAKER ACTIVE");
        }

        let amount_usdc = price * size;
        
        // 🆕 V5.4 (PR #44): DUAL CAP ENFORCEMENT
        // Check max_trade_size_usdc before keystore validation.
        // Whichever cap hits first (max_trade_sol or max_trade_size_usdc) blocks the trade.
        if let Some(max_usdc) = self.config.max_trade_size_usdc {
            if amount_usdc > max_usdc {
                bail!(
                    "[RealEngine] Trade blocked: ${:.2} exceeds max_trade_size_usdc=${:.2}",
                    amount_usdc, max_usdc
                );
            }
        }
        
        self.keystore.validate_transaction(amount_usdc).await?;

        let order_id = format!("REAL-{:06}", self.next_id.fetch_add(1, Ordering::SeqCst));
        info!("[Order] {:?} {:.4} SOL @ ${:.2}", side, size, price);

        self.total_executions.fetch_add(1, Ordering::SeqCst);

        let (versioned_tx, _last_valid) = self.build_jupiter_swap(side, price, size).await?;

        let executor  = self.executor.write().await;
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

                info!("[Order] {:?} confirmed: {}", side, sig);

                let count = self.total_executions.load(Ordering::SeqCst);
                if count % self.config.reconcile_balances_every_n_trades.unwrap_or(10) as u64 == 0 {
                    let _ = self.reconcile_balances(price).await;
                }

                Ok(order_id)
            }
            Err(e) => {
                self.failed_executions.fetch_add(1, Ordering::SeqCst);
                error!("[Order] Transaction failed: {}", e);
                Err(e)
            }
        }
    }

    async fn build_jupiter_swap(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
    ) -> Result<(VersionedTransaction, u64)> {
        info!("[Jupiter] Building VersionedTransaction V4.1 (secure)...");

        // Parse mint addresses
        let sol_mint_pubkey = Pubkey::from_str(SOL_MINT)
            .context("Failed to parse SOL_MINT")?;
        let usdc_mint_pubkey = Pubkey::from_str(USDC_MINT)
            .context("Failed to parse USDC_MINT")?;

        // V2.5 SECURITY: Pass public key only — signing happens via keystore.sign_versioned_transaction()
        let wallet_pubkey = self.keystore.pubkey();

        // Initial capital (doesn't matter for single swaps, but JupiterClient needs it)
        let (usdc_balance, sol_balance) = self.balance_tracker.get_balances().await;
        let initial_capital = usdc_balance + (sol_balance * price);

        // Get RPC URL and API key from config
        let rpc_url = self.config.rpc_url.clone()
            .unwrap_or_else(|| "https://api.mainnet-beta.solana.com".to_string());
        let jupiter_api_key = self.config.jupiter_api_key.clone()
            .ok_or_else(|| anyhow::anyhow!("jupiter_api_key not configured"))?;

        // Create Jupiter client with production API V4.1 (secure: accepts Pubkey)
        let jupiter = JupiterClient::new(
            rpc_url,
            wallet_pubkey,
            sol_mint_pubkey,
            usdc_mint_pubkey,
            initial_capital,
            jupiter_api_key,
        )?
        .with_slippage(self.config.slippage_bps.unwrap_or(50))
        .with_priority_fee(10_000, "high".to_string());

        // Determine swap direction and amount
        let (input_mint, output_mint, amount) = match side {
            OrderSide::Buy => {
                // Buy SOL with USDC
                let usdc_micro = (price * size * 1_000_000.0) as u64;
                info!("  BUY:  {:.2} USDC → SOL", price * size);
                (usdc_mint_pubkey, sol_mint_pubkey, usdc_micro)
            }
            OrderSide::Sell => {
                // Sell SOL for USDC
                let sol_lamports = (size * 1_000_000_000.0) as u64;
                info!("  SELL: {:.4} SOL → USDC", size);
                (sol_mint_pubkey, usdc_mint_pubkey, sol_lamports)
            }
        };

        // Call simple_swap() to get unsigned VersionedTransaction
        let (tx, last_valid) = jupiter
            .simple_swap(input_mint, output_mint, amount)
            .await
            .context("Failed to build Jupiter swap")?;

        info!("[Jupiter] Swap tx built (last valid block: {})", last_valid);
        Ok((tx, last_valid))
    }

    async fn reconcile_balances(&self, current_price: f64) -> Result<()> {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        let total_value = usdc + (sol * current_price);
        let initial     = self.balance_tracker.initial_balance_usd();
        let pnl         = total_value - initial;

        let mut breaker = self.circuit_breaker.write().await;
        breaker.record_trade(pnl, total_value);

        Ok(())
    }

    pub async fn auto_take_profit(&self, current_price: f64) -> Result<()> {
        let roi       = self.get_roi(current_price).await;
        let threshold = self.config.profit_take_threshold.unwrap_or(3.0);

        if roi >= threshold {
            let (_, sol) = self.balance_tracker.get_balances().await;
            let ratio        = self.config.profit_take_ratio.unwrap_or(0.4);
            let sell_amount  = sol * ratio;

            if sell_amount > 0.01 {
                info!("[ProfitTake] ROI {:.2}% >= {:.2}% -- taking profit", roi, threshold);
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
        let (usdc, sol)   = self.balance_tracker.get_balances().await;
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
        let roi         = self.get_roi(current_price).await;
        let stats       = self.get_performance_stats().await;
        let (daily_trades, daily_volume) = self.keystore.get_daily_stats().await;
        let executor_stats = self.executor.read().await.get_stats();

        println!();
        println!("=======================================================");
        println!("  REAL TRADING ENGINE V2.5 - STATUS");
        println!("=======================================================");
        println!();
        println!("Balances:");
        println!("  USDC : ${:.2}",           usdc);
        println!("  SOL  : {:.4} (${:.2})",   sol, sol * current_price);
        println!("  Total: ${:.2}",            usdc + (sol * current_price));
        println!();
        println!("Performance:");
        println!("  ROI        : {:.2}%",      roi);
        println!("  Total P&L  : ${:.2}",      stats.total_pnl);
        println!("  Total Fees : ${:.2}",      stats.total_fees);
        println!("  Win Rate   : {:.1}%",      stats.win_rate);
        println!("  Trades     : {} ({} wins, {} losses)",
            stats.total_trades, stats.winning_trades, stats.losing_trades);
        println!();
        println!("Today:");
        println!("  Trades : {}",              daily_trades);
        println!("  Volume : ${:.2}",          daily_volume);
        println!();
        println!("Executor:");
        println!("  Success Rate    : {:.1}%", executor_stats.success_rate);
        println!("  Total Exec      : {}",     executor_stats.total_executions);
        println!();
        println!("Circuit Breaker:");
        let breaker = self.circuit_breaker.read().await;
        let status  = breaker.status();
        if status.is_tripped {
            println!("  Status  : TRIPPED");
            if let Some(reason) = status.trip_reason {
                println!("  Reason  : {:?}", reason);
            }
            if let Some(cooldown) = status.cooldown_remaining {
                println!("  Cooldown: {}s", cooldown.as_secs());
            }
        } else {
            println!("  Status  : OK");
        }
        println!();
        println!("SOL Price: ${:.4}", current_price);
        println!("=======================================================");
        println!();
    }

    /// Trigger an emergency shutdown: sets the atomic flag, trips the
    /// circuit breaker, and dumps a status snapshot.  Named
    /// `trigger_emergency_shutdown` to avoid collision with the
    /// `TradingEngine::emergency_shutdown` trait method below.
    pub async fn trigger_emergency_shutdown(&self, reason: &str) -> Result<()> {
        error!("[RealEngine] EMERGENCY SHUTDOWN: {}", reason);
        self.emergency_shutdown.store(true, Ordering::SeqCst);
        let mut breaker = self.circuit_breaker.write().await;
        breaker.force_trip(TripReason::MaxDrawdown);
        drop(breaker);
        self.display_status(0.0).await;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// UNIFIED TRADING ENGINE TRAIT IMPLEMENTATION
// -----------------------------------------------------------------------------

#[async_trait]
impl TradingEngine for RealTradingEngine {
    /// Maps a grid level-crossing signal to a Jupiter atomic swap.
    /// `grid_level_id` is logged for observability but not stored --
    /// Jupiter swaps are atomic and cannot be cancelled by order ID.
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String> {
        if let Some(level) = grid_level_id {
            info!("[Grid] Level {} triggered {:?} @ ${:.4}", level, side, price);
        }
        self.execute_trade(side, price, size).await
    }

    /// Jupiter swaps are atomic -- there are no pending orders to cancel.
    /// Returns Ok(()) and logs a warning for observability.
    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        log::warn!(
            "[RealEngine] cancel_order('{}') -- Jupiter swaps are atomic; nothing to cancel",
            order_id
        );
        Ok(())
    }

    /// Jupiter swaps are atomic -- always returns 0 orders cancelled.
    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        log::warn!(
            "[RealEngine] cancel_all_orders() -- Jupiter swaps are atomic; 0 cancelled"
        );
        Ok(0)
    }

    /// Reconcile expected balances against circuit breaker thresholds.
    ///
    /// Ticks `is_trading_allowed()` first so the cooldown reset fires on
    /// every cycle, not just when a trade is attempted.  Without this tick
    /// the breaker would stay permanently tripped when fills == 0.
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        // Tick the cooldown gate so is_tripped resets after cooldown expires
        // even when no trades are being placed (fills == 0).
        let _ = self.circuit_breaker.write().await.is_trading_allowed();
        self.reconcile_balances(current_price).await?;
        Ok(vec![])
    }

    /// Jupiter swaps are atomic -- there are never open orders in flight.
    async fn open_order_count(&self) -> usize {
        0
    }

    /// Returns false if either the emergency shutdown flag is set
    /// or the circuit breaker has tripped.
    async fn is_trading_allowed(&self) -> bool {
        if self.emergency_shutdown.load(Ordering::SeqCst) {
            return false;
        }
        self.circuit_breaker.write().await.is_trading_allowed()
    }

    /// Override the trait default to correctly trip the circuit breaker
    /// and set the atomic halt flag (not just cancel_all_orders).
    async fn emergency_shutdown(&self, reason: &str) -> TradingResult<()> {
        self.trigger_emergency_shutdown(reason).await
    }

    // ── V5.2.2 / V2.5: Wallet and performance queries ────────────────
    async fn get_wallet(&self) -> VirtualWallet {
        // Use new_silent to avoid logging "[WALLET] Initialized" on every
        // price cycle.  new() is only appropriate at session start.
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        VirtualWallet::new_silent(usdc, sol)
    }

    async fn get_performance_stats(&self) -> PaperPerformanceStats {
        // Map RealPerformanceStats → PaperPerformanceStats
        let stats = self.get_performance_stats().await;
        PaperPerformanceStats {
            total_trades: stats.total_trades,
            winning_trades: stats.winning_trades,
            losing_trades: stats.losing_trades,
            total_pnl: stats.total_pnl,
            total_fees: stats.total_fees,
            win_rate: stats.win_rate,
            avg_win: stats.avg_win,
            avg_loss: stats.avg_loss,
            largest_win: stats.largest_win,
            largest_loss: stats.largest_loss,
            profit_factor: stats.profit_factor,
        }
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
        config.slippage_bps = Some(50);
        assert!(config.validate().is_ok());
        config.slippage_bps = Some(2000);
        assert!(config.validate().is_err());
    }
}
