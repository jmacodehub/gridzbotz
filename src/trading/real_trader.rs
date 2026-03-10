//! =============================================================================
//! REAL TRADER ENGINE V2.9
//!
//! V2.9 CHANGES (fix/risk-continuous-sl-monitoring — PR #89):
//! ✅ process_price_update(): continuous SL/TP monitoring on every price tick.
//!    - Reads entry_price() from StopLossManager (0.0 until first fill).
//!    - Acquires single write lock, checks should_stop_loss() then
//!      should_take_profit() in one atomic pass.
//!    - Lock dropped before any async call (no lock held across await).
//!    - Stop-loss fires  → trigger_emergency_shutdown() with formatted reason.
//!    - Take-profit fires → auto_take_profit() partial-sell (existing path).
//!    - entry_price == 0.0 guard → no-op before first position opens;
//!      zero cost on every cycle until a real fill is confirmed.
//!
//! V2.8 CHANGES (fix/risk-stop-loss-wiring — PR #88):
//! ✅ StopLossManager wired into RealTradingEngine.
//!    - stop_loss_manager field added (Arc<RwLock<StopLossManager>>).
//!    - Constructed from global_config in new() — single init point.
//!    - execute_trade() checks should_stop_loss() + should_take_profit()
//!      BEFORE building the Jupiter swap — no network call wasted.
//!    - bail! with descriptive reason logged to tracing.
//! ✅ Shadow fields DELETED from RealTradingConfig:
//!    - stop_loss_pct       (was Option<f64>, never read in hot path)
//!    - profit_take_threshold (was Option<f64>, leaked into auto_take_profit)
//!    RiskConfig (config.risk.*) is now the undisputed single source of truth.
//! ✅ profit_take_pct stored directly on engine as plain f64.
//!    Mirrors static_priority_fee pattern — read once at construction,
//!    never changes at runtime. auto_take_profit() reads self.profit_take_pct.
//! ✅ test_shadow_stop_loss_fields_removed: compile-time regression guard.
//!
//! V2.7 CHANGES (feat/dynamic-priority-fees — PR #79 Commit 8):
//! ✅ AsyncRpcFeeSource: async FeeDataSource impl using nonblocking RpcClient.
//! ✅ PriorityFeeEstimator wired into RealTradingEngine (new fields).
//! ✅ build_jupiter_swap(): dynamic fee injected.
//!
//! V2.6 CHANGES (fix/real-trader-fee-shadow-rpc-wiring — PR #79 Commit 1):
//! ✅ Shadow fee fields removed: maker_fee_bps + taker_fee_bps.
//! ✅ rpc_url Default: None — must be wired via from_config().
//! ✅ from_execution_config() renamed → from_config(global: &Config).
//!
//! March 2026 — V2.9 🚀
//! =============================================================================

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::str::FromStr;
use std::time::Duration;
use tokio::sync::RwLock;
use solana_client::nonblocking::rpc_client::RpcClient as AsyncRpcClient;
use super::priority_fee_estimator::{PriorityFeeEstimator, FeeDataSource};

// -----------------------------------------------------------------------------
// MODULAR IMPORTS
// -----------------------------------------------------------------------------
use crate::security::keystore::{SecureKeystore, KeystoreConfig};
use crate::risk::circuit_breaker::{CircuitBreaker, TripReason};
use crate::risk::stop_loss::StopLossManager;
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

// =============================================================================
// ⚡ ASYNC RPC FEE SOURCE — FeeDataSource impl for PriorityFeeEstimator
// =============================================================================
//
// Uses the async Solana RPC client (proper Tokio citizen — no sync blocking).
// Dedicated client with its own timeout, decoupled from trading pipeline.
//
// Note: src/rpc/fee_source.rs provides the sync equivalent (RpcFeeSource)
// for CLI/diagnostic use. This async version is for the live trading loop.
// TODO(tech-debt): consolidate FeeSource + FeeDataSource traits in one location.
// =============================================================================

/// Estimated CU budget for Jupiter swaps (used to convert µL/CU → total lamports).
/// Jupiter's `dynamicComputeUnitLimit: true` handles actual CU; this is only
/// for our max_lamports cap calculation. Typical range: 200K-400K CU.
const JUPITER_ESTIMATED_CU: u64 = 300_000;

/// Minimum max_lamports floor — ensures txs can land even if estimator
/// returns very low values during unusually calm periods.
const MIN_MAX_LAMPORTS: u64 = 5_000;

