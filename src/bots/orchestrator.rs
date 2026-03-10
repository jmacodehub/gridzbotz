//! ═════════════════════════════════════════════════════════════════════════
//! ORCHESTRATOR V1.0 — MULTI-BOT FLEET MANAGER
//!
//! Resolves GAP-3 (P0): Multi-bot orchestrator — runs N independent
//! bot instances in parallel Tokio tasks with:
//!
//!   • Shared `IntentRegistry` — lock-free DashMap conflict detection
//!   • Per-bot isolated panic handling — one crash never kills the fleet
//!   • Bounded mpsc channel — tick results aggregated without blocking
//!   • Graceful shutdown via `Arc<AtomicBool>` broadcast
//!   • Fleet-level `OrchestratorStats` emitted every N cycles
//!
//! Architecture:
//! ```text
//!   main() --orchestrate
//!     └─ Orchestrator::from_config(orchestrator.toml)
//!           └─ load N bot TOMLs → N GridBots
//!           └─ inject IntentRegistry via set_intent_registry()
//!           └─ call bot.initialize() on each
//!           └─ Orchestrator::run()
//!                 ├─ tokio::spawn(bot_task_0)  → process_tick() loop
//!                 ├─ tokio::spawn(bot_task_1)  → process_tick() loop
//!                 ├─ tokio::spawn(bot_task_N)  → process_tick() loop
//!                 └─ aggregation loop: rx.recv() → OrchestratorStats
//! ```
//!
//! March 2026 — V1.0 FLEET COMMANDER 🚀
//! ═════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use anyhow::{Context, Result};
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use crate::bots::bot_trait::{
    Bot, BotStats, IntentRegistry, OrchestratorStats, new_intent_registry,
};
use crate::bots::grid_bot::GridBot;
use crate::config::Config;
use crate::trading::{PriceFeed, EngineParams, create_engine};

// ─────────────────────────────────────────────────────────────────────────
// TYPE ALIAS
// `Vec<(String, Arc<Mutex<Box<dyn Bot>>>)>` would trigger clippy::type_complexity.
// `BotEntry` names the intent: an instance_id paired with its locked bot.
// ─────────────────────────────────────────────────────────────────────────
/// An owned, concurrently-accessible bot instance.
/// `String` = instance_id, `Arc<Mutex<Box<dyn Bot>>>` = the running bot.
type BotEntry = (String, Arc<Mutex<Box<dyn Bot>>>);

// ═════════════════════════════════════════════════════════════════════════
// ORCHESTRATOR CONFIG
// ═════════════════════════════════════════════════════════════════════════

/// Top-level orchestrator configuration loaded from `config/orchestrator.toml`.
///
/// Each `bot_configs` entry is a path to an individual bot TOML
/// (e.g. `config/bots/sol-usdc-grid-01.toml`). The orchestrator
/// loads each independently so bots can have different pairs, spacing,
/// and capital allocations — new pair = new file, never new code.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    /// Paths to individual bot config files.
    pub bot_configs: Vec<PathBuf>,

    /// Tick interval in milliseconds — applied uniformly across all bots.
    #[serde(default = "default_cycle_interval_ms")]
    pub cycle_interval_ms: u64,

    /// How often (in cycles) to emit aggregated fleet stats to logs.
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u32,

    /// Bounded channel buffer size per bot.
    #[serde(default = "default_channel_buffer_per_bot")]
    pub channel_buffer_per_bot: usize,
}

fn default_cycle_interval_ms()      -> u64   { 1000 }
fn default_stats_interval()         -> u32   { 30   }
fn default_channel_buffer_per_bot() -> usize { 10   }

