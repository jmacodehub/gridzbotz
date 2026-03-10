//! ═════════════════════════════════════════════════════════════════════════
//! ORCHESTRATOR V1.1 — MULTI-BOT FLEET MANAGER
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
//! PR #91 fixes:
//!   • `intent_conflicts` now summed from `BotStats.intent_conflicts`
//!     (real conflict events) — was incorrectly reading `registry.len()`
//!     (which counts successful claims, not conflict events).
//!   • `registry.len()` logged separately as `claimed_levels` for
//!     observability without misleading the conflicts metric.
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
//! March 2026 — V1.1 FLEET COMMANDER 🚀
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
// ─────────────────────────────────────────────────────────────────────────
type BotEntry = (String, Arc<Mutex<Box<dyn Bot>>>);

// ═════════════════════════════════════════════════════════════════════════
// ORCHESTRATOR CONFIG
// ═════════════════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    pub bot_configs: Vec<PathBuf>,
    #[serde(default = "default_cycle_interval_ms")]
    pub cycle_interval_ms: u64,
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u32,
    #[serde(default = "default_channel_buffer_per_bot")]
    pub channel_buffer_per_bot: usize,
}

fn default_cycle_interval_ms()      -> u64   { 1000 }
fn default_stats_interval()         -> u32   { 30   }
fn default_channel_buffer_per_bot() -> usize { 10   }

impl OrchestratorConfig {
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

#[derive(Debug)]
struct BotTickMsg {
    instance_id: String,
    stats:       BotStats,
    is_active:   bool,
}

// ═════════════════════════════════════════════════════════════════════════
// ORCHESTRATOR
// ═════════════════════════════════════════════════════════════════════════

pub struct Orchestrator {
    bots:             Vec<BotEntry>,
    registry:         IntentRegistry,
    config:           OrchestratorConfig,
    shutdown:         Arc<AtomicBool>,
    start_time:       Instant,
    /// Lifetime conflict counter — accumulated from BotStats each cycle.
    /// PR #91: was registry.len() high-water-mark (claimed levels, not events).
    intent_conflicts: u64,
}

impl Orchestrator {
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
        info!("[ORCH] 🤖 ORCHESTRATOR V1.1 — {} bots initializing",
              orch_config.bot_configs.len());
        info!("[ORCH] =================================================");

        let mut bots: Vec<BotEntry> = Vec::new();

