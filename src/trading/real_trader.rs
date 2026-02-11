//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥 REAL TRADER ENGINE V3.0 - MEV-PROTECTED! 🛡️
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! **V3.0 ENHANCEMENTS:**
//! ✅ Optional MEV Protection (Phase 5 Complete!)
//! ✅ Dynamic priority fee optimization
//! ✅ Pre-trade slippage validation
//! ✅ Jito bundle support (optional)
//! ✅ Backward compatible (MEV is opt-in)
//! ✅ Enhanced metrics tracking
//!
//! February 11, 2026 - Phase 5: MEV Protection Integrated!
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Context, Result};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::str::FromStr;
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
use super::jupiter_swap::{JupiterSwapClient, WSOL_MINT, USDC_MINT};
use super::mev_protection::MevProtectionConfig; // 🛡️ NEW!
use solana_sdk::{instruction::Instruction, pubkey::Pubkey};

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
    
    /// 🛡️ NEW: Optional MEV Protection (V3.0)
    /// If Some(), enables:
    /// - Dynamic priority fee optimization
    /// - Slippage validation before trades
    /// - Optional Jito bundle support
    pub mev_protection: Option<MevProtectionConfig>,
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
            mev_protection: None, // 🛡️ Disabled by default (opt-in)
        }
    }
}

impl RealTradingConfig {
    /// Create config with MEV protection enabled
    pub fn with_mev_protection(mut self, mev_config: MevProtectionConfig) -> Self {
        self.mev_protection = Some(mev_config);
        self
    }
    
    /// Create conservative config with MEV protection
    pub fn conservative_with_mev() -> Self {
        Self::default().with_mev_protection(MevProtectionConfig::conservative())
    }

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
        