impl OrchestratorConfig {
    /// Load from a TOML file at `path`.
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read orchestrator config: {}", path.display()))?;
        let config: Self = toml::from_str(&content)
            .with_context(|| format!("Invalid orchestrator TOML: {}", path.display()))?;
        Ok(config)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// BOT TASK MESSAGE
// ═════════════════════════════════════════════════════════════════════════

/// Message sent from each bot task to the aggregation loop.
#[derive(Debug)]
struct BotTickMsg {
    instance_id: String,
    stats:       BotStats,
    is_active:   bool,
}

// ═════════════════════════════════════════════════════════════════════════
// ORCHESTRATOR
// ═════════════════════════════════════════════════════════════════════════

/// Multi-bot fleet manager.
///
/// Owns N initialized `Box<dyn Bot>` instances wrapped in
/// `Arc<Mutex<_>>` for safe concurrent access from Tokio tasks.
/// Shares a single `IntentRegistry` across all bots to prevent
/// overlapping order placement on the same price levels.
pub struct Orchestrator {
    /// Bot instances ready to run (post-initialize).
    /// Uses `BotEntry = (String, Arc<Mutex<Box<dyn Bot>>>)` alias
    /// to satisfy clippy::type_complexity gate in lib.rs.
    bots:             Vec<BotEntry>,
    /// Shared intent registry wired into every bot.
    registry:         IntentRegistry,
    /// Orchestrator-level config (intervals, buffer sizes).
    config:           OrchestratorConfig,
    /// Shutdown broadcast — set to true by Ctrl+C handler or fatal error.
    shutdown:         Arc<AtomicBool>,
    /// Wall-clock start time for uptime tracking.
    start_time:       Instant,
    /// Running intent conflict counter (aggregated across all bots).
    intent_conflicts: u64,
}

impl Orchestrator {
    /// Build and initialize an orchestrator from a config file.
    pub async fn from_config(
        config_path: &std::path::Path,
        shutdown:    Arc<AtomicBool>,
    ) -> Result<Self> {
        let orch_config = OrchestratorConfig::from_file(config_path)?;
        let registry    = new_intent_registry();

        if orch_config.bot_configs.is_empty() {
            anyhow::bail!("Orchestrator config has no bot_configs entries");
        }

        info!("[ORCH] =================================================");
        info!("[ORCH] 🤖 ORCHESTRATOR V1.0 — {} bots initializing",
              orch_config.bot_configs.len());
        info!("[ORCH] =================================================");

        let mut bots: Vec<BotEntry> = Vec::new();

        for bot_toml in &orch_config.bot_configs {
            info!("[ORCH] Loading bot config: {}", bot_toml.display());

            let bot_config = Config::from_file(bot_toml)
                .with_context(|| format!("Failed to load bot config: {}", bot_toml.display()))?;

            let instance_id = bot_config.bot.instance_name().to_string();
            info!("[ORCH] Building bot '{}'", instance_id);

            // Build price feed
            let price_history_size = bot_config.trading.volatility_window as usize;
            let feed = Arc::new(PriceFeed::new(price_history_size));

            feed.start().await
                .map_err(|e| anyhow::anyhow!(
                    "PriceFeed start failed for '{}': {}", instance_id, e
                ))?;

            // Brief warm-up
            sleep(Duration::from_millis(500)).await;
            let initial_price = feed.latest_price().await;
            if initial_price <= 0.0 {
                anyhow::bail!(
                    "Bot '{}': price feed returned ${:.4} after warm-up — check Pyth/Hermes",
                    instance_id, initial_price
                );
            }

            // Build engine
            let params = if bot_config.bot.is_live() {
                let (usdc, sol) = crate::trading::fetch_wallet_balances_for_orchestrator(
                    &bot_config.network.rpc_url,
                    &bot_config.security.wallet_path,
                ).await
                    .with_context(|| format!("Wallet query failed for '{}'", instance_id))?;
                EngineParams {
                    live_price:       Some(initial_price),
                    wallet_balances:  Some((usdc, sol)),
                }
            } else {
                EngineParams::default()
            };

            let engine = create_engine(&bot_config, params).await
                .with_context(|| format!("Engine creation failed for '{}'", instance_id))?;

            // Construct bot
            let mut bot: Box<dyn Bot> = Box::new(
                GridBot::new(bot_config, engine, feed)?
            );

            // Wire shared intent registry
            bot.set_intent_registry(Arc::clone(&registry));
            info!("[ORCH] ✅ Intent registry wired for '{}'", instance_id);

            // Initialize (grid placement)
            bot.initialize().await
                .with_context(|| format!("Bot::initialize() failed for '{}'", instance_id))?;
            info!("[ORCH] ✅ Bot '{}' initialized — grid placed", instance_id);

            bots.push((instance_id, Arc::new(Mutex::new(bot))));
        }

        info!("[ORCH] ✅ All {} bots ready — launching fleet", bots.len());

        Ok(Self {
            bots,
            registry,
            config: orch_config,
            shutdown,
            start_time: Instant::now(),
            intent_conflicts: 0,
        })
    }

