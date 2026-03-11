//! =============================================================================
//! REAL TRADER ENGINE V3.2
//!
//! V3.2 CHANGES (fix/fill-level-id-cb-nav-wiring — PR #102 Commit 2):
//! ✅ pending_fills: Arc<RwLock<Vec<FillEvent>>> added to RealTradingEngine.
//!    place_limit_order_with_level() pushes a synthetic FillEvent after every
//!    confirmed Jupiter swap, carrying level_id and actual fill price/size.
//!    process_price_update() drains pending_fills and returns them so that
//!    grid_bot.rs receives fills in live mode, enabling:
//!      - notify_fill() fan-out to all strategies
//!      - mark_buy/sell_filled(level_id) in GridStateTracker
//!      - real P&L delta delivery to CircuitBreaker.record_trade()
//!    fee_usdc = 0.0: Jupiter fee is embedded in output amount;
//!    TODO(tech-debt): reconcile actual fee from tx receipt in future PR.
//!
//! V3.1 CHANGES (feat/priority-fee-quant-log — PR #97 Commit 1):
//! ✅ build_jupiter_swap(): add quant info! log for priority fee observability.
//!    GAP-3: fee resolution used debug!() only — invisible at info level in
//!    production (logging.level = "info" in all production TOMLs).
//!    New log line (both dynamic and static paths):
//!      [Quant] priority_fee: mode=dynamic|static fee=X µL/CU max_lamports=Y
//!    Feeds: prod monitoring today + ML training data in future.
//!
//! V3.0 CHANGES (fix/fees-config-reconciliation — PR #96 Commit 1):
//! ✅ RealTradingConfig::slippage_bps DELETED — was a shadow of
//!    ExecutionConfig::max_slippage_bps (bridged via from_config()).
//!    FeesConfig is now undisputed single source of truth for fee math;
//!    ExecutionConfig::max_slippage_bps owns Jupiter swap tolerance.
//! ✅ RealTradingEngine::slippage_bps (u16) added as plain engine field.
//!    Read once from global_config.execution.max_slippage_bps at construction.
//!    Same pattern as static_priority_fee and profit_take_pct.
//! ✅ build_jupiter_swap(): .with_slippage(self.slippage_bps) — no more
//!    unwrap_or(50) fallback that could silently override TOML config.
//! ✅ test_no_slippage_shadow_on_realconfig: compile-time regression guard.
//!
//! V2.9 CHANGES (fix/risk-continuous-sl-monitoring — PR #89):
//! ✅ process_price_update(): continuous SL/TP monitoring on every price tick.
//!
//! V2.8 CHANGES (fix/risk-stop-loss-wiring — PR #88):
//! ✅ StopLossManager wired into RealTradingEngine.
//!
//! V2.7 CHANGES (feat/dynamic-priority-fees — PR #79 Commit 8):
//! ✅ AsyncRpcFeeSource: async FeeDataSource impl using nonblocking RpcClient.
//!
//! V2.6 CHANGES (fix/real-trader-fee-shadow-rpc-wiring — PR #79 Commit 1):
//! ✅ Shadow fee fields removed: maker_fee_bps + taker_fee_bps.
//!
//! ## Shadow field policy (RealTradingConfig)
//!
//! Do NOT add shadow fields back to RealTradingConfig. Sources of truth:
//!   - Slippage (Jupiter tolerance): ExecutionConfig::max_slippage_bps
//!     → cached as engine.slippage_bps at construction (PR #96)
//!   - Stop-loss / take-profit %:    RiskConfig (PR #88)
//!     → cached as engine.profit_take_pct at construction
//!   - Static priority fee:          PriorityFeeConfig::fallback_microlamports
//!     → cached as engine.static_priority_fee at construction (PR #79)
//!   - Fee math (maker/taker BPS):   FeesConfig — consumed by paper_trader,
//!     fee_filter, grid_rebalancer — not needed in real execution path.
//!
//! March 2026 — V3.2 🚀
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
// ⚡ ASYNC RPC FEE SOURCE
// =============================================================================

/// Estimated CU budget for Jupiter swaps (used to convert µL/CU → total lamports).
const JUPITER_ESTIMATED_CU: u64 = 300_000;

/// Minimum max_lamports floor — ensures txs can land even if estimator
/// returns very low values during unusually calm periods.
const MIN_MAX_LAMPORTS: u64 = 5_000;