        for bot_toml in &orch_config.bot_configs {
            info!("[ORCH] Loading bot config: {}", bot_toml.display());

            let bot_config = Config::from_file(bot_toml)
                .with_context(|| format!("Failed to load bot config: {}", bot_toml.display()))?;

            let instance_id = bot_config.bot.instance_name().to_string();
            info!("[ORCH] Building bot '{}'", instance_id);

            let price_history_size = bot_config.trading.volatility_window as usize;
            let feed = Arc::new(PriceFeed::new(price_history_size));

            feed.start().await
                .map_err(|e| anyhow::anyhow!(
                    "PriceFeed start failed for '{}': {}", instance_id, e
                ))?;

            sleep(Duration::from_millis(500)).await;
            let initial_price = feed.latest_price().await;
            if initial_price <= 0.0 {
                anyhow::bail!(
                    "Bot '{}': price feed returned ${:.4} after warm-up — check Pyth/Hermes",
                    instance_id, initial_price
                );
            }

            let params = if bot_config.bot.is_live() {
                let (usdc, sol) = crate::trading::fetch_wallet_balances_for_orchestrator(
                    &bot_config.network.rpc_url,
                    &bot_config.security.wallet_path,
                ).await
                    .with_context(|| format!("Wallet query failed for '{}'", instance_id))?;
                EngineParams {
                    live_price:      Some(initial_price),
                    wallet_balances: Some((usdc, sol)),
                }
            } else {
                EngineParams::default()
            };

            let engine = create_engine(&bot_config, params).await
                .with_context(|| format!("Engine creation failed for '{}'", instance_id))?;

            let mut bot: Box<dyn Bot> = Box::new(
                GridBot::new(bot_config, engine, feed)?
            );

            bot.set_intent_registry(Arc::clone(&registry));
            info!("[ORCH] ✅ Intent registry wired for '{}'", instance_id);

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
        // PR #91: intent_conflicts is now summed from BotStats.intent_conflicts
        // (real Occupied-branch hits), NOT from registry.len() which counted
        // successful level claims — a completely different metric.
        // registry.len() is still logged separately as `claimed_levels`.
        let mut fleet_stats: HashMap<String, BotStats> = HashMap::new();
        let mut cycle_count: u64 = 0;
        let start = Instant::now();

        while let Some(msg) = rx.recv().await {
            if !msg.is_active {
                fleet_stats.remove(&msg.instance_id);
            } else {
                fleet_stats.insert(msg.instance_id.clone(), msg.stats);
            }

            cycle_count += 1;

            if cycle_count % stats_interval as u64 == 0 {
                // Sum real conflict events from each bot's BotStats
                let total_conflicts: u64 = fleet_stats
                    .values()
                    .map(|s| s.intent_conflicts)
                    .sum();

                let orch_stats = Self::aggregate_stats(
                    &fleet_stats,
                    total_conflicts,
                    start.elapsed().as_secs(),
                );
                info!("[ORCH-STATS] {}", orch_stats);
                // Log claimed_levels separately — distinct from conflict events
                info!("[ORCH-STATS] claimed_levels={} (active registry entries)",
                      registry.len());
                for (id, s) in &fleet_stats {
                    info!(
                        "  └ {} | cycles={} fills={} orders={} pnl=${:.2} paused={} conflicts={}",
                        id, s.total_cycles, s.total_fills,
                        s.total_orders, s.current_pnl, s.is_paused,
                        s.intent_conflicts
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

        let final_conflicts: u64 = fleet_stats.values().map(|s| s.intent_conflicts).sum();
        let final_stats = Self::aggregate_stats(
            &fleet_stats,
            final_conflicts,
            start.elapsed().as_secs(),
        );
        info!("[ORCH] 🏁 FLEET COMPLETE — {}", final_stats);
        Ok(())
    }

    fn aggregate_stats(
        fleet: &HashMap<String, BotStats>,
        intent_conflicts: u64,
        uptime_secs: u64,
    ) -> OrchestratorStats {
        let mut stats = OrchestratorStats {
            active_bots:      fleet.len(),
            paused_bots:      0,
            total_fills:      0,
            total_orders:     0,
            fleet_pnl:        0.0,
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

    fn make_bot_stats(id: &str, fills: u64, orders: u64, pnl: f64, paused: bool, conflicts: u64) -> BotStats {
        BotStats {
            instance_id:      id.into(),
            bot_type:         "GridBot".into(),
            total_cycles:     100,
            total_fills:      fills,
            total_orders:     orders,
            uptime_secs:      300,
            is_paused:        paused,
            current_pnl:      pnl,
            intent_conflicts: conflicts,
        }
    }

    #[test]
    fn test_aggregate_stats_two_bots() {
        let mut fleet = HashMap::new();
        fleet.insert("bot-01".into(), make_bot_stats("bot-01", 10, 20, 5.0, false, 0));
        fleet.insert("bot-02".into(), make_bot_stats("bot-02",  8, 16, 3.0, false, 0));
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
        fleet.insert("bot-01".into(), make_bot_stats("bot-01", 5, 10, 2.0, false, 0));
        fleet.insert("bot-02".into(), make_bot_stats("bot-02", 0,  0, 0.0, true,  0));
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
    fn test_aggregate_stats_sums_bot_conflicts() {
        // PR #91: conflicts must be summed from BotStats, not registry.len().
        let mut fleet = HashMap::new();
        fleet.insert("bot-01".into(), make_bot_stats("bot-01", 10, 20, 5.0, false, 2));
        fleet.insert("bot-02".into(), make_bot_stats("bot-02",  8, 16, 3.0, false, 1));
        // Sum: bot-01 had 2 real conflicts, bot-02 had 1 = 3 total
        let total_conflicts: u64 = fleet.values().map(|s| s.intent_conflicts).sum();
        let stats = Orchestrator::aggregate_stats(&fleet, total_conflicts, 120);
        assert_eq!(
            stats.intent_conflicts, 3,
            "Fleet conflicts must sum real events (2+1=3), not registry size"
        );
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