    /// Run the fleet: spawn N bot tasks + aggregation loop.
    pub async fn run(self) -> Result<()> {
        let n_bots         = self.bots.len();
        let interval_ms    = self.config.cycle_interval_ms;
        let stats_interval = self.config.stats_interval;
        let buf_size       = self.config.channel_buffer_per_bot * n_bots;
        let shutdown       = Arc::clone(&self.shutdown);
        let registry       = Arc::clone(&self.registry);

        let (tx, mut rx) = mpsc::channel::<BotTickMsg>(buf_size.max(32));

        info!("[ORCH] =================================================");
        info!("[ORCH] 🚀 FLEET LAUNCH — {} bots | {}ms interval | stats/{}c",
              n_bots, interval_ms, stats_interval);
        info!("[ORCH] =================================================");

        let mut handles = Vec::with_capacity(n_bots);

        for (instance_id, bot_arc) in &self.bots {
            let id       = instance_id.clone();
            let bot      = Arc::clone(bot_arc);
            let tx_clone = tx.clone();
            let sd       = Arc::clone(&shutdown);
            let interval = interval_ms;

            let handle = tokio::spawn(async move {
                info!("[BOT-TASK] '{}' starting tick loop", id);
                loop {
                    if sd.load(Ordering::Relaxed) {
                        info!("[BOT-TASK] '{}' received shutdown", id);
                        break;
                    }

                    let tick_result = {
                        let mut guard = bot.lock().await;
                        guard.process_tick().await
                    };

                    match tick_result {
                        Ok(tick) => {
                            let stats = {
                                let guard = bot.lock().await;
                                guard.stats()
                            };
                            let is_active = tick.active;
                            let _ = tx_clone.try_send(BotTickMsg {
                                instance_id: id.clone(),
                                stats,
                                is_active,
                            });
                            if !is_active {
                                warn!("[BOT-TASK] '{}' signalled shutdown — exiting", id);
                                break;
                            }
                        }
                        Err(e) => {
                            error!("[BOT-TASK] '{}' tick error: {} — continuing", id, e);
                        }
                    }

                    sleep(Duration::from_millis(interval)).await;
                }

                let mut guard = bot.lock().await;
                if let Err(e) = guard.shutdown().await {
                    error!("[BOT-TASK] '{}' shutdown error: {}", id, e);
                }
                info!("[BOT-TASK] '{}' task complete", id);
            });

            handles.push((instance_id.clone(), handle));
        }

        drop(tx);

        // ── Aggregation loop ────────────────────────────────────────────────
        let mut fleet_stats: HashMap<String, BotStats> = HashMap::new();
        let mut cycle_count: u64 = 0;
        let mut total_intent_conflicts: u64 = 0;
        let start = Instant::now();

        while let Some(msg) = rx.recv().await {
            if !msg.is_active {
                fleet_stats.remove(&msg.instance_id);
            } else {
                fleet_stats.insert(msg.instance_id.clone(), msg.stats);
            }

            cycle_count += 1;

            let registry_entries = registry.len() as u64;
            if registry_entries > total_intent_conflicts {
                total_intent_conflicts = registry_entries;
            }

            if cycle_count % stats_interval as u64 == 0 {
                let orch_stats = Self::aggregate_stats(
                    &fleet_stats,
                    total_intent_conflicts,
                    start.elapsed().as_secs(),
                );
                info!("[ORCH-STATS] {}", orch_stats);
                info!("[ORCH-STATS] Registry entries: {}", registry.len());
                for (id, s) in &fleet_stats {
                    info!(
                        "  └ {} | cycles={} fills={} orders={} pnl=${:.2} paused={}",
                        id, s.total_cycles, s.total_fills,
                        s.total_orders, s.current_pnl, s.is_paused
                    );
                }
            }
        }

        // ── Join all tasks ────────────────────────────────────────────────
        for (id, handle) in handles {
            match handle.await {
                Ok(_)  => info!("[ORCH] Task '{}' joined cleanly", id),
                Err(e) => warn!("[ORCH] Task '{}' panicked: {:?} — fleet unaffected", id, e),
            }
        }

        let final_stats = Self::aggregate_stats(
            &fleet_stats,
            total_intent_conflicts,
            start.elapsed().as_secs(),
        );
        info!("[ORCH] 🏁 FLEET COMPLETE — {}", final_stats);
        Ok(())
    }