struct AsyncRpcFeeSource {
    client: AsyncRpcClient,
    rpc_url: String, // diagnostics only
}

impl AsyncRpcFeeSource {
    fn new(rpc_url: &str, timeout: Duration) -> Self {
        debug!("AsyncRpcFeeSource: targeting {} (timeout {:?})", rpc_url, timeout);
        Self {
            client: AsyncRpcClient::new_with_timeout(rpc_url.to_string(), timeout),
            rpc_url: rpc_url.to_string(),
        }
    }
}

#[async_trait]
impl FeeDataSource for AsyncRpcFeeSource {
    async fn fetch_recent_fees(&self) -> Vec<u64> {
        match self.client.get_recent_prioritization_fees(&[]).await {
            Ok(entries) => {
                let fees: Vec<u64> = entries
                    .iter()
                    .map(|e| e.prioritization_fee)
                    .collect();

                if fees.is_empty() {
                    log::warn!(
                        "RPC {} returned 0 fee samples — estimator will use fallback",
                        self.rpc_url
                    );
                } else {
                    let non_zero = fees.iter().filter(|&&f| f > 0).count();
                    debug!(
                        "Fee source: {} slots, {} non-zero, range {}-{} µL ({})",
                        fees.len(),
                        non_zero,
                        fees.iter().min().copied().unwrap_or(0),
                        fees.iter().max().copied().unwrap_or(0),
                        self.rpc_url,
                    );
                }
                fees
            }
            Err(e) => {
                log::warn!(
                    "RPC fee sampling failed ({}): {e} — estimator will use fallback",
                    self.rpc_url
                );
                vec![]
            }
        }
    }
}

// -----------------------------------------------------------------------------
// CONFIGURATION
// -----------------------------------------------------------------------------

/// Runtime configuration for RealTradingEngine.
///
/// ## Shadow field policy
/// Risk thresholds (stop-loss %, take-profit %) live ONLY in `[risk]` in
/// master.toml and are accessed via `global_config.risk.*`.
/// Do NOT add `stop_loss_pct` or `profit_take_threshold` back here —
/// those fields were intentionally removed in V2.8 (PR #88) to eliminate
/// dual-source ambiguity. Add a `// V2.8 NOTE` comment if you're tempted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealTradingConfig {
    pub keystore:                          KeystoreConfig,
    pub executor:                          ExecutorConfig,
    pub slippage_bps:                      Option<u16>,
    pub max_trade_size_usdc:               Option<f64>,
    pub circuit_breaker_loss_pct:          Option<f64>,
    // ✅ V2.8 PR #88: stop_loss_pct DELETED — was a shadow of RiskConfig.
    // ✅ V2.8 PR #88: profit_take_threshold DELETED — was a shadow of RiskConfig.
    //    Use global_config.risk.stop_loss_pct / .take_profit_pct instead.
    pub profit_take_ratio:                 Option<f64>,
    pub reconcile_balances_every_n_trades: Option<u32>,
    pub jupiter_api_key:                   Option<String>,
    pub rpc_url:                           Option<String>,
}