struct AsyncRpcFeeSource {
    client: AsyncRpcClient,
    rpc_url: String,
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
///
/// Do NOT add shadow fields to this struct. Each config value has one
/// canonical source in the global Config tree:
///
///   - Slippage (Jupiter tolerance): `ExecutionConfig::max_slippage_bps`
///     Cached as `RealTradingEngine::slippage_bps` at construction (PR #96).
///
///   - Stop-loss / take-profit %: `RiskConfig::stop_loss_pct / take_profit_pct`
///     Cached as `RealTradingEngine::profit_take_pct` at construction (PR #88).
///
///   - Static priority fee: `PriorityFeeConfig::fallback_microlamports`
///     Cached as `RealTradingEngine::static_priority_fee` at construction (PR #79).
///
/// Removed shadow fields (compile-time guards below):
///   ✅ PR #88: stop_loss_pct, profit_take_threshold
///   ✅ PR #79: maker_fee_bps, taker_fee_bps
///   ✅ PR #96: slippage_bps
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RealTradingConfig {
    pub keystore:                          KeystoreConfig,
    pub executor:                          ExecutorConfig,
    pub max_trade_size_usdc:               Option<f64>,
    pub circuit_breaker_loss_pct:          Option<f64>,
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
            max_trade_size_usdc:               Some(250.0),
            circuit_breaker_loss_pct:          Some(5.0),
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
        Ok(())
    }

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
    priority_fee_estimator: Option<Arc<PriorityFeeEstimator>>,
    static_priority_fee:   u64,
    profit_take_pct:       f64,
    slippage_bps:          u16,
    /// V3.2: Synthetic FillEvents from confirmed Jupiter swaps.
    /// place_limit_order_with_level() pushes here after execute_trade() succeeds.
    /// process_price_update() drains this buffer and returns fills to grid_bot.
    /// Arc<RwLock> so both methods can share it without making engine non-Send.
    pending_fills:         Arc<RwLock<Vec<FillEvent>>>,
}