    /// Aggregate per-bot `BotStats` into fleet-level `OrchestratorStats`.
    fn aggregate_stats(
        fleet: &HashMap<String, BotStats>,
        intent_conflicts: u64,
        uptime_secs: u64,
    ) -> OrchestratorStats {
        let mut stats = OrchestratorStats {
            active_bots:    fleet.len(),
            paused_bots:    0,
            total_fills:    0,
            total_orders:   0,
            fleet_pnl:      0.0,
            intent_conflicts,
            uptime_secs,
        };
        for s in fleet.values() {
            if s.is_paused { stats.paused_bots += 1; }
            stats.total_fills  += s.total_fills;
            stats.total_orders += s.total_orders;
            stats.fleet_pnl    += s.current_pnl;
        }
        stats
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::bot_trait::new_intent_registry;

    fn make_bot_stats(id: &str, fills: u64, orders: u64, pnl: f64, paused: bool) -> BotStats {
        BotStats {
            instance_id:  id.into(),
            bot_type:     "GridBot".into(),
            total_cycles: 100,
            total_fills:  fills,
            total_orders: orders,
            uptime_secs:  300,
            is_paused:    paused,
            current_pnl:  pnl,
        }
    }

    #[test]
    fn test_aggregate_stats_two_bots() {
        let mut fleet = HashMap::new();
        fleet.insert("bot-01".into(), make_bot_stats("bot-01", 10, 20, 5.0, false));
        fleet.insert("bot-02".into(), make_bot_stats("bot-02",  8, 16, 3.0, false));
        let stats = Orchestrator::aggregate_stats(&fleet, 0, 120);
        assert_eq!(stats.active_bots, 2);
        assert_eq!(stats.paused_bots, 0);
        assert_eq!(stats.total_fills, 18);
        assert_eq!(stats.total_orders, 36);
        assert!((stats.fleet_pnl - 8.0).abs() < 1e-9);
    }

    #[test]
    fn test_aggregate_stats_one_paused() {
        let mut fleet = HashMap::new();
        fleet.insert("bot-01".into(), make_bot_stats("bot-01", 5, 10, 2.0, false));
        fleet.insert("bot-02".into(), make_bot_stats("bot-02", 0,  0, 0.0, true));
        let stats = Orchestrator::aggregate_stats(&fleet, 0, 60);
        assert_eq!(stats.active_bots, 2);
        assert_eq!(stats.paused_bots, 1);
        assert_eq!(stats.total_fills, 5);
    }

    #[test]
    fn test_aggregate_stats_intent_conflicts() {
        let fleet = HashMap::new();
        let stats = Orchestrator::aggregate_stats(&fleet, 3, 60);
        assert_eq!(stats.intent_conflicts, 3);
    }

    #[test]
    fn test_orchestrator_config_defaults() {
        let toml_str = r#"
            bot_configs = ["config/bots/sol-usdc-grid-01.toml"]
        "#;
        let cfg: OrchestratorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.cycle_interval_ms, 1000);
        assert_eq!(cfg.stats_interval, 30);
        assert_eq!(cfg.channel_buffer_per_bot, 10);
        assert_eq!(cfg.bot_configs.len(), 1);
    }

    #[test]
    fn test_orchestrator_config_custom() {
        let toml_str = r#"
            bot_configs = [
                "config/bots/sol-usdc-grid-01.toml",
                "config/bots/sol-usdc-grid-02.toml"
            ]
            cycle_interval_ms = 500
            stats_interval = 20
            channel_buffer_per_bot = 15
        "#;
        let cfg: OrchestratorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.bot_configs.len(), 2);
        assert_eq!(cfg.cycle_interval_ms, 500);
        assert_eq!(cfg.stats_interval, 20);
        assert_eq!(cfg.channel_buffer_per_bot, 15);
    }

    #[test]
    fn test_intent_registry_shared_across_bots() {
        let registry = new_intent_registry();
        let r1 = Arc::clone(&registry);
        let r2 = Arc::clone(&registry);

        r1.insert(("SOL/USDC".into(), 1u64), "bot-01".into());
        assert!(r2.contains_key(&("SOL/USDC".into(), 1u64)));
        r2.insert(("SOL/USDC".into(), 2u64), "bot-02".into());
        assert_eq!(registry.len(), 2);
    }

    #[test]
    fn test_orchestrator_config_deny_unknown_fields() {
        let bad_toml = r#"
            bot_configs = []
            unknown_field = 42
        "#;
        let result: Result<OrchestratorConfig, _> = toml::from_str(bad_toml);
        assert!(result.is_err(), "deny_unknown_fields should reject unknown_field");
    }
}