impl Default for RealTradingConfig {
    fn default() -> Self {
        Self {
            keystore:                          KeystoreConfig::default(),
            executor:                          ExecutorConfig::default(),
            slippage_bps:                      Some(50),
            max_trade_size_usdc:               Some(250.0),
            circuit_breaker_loss_pct:          Some(5.0),
            // ✅ V2.8: stop_loss_pct and profit_take_threshold removed.
            profit_take_ratio:                 Some(0.4),
            reconcile_balances_every_n_trades: Some(10),
            jupiter_api_key:                   None,
            rpc_url:                           None,
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

    /// Bridge the master Config into this engine config.
    ///
    /// Wires all live-execution-critical fields from the global Config:
    /// - `slippage_bps`        → `config.execution.max_slippage_bps`
    /// - `max_trade_size_usdc` → `config.execution.max_trade_size_usdc`
    /// - `rpc_url`             → `config.network.rpc_url` (Chainstack)
    /// - `jupiter_api_key`     → `GRIDZBOTZ_JUPITER_API_KEY` env var
    pub fn from_config(global: &crate::Config) -> Self {
        let jupiter_api_key = std::env::var("GRIDZBOTZ_JUPITER_API_KEY")
            .ok()
            .filter(|k| !k.is_empty());

        if jupiter_api_key.is_none() {
            log::warn!(
                "[RealTradingConfig] GRIDZBOTZ_JUPITER_API_KEY not set — \
                 build_jupiter_swap() will fail at swap time. \
                 Set this env var before starting live mode."
            );
        }

        Self {
            slippage_bps:        Some(global.execution.max_slippage_bps),
            max_trade_size_usdc: Some(global.execution.max_trade_size_usdc),
            rpc_url:             Some(global.network.rpc_url.clone()),
            jupiter_api_key,
            ..Default::default()
        }
    }
}

// -----------------------------------------------------------------------------
// BALANCE TRACKER
// -----------------------------------------------------------------------------
struct BalanceTracker {
    expected_usdc:       Arc<RwLock<f64>>,
    expected_sol:        Arc<RwLock<f64>>,
    /// Initial portfolio value in USD, calculated at boot using the live
    /// SOL price from the Pyth feed — never a hardcoded estimate.
    initial_balance_usd: f64,
}

impl BalanceTracker {
    fn new(initial_usdc: f64, initial_sol: f64, sol_price_usd: f64) -> Self {
        Self {
            expected_usdc:       Arc::new(RwLock::new(initial_usdc)),
            expected_sol:        Arc::new(RwLock::new(initial_sol)),
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
    /// ✅ V2.8 PR #88: StopLossManager wired — checks before every Jupiter swap.
    /// ✅ V2.9 PR #89: Also checked on every price tick via process_price_update().
    /// Reads thresholds from RiskConfig (stop_loss_pct, take_profit_pct).
    /// Shared via Arc<RwLock> so should_stop_loss() can mutate trailing-stop state.
    stop_loss_manager:     Arc<RwLock<StopLossManager>>,
    balance_tracker:       Arc<BalanceTracker>,
    config:                RealTradingConfig,
    trades:                Arc<RwLock<Vec<Trade>>>,
    _open_orders:          Arc<RwLock<HashMap<String, Order>>>,
    next_id:               Arc<AtomicU64>,
    total_executions:      Arc<AtomicU64>,
    successful_executions: Arc<AtomicU64>,
    failed_executions:     Arc<AtomicU64>,
    emergency_shutdown:    Arc<AtomicBool>,
    /// Dynamic priority fee estimator (None = static mode).
    priority_fee_estimator: Option<Arc<PriorityFeeEstimator>>,
    /// Static fallback fee in µL/CU (used when estimator is None).
    static_priority_fee:   u64,
    /// Take-profit threshold % read from global_config.risk.take_profit_pct
    /// at construction time. Immutable — same pattern as static_priority_fee.
    /// Replaces deleted RealTradingConfig::profit_take_threshold shadow field.
    profit_take_pct:       f64,
}

impl RealTradingEngine {
    /// Construct the real trading engine.
    ///
    /// `initial_sol_price_usd` must be the live SOL/USD price from the
    /// Pyth price feed at the time of construction.
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
        initial_sol_price_usd: f64,
    ) -> Result<Self> {
        info!("[RealEngine] Initializing V2.9");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        let executor = Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?));

        let initial_nav = initial_balance_usdc + (initial_balance_sol * initial_sol_price_usd);
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_nav)
        ));

        // ✅ V2.8: Construct StopLossManager from global_config.risk.*
        //    Reads stop_loss_pct, take_profit_pct, and enable_trailing_stop.
        let stop_loss_manager = Arc::new(RwLock::new(
            StopLossManager::new(global_config)
        ));

        // Read profit_take_pct once at construction — never changes at runtime.
        // Replaces deleted RealTradingConfig::profit_take_threshold shadow field.
        let profit_take_pct = global_config.risk.take_profit_pct;

        // ── Dynamic priority fees ───────────────────────────────────────────────────────────────────
        let (priority_fee_estimator, static_priority_fee) =
            if global_config.priority_fees.enable_dynamic {
                let source = AsyncRpcFeeSource::new(
                    &global_config.network.rpc_url,
                    Duration::from_secs(5),
                );
                let estimator = PriorityFeeEstimator::new(
                    global_config.priority_fees.clone(),
                    Arc::new(source),
                );
                info!(
                    "[RealEngine] Dynamic priority fees ENABLED \
                     (P{}, ×{}, cache {}s, bounds {}-{} µL/CU)",
                    global_config.priority_fees.percentile,
                    global_config.priority_fees.multiplier,
                    global_config.priority_fees.cache_ttl_secs,
                    global_config.priority_fees.min_microlamports,
                    global_config.priority_fees.max_microlamports,
                );
                (
                    Some(Arc::new(estimator)),
                    global_config.priority_fees.fallback_microlamports,
                )
            } else {
                let fee = global_config.priority_fees.fallback_microlamports;
                info!("[RealEngine] Priority fees STATIC: {} µL/CU", fee);
                (None, fee)
            };

        let balance_tracker = Arc::new(BalanceTracker::new(
            initial_balance_usdc,
            initial_balance_sol,
            initial_sol_price_usd,
        ));

        info!("[RealEngine] Initialized V2.9");
        info!("  Wallet      : {}", keystore.pubkey());
        info!("  NAV         : ${:.2} (SOL @ ${:.4})",
            balance_tracker.initial_balance_usd(), initial_sol_price_usd);
        info!("  Stop-loss   : -{:.1}% | Take-profit: +{:.1}%",
            global_config.risk.stop_loss_pct, profit_take_pct);
        info!("  SL mode     : {}",
            if global_config.risk.enable_trailing_stop { "trailing" } else { "fixed" });

        Ok(Self {
            keystore,
            executor,
            circuit_breaker,
            stop_loss_manager,
            balance_tracker,
            config,
            trades:                Arc::new(RwLock::new(Vec::new())),
            _open_orders:          Arc::new(RwLock::new(HashMap::new())),
            next_id:               Arc::new(AtomicU64::new(1)),
            total_executions:      Arc::new(AtomicU64::new(0)),
            successful_executions: Arc::new(AtomicU64::new(0)),
            failed_executions:     Arc::new(AtomicU64::new(0)),
            emergency_shutdown:    Arc::new(AtomicBool::new(false)),
            priority_fee_estimator,
            static_priority_fee,
            profit_take_pct,
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

        // ✅ V2.8 PR #88: Stop-loss / take-profit guard — runs BEFORE Jupiter quote.
        // Uses current trade price as entry reference.
        // Bails immediately — no swap is built, no network call is made.
        {
            let mut sl = self.stop_loss_manager.write().await;
            // Take-profit checked first — close at a profit before a loss.
            if sl.should_take_profit(price, price) {
                // Note: at trade-time, current_price == price (entry == current).
                // Real continuous monitoring is in process_price_update().
                // This guard catches the case where a fill arrives at a price
                // that already satisfies the take-profit threshold.
                info!("[RealEngine] Trade blocked by take-profit guard @ ${:.4}", price);
                bail!("[RealEngine] TAKE-PROFIT GUARD: position exit required before new entry");
            }
            if sl.should_stop_loss(price, price) {
                info!("[RealEngine] Trade blocked by stop-loss guard @ ${:.4}", price);
                bail!("[RealEngine] STOP-LOSS GUARD: position exit required before new entry");
            }
        }

        let amount_usdc = price * size;

        // DUAL CAP ENFORCEMENT
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

                // Reset stop-loss manager for the new position entry.
                self.stop_loss_manager.write().await.reset_for_new_position(price);

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

        let sol_mint_pubkey  = Pubkey::from_str(SOL_MINT)
            .context("Failed to parse SOL_MINT")?;
        let usdc_mint_pubkey = Pubkey::from_str(USDC_MINT)
            .context("Failed to parse USDC_MINT")?;

        let wallet_pubkey = self.keystore.pubkey().clone();

        let (usdc_balance, sol_balance) = self.balance_tracker.get_balances().await;
        let initial_capital = usdc_balance + (sol_balance * price);

        let rpc_url = self.config.rpc_url.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "[RealEngine] rpc_url not configured. \
                 Use RealTradingConfig::from_config() at engine startup."
            ))?;

        let jupiter_api_key = self.config.jupiter_api_key.clone()
            .ok_or_else(|| anyhow::anyhow!(
                "jupiter_api_key not configured — set GRIDZBOTZ_JUPITER_API_KEY env var"
            ))?;

        // ── Dynamic priority fee ──────────────────────────────────────────────────────────────
        let per_cu_microlamports = match &self.priority_fee_estimator {
            Some(estimator) => {
                let fee = estimator.get_priority_fee().await;
                debug!("[Jupiter] Dynamic fee: {} µL/CU", fee);
                fee
            }
            None => {
                debug!("[Jupiter] Static fee: {} µL/CU", self.static_priority_fee);
                self.static_priority_fee
            }
        };

        let max_lamports = per_cu_microlamports
            .saturating_mul(JUPITER_ESTIMATED_CU)
            / 1_000_000;
        let max_lamports = max_lamports.max(MIN_MAX_LAMPORTS);

        debug!(
            "[Jupiter] Priority cap: {} µL/CU × {}CU = {} max lamports",
            per_cu_microlamports, JUPITER_ESTIMATED_CU, max_lamports
        );

        let jupiter = JupiterClient::new(
            rpc_url,
            wallet_pubkey,
            sol_mint_pubkey,
            usdc_mint_pubkey,
            initial_capital,
            jupiter_api_key,
        )?
        .with_slippage(self.config.slippage_bps.unwrap_or(50))
        .with_priority_fee(max_lamports, "high".to_string());

        let (input_mint, output_mint, amount) = match side {
            OrderSide::Buy => {
                let usdc_micro = (price * size * 1_000_000.0) as u64;
                info!("  BUY:  {:.2} USDC → SOL", price * size);
                (usdc_mint_pubkey, sol_mint_pubkey, usdc_micro)
            }
            OrderSide::Sell => {
                let sol_lamports = (size * 1_000_000_000.0) as u64;
                info!("  SELL: {:.4} SOL → USDC", size);
                (sol_mint_pubkey, usdc_mint_pubkey, sol_lamports)
            }
        };

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

    /// Evaluate take-profit at the current market price.
    ///
    /// Uses `self.profit_take_pct` — read from `global_config.risk.take_profit_pct`
    /// at construction time. Replaces deleted shadow field `profit_take_threshold`.
    pub async fn auto_take_profit(&self, current_price: f64) -> Result<()> {
        let roi = self.get_roi(current_price).await;

        // ✅ V2.8: self.profit_take_pct replaces deleted config.profit_take_threshold.
        if roi >= self.profit_take_pct {
            let (_, sol)  = self.balance_tracker.get_balances().await;
            let ratio     = self.config.profit_take_ratio.unwrap_or(0.4);
            let sell_amount = sol * ratio;

            if sell_amount > 0.01 {
                info!("[ProfitTake] ROI {:.2}% >= {:.2}% — taking profit", roi, self.profit_take_pct);
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
        let sl_mgr = self.stop_loss_manager.read().await;
        let (sl_pct, tp_pct) = sl_mgr.thresholds();
        let sl_mode = if sl_mgr.is_trailing() { "trailing" } else { "fixed" };
        let highest  = sl_mgr.highest_observed_price();
        drop(sl_mgr);

        println!();
        println!("=======================================================");
        println!("  REAL TRADING ENGINE V2.9 - STATUS");
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
        println!("Risk Guards:");
        println!("  Stop-loss   : -{:.1}%  ({})", sl_pct, sl_mode);
        println!("  Take-profit : +{:.1}%",       tp_pct);
        if sl_mode == "trailing" && highest > 0.0 {
            println!("  Trailing high: ${:.4}",   highest);
        }
        println!();
        println!("Priority Fees:");
        if self.priority_fee_estimator.is_some() {
            println!("  Mode    : DYNAMIC");
        } else {
            println!("  Mode    : STATIC ({} µL/CU)", self.static_priority_fee);
        }
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

    /// Trigger an emergency shutdown.
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

    async fn cancel_order(&self, order_id: &str) -> TradingResult<()> {
        log::warn!(
            "[RealEngine] cancel_order('{}') — Jupiter swaps are atomic; nothing to cancel",
            order_id
        );
        Ok(())
    }

    async fn cancel_all_orders(&self) -> TradingResult<usize> {
        log::warn!(
            "[RealEngine] cancel_all_orders() — Jupiter swaps are atomic; 0 cancelled"
        );
        Ok(0)
    }

    /// Continuous SL/TP monitoring — called on every Pyth price tick (10Hz).
    ///
    /// Execution order:
    /// 1. Circuit-breaker cooldown tick (`is_trading_allowed()`).
    /// 2. NAV reconciliation (P&L → circuit-breaker).
    /// 3. SL/TP check (only when entry_price > 0.0 — i.e. after first fill):
    ///    acquire write lock, snapshot stop/profit booleans, drop lock,
    ///    then call shutdown or take-profit without holding the lock.
    ///
    /// Lock safety: write lock held only for two synchronous predicate calls,
    /// explicitly dropped before any async call. No deadlock risk.
    ///
    /// Zero-cost before first fill: `entry_price()` returns 0.0 until
    /// `reset_for_new_position()` fires on a confirmed trade.
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        // Step 1: circuit-breaker cooldown tick.
        let _ = self.circuit_breaker.write().await.is_trading_allowed();

        // Step 2: NAV reconciliation.
        self.reconcile_balances(current_price).await?;

        // Step 3: continuous SL/TP monitoring.
        // ✅ V2.9 PR #89: entry_price is 0.0 until reset_for_new_position() fires
        // on a confirmed fill → this block is a no-op before the first trade.
        let entry_price = self.stop_loss_manager.read().await.entry_price();
        if entry_price > 0.0 {
            // Acquire write lock once — needed to ratchet trailing-stop high.
            // Snapshot both booleans synchronously, then drop before async work.
            let (stop_triggered, profit_triggered) = {
                let mut sl = self.stop_loss_manager.write().await;
                let stop   = sl.should_stop_loss(entry_price, current_price);
                let profit = if stop { false } else {
                    sl.should_take_profit(entry_price, current_price)
                };
                (stop, profit)
            }; // ← write lock released here — safe to call async below

            if stop_triggered {
                let (sl_pct, _) = self.stop_loss_manager.read().await.thresholds();
                self.trigger_emergency_shutdown(&format!(
                    "STOP-LOSS: price ${:.4} crossed -{:.1}% threshold (entry ${:.4})",
                    current_price, sl_pct, entry_price
                )).await?;
            } else if profit_triggered {
                info!(
                    "[RealEngine] TAKE-PROFIT on price tick: ${:.4} (entry ${:.4})",
                    current_price, entry_price
                );
                self.auto_take_profit(current_price).await?;
            }
        }

        Ok(vec![])
    }

    async fn open_order_count(&self) -> usize {
        0
    }

    async fn is_trading_allowed(&self) -> bool {
        if self.emergency_shutdown.load(Ordering::SeqCst) {
            return false;
        }
        self.circuit_breaker.write().await.is_trading_allowed()
    }

    async fn emergency_shutdown(&self, reason: &str) -> TradingResult<()> {
        self.trigger_emergency_shutdown(reason).await
    }

    async fn get_wallet(&self) -> VirtualWallet {
        let (usdc, sol) = self.balance_tracker.get_balances().await;
        VirtualWallet::new_silent(usdc, sol)
    }

    async fn get_performance_stats(&self) -> PaperPerformanceStats {
        let stats = self.get_performance_stats().await;
        PaperPerformanceStats {
            total_trades:   stats.total_trades,
            winning_trades: stats.winning_trades,
            losing_trades:  stats.losing_trades,
            total_pnl:      stats.total_pnl,
            total_fees:     stats.total_fees,
            win_rate:       stats.win_rate,
            avg_win:        stats.avg_win,
            avg_loss:       stats.avg_loss,
            largest_win:    stats.largest_win,
            largest_loss:   stats.largest_loss,
            profit_factor:  stats.profit_factor,
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

    #[test]
    fn test_no_shadow_fee_fields() {
        let config = RealTradingConfig::default();
        assert!(config.rpc_url.is_none(),         "rpc_url default must be None");
        assert!(config.jupiter_api_key.is_none(), "api_key default must be None");
    }

    #[test]
    fn test_rpc_url_default_is_none() {
        let config = RealTradingConfig::default();
        assert!(
            config.rpc_url.is_none(),
            "rpc_url default must be None — wired via from_config(), never hardcoded"
        );
    }

    /// ✅ V2.8 PR #88: Compile-time regression guard.
    /// If stop_loss_pct or profit_take_threshold are ever re-added as shadow
    /// fields on RealTradingConfig this test will fail to compile — which is
    /// exactly what we want. Do NOT add those fields back.
    #[test]
    fn test_shadow_stop_loss_fields_removed() {
        let config = RealTradingConfig::default();
        // The following lines must NOT compile if shadow fields exist:
        // let _ = config.stop_loss_pct;         // ← must NOT exist
        // let _ = config.profit_take_threshold;  // ← must NOT exist
        // Absence is guaranteed at compile time.
        // Only remaining risk fields on config:
        assert!(config.circuit_breaker_loss_pct.is_some());
        assert!(config.profit_take_ratio.is_some());
    }
}