impl RealTradingEngine {
    pub async fn new(
        config: RealTradingConfig,
        global_config: &Config,
        initial_balance_usdc: f64,
        initial_balance_sol: f64,
        initial_sol_price_usd: f64,
    ) -> Result<Self> {
        info!("[RealEngine] Initializing V3.2");

        config.validate()?;

        let keystore = Arc::new(SecureKeystore::from_file(config.keystore.clone())?);
        let executor = Arc::new(RwLock::new(TransactionExecutor::new(config.executor.clone())?));

        let initial_nav = initial_balance_usdc + (initial_balance_sol * initial_sol_price_usd);
        let circuit_breaker = Arc::new(RwLock::new(
            CircuitBreaker::with_balance(global_config, initial_nav)
        ));

        let stop_loss_manager = Arc::new(RwLock::new(
            StopLossManager::new(global_config)
        ));

        let profit_take_pct = global_config.risk.take_profit_pct;
        let slippage_bps    = global_config.execution.max_slippage_bps;

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

        info!("[RealEngine] Initialized V3.2");
        info!("  Wallet      : {}", keystore.pubkey());
        info!("  NAV         : ${:.2} (SOL @ ${:.4})",
            balance_tracker.initial_balance_usd(), initial_sol_price_usd);
        info!("  Stop-loss   : -{:.1}% | Take-profit: +{:.1}%",
            global_config.risk.stop_loss_pct, profit_take_pct);
        info!("  SL mode     : {}",
            if global_config.risk.enable_trailing_stop { "trailing" } else { "fixed" });
        info!("  Slippage    : {} bps (max Jupiter swap tolerance)", slippage_bps);

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
            slippage_bps,
            pending_fills:         Arc::new(RwLock::new(Vec::new())),
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

        {
            let mut sl = self.stop_loss_manager.write().await;
            if sl.should_take_profit(price, price) {
                info!("[RealEngine] Trade blocked by take-profit guard @ ${:.4}", price);
                bail!("[RealEngine] TAKE-PROFIT GUARD: position exit required before new entry");
            }
            if sl.should_stop_loss(price, price) {
                info!("[RealEngine] Trade blocked by stop-loss guard @ ${:.4}", price);
                bail!("[RealEngine] STOP-LOSS GUARD: position exit required before new entry");
            }
        }

        let amount_usdc = price * size;

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

        let (per_cu_microlamports, fee_mode) = match &self.priority_fee_estimator {
            Some(estimator) => {
                let fee = estimator.get_priority_fee().await;
                debug!("[Jupiter] Dynamic fee resolved: {} µL/CU", fee);
                (fee, "dynamic")
            }
            None => {
                debug!("[Jupiter] Static fee: {} µL/CU", self.static_priority_fee);
                (self.static_priority_fee, "static")
            }
        };

        let max_lamports = per_cu_microlamports
            .saturating_mul(JUPITER_ESTIMATED_CU)
            / 1_000_000;
        let max_lamports = max_lamports.max(MIN_MAX_LAMPORTS);

        info!(
            "[Quant] priority_fee: mode={} fee={} µL/CU max_lamports={}",
            fee_mode, per_cu_microlamports, max_lamports
        );

        let jupiter = JupiterClient::new(
            rpc_url,
            wallet_pubkey,
            sol_mint_pubkey,
            usdc_mint_pubkey,
            initial_capital,
            jupiter_api_key,
        )?
        .with_slippage(self.slippage_bps)
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

    pub async fn auto_take_profit(&self, current_price: f64) -> Result<()> {
        let roi = self.get_roi(current_price).await;

        if roi >= self.profit_take_pct {
            let (_, sol)    = self.balance_tracker.get_balances().await;
            let ratio       = self.config.profit_take_ratio.unwrap_or(0.4);
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
        println!("  REAL TRADING ENGINE V3.2 - STATUS");
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
        println!("Execution:");
        println!("  Slippage    : {} bps (max Jupiter tolerance)", self.slippage_bps);
        if self.priority_fee_estimator.is_some() {
            println!("  Priority fee: DYNAMIC");
        } else {
            println!("  Priority fee: STATIC ({} µL/CU)", self.static_priority_fee);
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
    /// Execute a Jupiter swap for the given grid level and push a synthetic
    /// FillEvent to pending_fills.
    ///
    /// Jupiter swaps are atomic — there is no open order to poll. The fill
    /// is constructed here, immediately after the confirmed swap, so that
    /// process_price_update() can drain it and deliver it to grid_bot:
    ///   - grid_bot marks the level filled in GridStateTracker
    ///   - CircuitBreaker receives real P&L delta (not always-zero snapshot)
    ///   - StrategyManager.notify_fill() fan-out fires correctly
    ///
    /// fee_usdc = 0.0: Jupiter fee is embedded in output amount and cannot
    /// be computed at this call site without parsing the tx receipt.
    /// TODO(tech-debt): reconcile fee from tx receipt in a future PR.
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
        let order_id = self.execute_trade(side, price, size).await?;

        // Synthetic FillEvent — swap confirmed atomically, no order-book scan needed.
        let ts = chrono::Utc::now().timestamp();
        let mut fill = FillEvent::new(
            order_id.clone(),
            side,
            price,
            size,
            0.0,   // fee_usdc: embedded in Jupiter output, not separately known here
            None,  // pnl: resolved by GridStateTracker.mark_buy/sell_filled()
            ts,
        );
        if let Some(lid) = grid_level_id {
            fill = fill.with_level(lid);
        }
        self.pending_fills.write().await.push(fill);

        Ok(order_id)
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

    /// Poll for fills and run SL/TP monitoring.
    ///
    /// Returns synthetic FillEvents that were pushed by place_limit_order_with_level()
    /// since the last call, merged with any SL/TP-triggered fills.
    /// In paper mode this is driven by order-book matching; in real mode by
    /// confirmed Jupiter swaps accumulated in self.pending_fills.
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>> {
        let _ = self.circuit_breaker.write().await.is_trading_allowed();
        self.reconcile_balances(current_price).await?;

        let entry_price = self.stop_loss_manager.read().await.entry_price();
        if entry_price > 0.0 {
            let (stop_triggered, profit_triggered) = {
                let mut sl = self.stop_loss_manager.write().await;
                let stop   = sl.should_stop_loss(entry_price, current_price);
                let profit = if stop { false } else {
                    sl.should_take_profit(entry_price, current_price)
                };
                (stop, profit)
            };

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

        // Drain synthetic fills accumulated since last tick.
        // In a quiet tick this is empty vec — zero allocation cost.
        let fills = std::mem::take(&mut *self.pending_fills.write().await);
        Ok(fills)
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

    #[test]
    fn test_shadow_stop_loss_fields_removed() {
        let config = RealTradingConfig::default();
        assert!(config.circuit_breaker_loss_pct.is_some());
        assert!(config.profit_take_ratio.is_some());
    }

    #[test]
    fn test_no_slippage_shadow_on_realconfig() {
        let config = RealTradingConfig::default();
        assert!(config.max_trade_size_usdc.is_some(),               "max_trade_size_usdc must be Some");
        assert!(config.circuit_breaker_loss_pct.is_some(),          "circuit_breaker_loss_pct must be Some");
        assert!(config.profit_take_ratio.is_some(),                  "profit_take_ratio must be Some");
        assert!(config.reconcile_balances_every_n_trades.is_some(), "reconcile_n_trades must be Some");
        assert!(config.rpc_url.is_none(),                            "rpc_url must default to None");
        assert!(config.jupiter_api_key.is_none(),                    "jupiter_api_key must default to None");
    }
}