        // 🛡️ Validate MEV config if present
        if let Some(ref mev) = self.mev_protection {
            mev.validate()?;
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
    initial_balance: f64,
}

impl BalanceTracker {
    fn new(initial_usdc: f64, initial_sol: f64) -> Self {
        Self {
            expected_usdc: Arc::new(RwLock::new(initial_usdc)),
            expected_sol: Arc::new(RwLock::new(initial_sol)),
            initial_balance: initial_usdc + (initial_sol * 190.0),  // Estimate
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

    fn initial_balance(&self) -> f64 {
        self.initial_balance
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
    
    /// 🛡️ NEW: MEV protection stats (V3.0)
    pub mev_protected_trades: usize,
    pub slippage_rejections: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// 🔥 REAL TRADING ENGINE (V3.0 - MEV-PROTECTED!)
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
    slippage_rejections: Arc<AtomicU64>, // 🛡️ Track slippage rejections
}

impl RealTradingEngine {
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
    ) -> Result<Self> {
        info!("🚀 Initializing Real Trading Engine V3.0 (MEV-Protected!)");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        
        // 🛡️ Create executor with optional MEV protection
        let executor = if let Some(ref mev_config) = config.mev_protection {
            info!("🛡️  MEV Protection: ENABLED");
            Arc::new(RwLock::new(
                TransactionExecutor::new(config.executor.clone())?
                    .with_mev_protection(mev_config.clone())?
            ))
        } else {
            info!("🛡️  MEV Protection: DISABLED (enable with .with_mev_protection())");
            Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?))
        };

        // Use Config for CircuitBreaker
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_balance_usdc)
        ));

        let balance_tracker = Arc::new(BalanceTracker::new(initial_balance_usdc, initial_balance_sol));

        info!("✅ Real Trading Engine initialized");
        info!("   Wallet: {}", keystore.pubkey());

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
            slippage_rejections: Arc::new(AtomicU64::new(0)),
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
        info!("📋 {:?} order: {:.4} SOL @ ${:.2}", side, size, price);

        self.total_executions.fetch_add(1, Ordering::SeqCst);

        // 🔥 BUILD ACTUAL JUPITER SWAP!
        let (swap_instructions, quote_price) = self.build_jupiter_swap(side, price, size).await?;
        
        // 🛡️ VALIDATE SLIPPAGE (if MEV protection enabled)
        let executor = self.executor.read().await;
        if executor.is_mev_protected() {
            let slippage_ok = executor.validate_slippage(price, quote_price)?;
            
            if !slippage_ok {
                self.slippage_rejections.fetch_add(1, Ordering::SeqCst);
                bail!(
                    "🛡️  Trade rejected: Slippage too high! Expected ${:.4}, got ${:.4}",
                    price, quote_price
                );
            }
            
            info!("🛡️  Slippage validated: ${:.4} -> ${:.4} (✅ OK)", price, quote_price);
        }
        
        // Drop read lock before executing
        drop(executor);
        let executor = self.executor.write().await;

        let signature = executor.execute(
            self.keystore.pubkey(),
            swap_instructions,
            |tx| self.keystore.sign_transaction(tx),
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

    // ─────────────────────────────────────────────────────────────────────────────
    // 🪐 JUPITER SWAP INTEGRATION - THE MONEY MAKER!
    // ─────────────────────────────────────────────────────────────────────────────
    /// Build Jupiter swap transaction and return (instructions, actual_quote_price)
    async fn build_jupiter_swap(
        &self,
        side: OrderSide,
        _expected_price: f64,
        size: f64,
    ) -> Result<(Vec<Instruction>, f64)> {
        info!("🪐 Building Jupiter swap transaction...");

        // Initialize Jupiter client with slippage from config
        let slippage_bps = self.config.slippage_bps.unwrap_or(50);
        let jupiter = JupiterSwapClient::new(slippage_bps)?
            .with_priority_fee(5000);

        let wsol = Pubkey::from_str(WSOL_MINT)?;
        let usdc = Pubkey::from_str(USDC_MINT)?;

        // Determine swap direction and amounts
        let (input_mint, output_mint, amount_lamports) = match side {
            OrderSide::Buy => {
                // Buy SOL with USDC
                let usdc_amount = (_expected_price * size * 1_000_000.0) as u64; // USDC has 6 decimals
                (usdc, wsol, usdc_amount)
            }
            OrderSide::Sell => {
                // Sell SOL for USDC  
                let sol_lamports = (size * 1_000_000_000.0) as u64; // SOL has 9 decimals
                (wsol, usdc, sol_lamports)
            }
        };

        debug!("   Input:  {} ({})", input_mint, amount_lamports);
        debug!("   Output: {}", output_mint);

        // Get quote from Jupiter
        let quote = jupiter
            .get_quote(input_mint, output_mint, amount_lamports)
            .await
            .context("Failed to get Jupiter quote")?;

        // Calculate actual price from quote
        // NOTE: quote.out_amount is a String from Jupiter API, needs parsing
        let quote_price = match side {
            OrderSide::Buy => {
                // Price = USDC spent / SOL received
                let usdc_spent = amount_lamports as f64 / 1_000_000.0;
                let out_amount = quote.out_amount.parse::<u64>()
                    .context("Failed to parse quote.out_amount")?;
                let sol_received = out_amount as f64 / 1_000_000_000.0;
                usdc_spent / sol_received
            }
            OrderSide::Sell => {
                // Price = USDC received / SOL sold
                let sol_sold = amount_lamports as f64 / 1_000_000_000.0;
                let out_amount = quote.out_amount.parse::<u64>()
                    .context("Failed to parse quote.out_amount")?;
                let usdc_received = out_amount as f64 / 1_000_000.0;
                usdc_received / sol_sold
            }
        };

        info!("   Quote received: {} → {} lamports", amount_lamports, quote.out_amount);
        info!("   Price impact: {:.3}%", quote.price_impact_pct);
        info!("   Effective price: ${:.4}", quote_price);

        // Get swap transaction
        let (versioned_tx, _last_valid_height) = jupiter
            .get_swap_transaction(&quote, *self.keystore.pubkey())
            .await
            .context("Failed to get swap transaction")?;

        // Extract instructions from versioned transaction
        let message = versioned_tx.message;
        let instructions: Vec<Instruction> = match message {
            solana_sdk::message::VersionedMessage::Legacy(msg) => {
                msg.instructions
                    .into_iter()
                    .map(|ix| Instruction {
                        program_id: msg.account_keys[ix.program_id_index as usize],
                        accounts: ix
                            .accounts
                            .into_iter()
                            .map(|idx| solana_sdk::instruction::AccountMeta {
                                pubkey: msg.account_keys[idx as usize],
                                is_signer: false,
                                is_writable: false,
                            })
                            .collect(),
                        data: ix.data,
                    })
                    .collect()
            }
            solana_sdk::message::VersionedMessage::V0(msg) => {
                msg.instructions
                    .into_iter()
                    .map(|ix| {
                        let account_keys = msg.account_keys.clone();
                        Instruction {
                            program_id: account_keys[ix.program_id_index as usize],
                            accounts: ix
                                .accounts
                                .into_iter()
                                .map(|idx| solana_sdk::instruction::AccountMeta {
                                    pubkey: account_keys[idx as usize],
                                    is_signer: false,
                                    is_writable: false,
                                })
                                .collect(),
                            data: ix.data,
                        }
                    })
                    .collect()
            }
        };

        info!("✅ Jupiter swap transaction built ({} instructions)", instructions.len());

        Ok((instructions, quote_price))
    }

    async fn reconcile_balances(&self, current_price: f64) -> Result<()> {
        debug!("🔄 Reconciling balances...");

        let (usdc, sol) = self.balance_tracker.get_balances().await;
        let total_value = usdc + (sol * current_price);

        let initial = self.balance_tracker.initial_balance();
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
        
        // 🛡️ Add MEV protection stats
        let executor = self.executor.read().await;
        let exec_stats = executor.get_stats();
        stats.mev_protected_trades = exec_stats.mev_protected_executions as usize;
        stats.slippage_rejections = self.slippage_rejections.load(Ordering::SeqCst) as usize;

        stats
    }

    pub async fn get_roi(&self, current_price: f64) -> f64 {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        let current_value = usdc + (sol * current_price);
        let initial_value = self.balance_tracker.initial_balance();

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
        let is_mev_protected = self.executor.read().await.is_mev_protected();

        println!();
        println!("═══════════════════════════════════════════════════════");
        println!("  REAL TRADING ENGINE V3.0 - STATUS {}",
            if is_mev_protected { "🛡️  [MEV-PROTECTED]" } else { "" });
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
        
        if is_mev_protected {
            println!("🛡️  MEV Protection:");
            println!("   Protected Trades:     {}", stats.mev_protected_trades);
            println!("   Slippage Rejections:  {}", stats.slippage_rejections);
            println!();
        }
        
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
    
    /// Check if MEV protection is enabled
    pub fn is_mev_protected(&self) -> bool {
        // We can't await here, so return based on config
        self.config.mev_protection.is_some()
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
    
    #[test]
    fn test_mev_config() {
        // Default config (no MEV)
        let config = RealTradingConfig::default();
        assert!(config.mev_protection.is_none());
        
        // Conservative MEV config
        let mev_config = RealTradingConfig::conservative_with_mev();
        assert!(mev_config.mev_protection.is_some());
    }
}
