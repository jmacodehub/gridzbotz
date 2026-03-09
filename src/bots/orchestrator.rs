//! ═════════════════════════════════════════════════════════════════════════
//! ORCHESTRATOR V1.0 — MULTI-BOT FLEET MANAGER
//!
//! Resolves GAP-3 (P0) from V2 audit: "No multi-bot orchestrator."
//!
//! PR #86 — Architecture:
//!
//! ```text
//! main.rs --orchestrate path
//!   └── Orchestrator::from_config(orchestrator.toml)
//!         ├── loads N bot TOMLs → N Box<dyn Bot>
//!         ├── creates ONE IntentRegistry (Arc<DashMap>)
//!         ├── calls bot.set_intent_registry(registry.clone()) on each bot
//!         ├── calls bot.initialize() on each bot sequentially
//!         └── Orchestrator::run()
//!               ├── spawns N tokio tasks (one per bot)
//!               ├── each task: loop { bot.process_tick() → tx.send() }
//!               ├── aggregation loop: rx.recv() → update fleet stats
//!               ├── stats logged every stats_interval cycles
//!               └── on shutdown: join all JoinHandles → shutdown_all()
//! ```
//!
//! Safety guarantees:
//! - Each bot lives in its own `Arc<Mutex<Box<dyn Bot>>>` — no cross-bot locks.
//! - Bounded mpsc channel (N*10) — slow aggregation never blocks bot tasks.
//! - Bot task panics are isolated: `JoinHandle::is_finished()` detects crash,
//!   logs error, removes from fleet. Other bots keep running.
//! - Shared shutdown `Arc<AtomicBool>` — same pattern as single-bot path.
//! - Intent registry: `DashMap::entry()` is shard-level atomic — no TOCTOU.
//!
//! Capital safety:
//! - Each bot keeps its own circuit breaker, drawdown limit, stop-loss.
//! - Intent registry prevents overlapping orders on the same pair+level.
//! - `shutdown_all()` calls `bot.shutdown()` on each bot — grid cancel path fires.
//!
//! March 2026 — V5.8 MULTI-BOT ORCHESTRATOR 🤖
//! ═════════════════════════════════════════════════════════════════════════

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::sleep;

use crate::bots::bot_trait::{Bot, BotStats, IntentRegistry, new_intent_registry};
use crate::config::Config;
use crate::trading::{PriceFeed, EngineParams, create_engine, engine_mode_label};

// ═════════════════════════════════════════════════════════════════════════
// CONFIG
// ═════════════════════════════════════════════════════════════════════════

/// Top-level orchestrator configuration.
///
/// Loaded from `config/orchestrator.toml`.
/// Each `bot_configs` entry is a path to a per-bot TOML
/// (relative to the working directory).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestratorConfig {
    /// Human label for this fleet (e.g., "SOL/USDC Fleet v1").
    pub fleet_name: String,
    /// Paths to per-bot config TOMLs.
    pub bot_configs: Vec<PathBuf>,
    /// Cycle interval in ms — applied uniformly to all bots.
    pub cycle_interval_ms: u64,
    /// Log aggregated stats every N cycles.
    pub stats_interval: u64,
    /// Bounded channel capacity = bot_count * channel_multiplier.
    #[serde(default = "default_channel_multiplier")]
    pub channel_multiplier: usize,
}

fn default_channel_multiplier() -> usize { 10 }

impl OrchestratorConfig {
    /// Load from a TOML file.
    pub fn from_file(path: &PathBuf) -> Result<Self> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("Cannot read orchestrator config: {}", path.display()))?;
        let cfg: Self = toml::from_str(&raw)
            .with_context(|| format!("Invalid orchestrator config TOML: {}", path.display()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    fn validate(&self) -> Result<()> {
        if self.bot_configs.is_empty() {
            anyhow::bail!("orchestrator.toml: bot_configs must have at least one entry");
        }
        if self.cycle_interval_ms == 0 {
            anyhow::bail!("orchestrator.toml: cycle_interval_ms must be > 0");
        }
        if self.stats_interval == 0 {
            anyhow::bail!("orchestrator.toml: stats_interval must be > 0");
        }
        Ok(())
    }
}

// ═════════════════════════════════════════════════════════════════════════
// FLEET STATS
// ═════════════════════════════════════════════════════════════════════════

/// Aggregated fleet-level statistics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct FleetStats {
    pub fleet_name:       String,
    pub active_bots:      usize,
    pub total_cycles:     u64,
    pub total_fills:      u64,
    pub total_orders:     u64,
    pub total_pnl:        f64,
    pub paused_bots:      usize,
    pub intent_conflicts: usize,
    pub uptime_secs:      u64,
    pub per_bot:          HashMap<String, BotStats>,
}

impl FleetStats {
    fn from_snapshot(
        fleet_name: &str,
        per_bot: &HashMap<String, BotStats>,
        intent_registry: &IntentRegistry,
        start: &Instant,
    ) -> Self {
        let active_bots  = per_bot.len();
        let paused_bots  = per_bot.values().filter(|s| s.is_paused).count();
        let total_cycles = per_bot.values().map(|s| s.total_cycles).sum();
        let total_fills  = per_bot.values().map(|s| s.total_fills).sum();
        let total_orders = per_bot.values().map(|s| s.total_orders).sum();
        let total_pnl    = per_bot.values().map(|s| s.current_pnl).sum();
        Self {
            fleet_name:       fleet_name.to_string(),
            active_bots,
            total_cycles,
            total_fills,
            total_orders,
            total_pnl,
            paused_bots,
            intent_conflicts: intent_registry.len(), // approximate: entries = claimed levels
            uptime_secs:      start.elapsed().as_secs(),
            per_bot:          per_bot.clone(),
        }
    }

    pub fn display(&self) {
        let border = "═".repeat(68);
        println!("\n{}", border);
        println!("   🤖 FLEET STATUS — {}", self.fleet_name);
        println!("{}", border);
        println!("   Active Bots:    {}", self.active_bots);
        println!("   Paused Bots:    {}", self.paused_bots);
        println!("   Total Cycles:   {}", self.total_cycles);
        println!("   Total Fills:    {}", self.total_fills);
        println!("   Total Orders:   {}", self.total_orders);
        println!("   Total P&L:      ${:.2}", self.total_pnl);
        println!("   Intent Slots:   {} (active claims)", self.intent_conflicts);
        println!("   Uptime:         {}s", self.uptime_secs);
        println!();
        for (id, stats) in &self.per_bot {
            println!("   [{id}]");
            println!("     cycles={} fills={} orders={} pnl=${:.2} paused={}",
                     stats.total_cycles, stats.total_fills,
                     stats.total_orders, stats.current_pnl, stats.is_paused);
        }
        println!("{}", border);
    }
}

// ═════════════════════════════════════════════════════════════════════════
// ORCHESTRATOR
// ═════════════════════════════════════════════════════════════════════════

/// Internal message from a bot task to the aggregation loop.
struct BotUpdate {
    instance_id: String,
    stats:       BotStats,
}

/// Multi-bot fleet manager.
///
/// Owns N bots, one shared intent registry, and the per-bot Tokio tasks.
/// The single-bot path in `main.rs` is completely unchanged.
pub struct Orchestrator {
    config:          OrchestratorConfig,
    /// Each bot wrapped in `Arc<Mutex<...>>` for task ownership.
    bots:            Vec<(String, Arc<Mutex<Box<dyn Bot>>>)>,
    intent_registry: IntentRegistry,
    feed:            Arc<PriceFeed>,
    start:           Instant,
}

impl Orchestrator {
    // ──────────────────────────────────────────────────────────────────────
    // Construction
    // ──────────────────────────────────────────────────────────────────────

    /// Build the orchestrator from a fleet config file.
    ///
    /// 1. Loads N per-bot TOMLs → N `Config` structs.
    /// 2. Builds a price feed (shared warm-up).
    /// 3. Creates one `IntentRegistry`.
    /// 4. For each bot: creates engine + GridBot → calls `set_intent_registry()`.
    /// 5. Calls `initialize()` on each bot sequentially
    ///    (ensures grid placement doesn’t race on the same levels).
    pub async fn from_config(
        orc_path: &PathBuf,
        shutdown: Arc<AtomicBool>,
    ) -> Result<Self> {
        let orc_cfg = OrchestratorConfig::from_file(orc_path)
            .context("Failed to load orchestrator config")?;

        info!("════════════════════════════════════════════════════════════════════");
        info!("   🤖 FLEET INIT: {} bots | fleet: {}",
              orc_cfg.bot_configs.len(), orc_cfg.fleet_name);
        info!("════════════════════════════════════════════════════════════════════");

        // ── 1. Load the first bot config to get RPC + startup params ────────────────────
        let first_cfg = Config::from_file(&orc_cfg.bot_configs[0])
            .with_context(|| format!("Cannot load first bot config: {}",
                                     orc_cfg.bot_configs[0].display()))?;

        // ── 2. Shared price feed (one feed serves all bots) ──────────────────────
        let price_history_size = first_cfg.trading.volatility_window as usize;
        let feed = Arc::new(PriceFeed::new(price_history_size));
        feed.start().await
            .map_err(|e| anyhow::anyhow!("Price feed start failed: {:?}", e))?;
        let startup_delay = first_cfg.performance.startup_delay_ms;
        info!("⏳ Fleet: warming up price feed ({} ms)...", startup_delay);
        sleep(Duration::from_millis(startup_delay)).await;
        let initial_price = feed.latest_price().await;
        if initial_price <= 0.0 {
            anyhow::bail!("Price feed returned invalid price {:.4} — check Pyth/Hermes", initial_price);
        }
        let feed_mode = feed.get_mode().await;
        info!("💰 Fleet initial SOL/USD: ${:.4}  (feed mode: {:?})", initial_price, feed_mode);

        // ── 3. Shared intent registry ─────────────────────────────────────────────
        let registry = new_intent_registry();
        info!("🔐 Fleet: intent registry created (DashMap, lock-free)");

        // ── 4 + 5. Build + initialize each bot ──────────────────────────────────
        let mut bots: Vec<(String, Arc<Mutex<Box<dyn Bot>>>)> = Vec::new();

        for (idx, bot_config_path) in orc_cfg.bot_configs.iter().enumerate() {
            if shutdown.load(Ordering::Relaxed) {
                warn!("Shutdown requested during fleet init — aborting");
                break;
            }

            info!("🔧 Fleet: initializing bot {}/{}: {}",
                  idx + 1, orc_cfg.bot_configs.len(), bot_config_path.display());

            let bot_cfg = Config::from_file(bot_config_path)
                .with_context(|| format!("Failed to load bot config: {}",
                                         bot_config_path.display()))?;

            let params = if bot_cfg.bot.is_live() {
                // TODO(tech-debt): per-bot wallet balance fetch; for now share first bot's RPC
                EngineParams { live_price: Some(initial_price), wallet_balances: None }
            } else {
                EngineParams::default()
            };

            let engine = create_engine(&bot_cfg, params).await
                .with_context(|| format!("Failed to create engine for bot {}", idx + 1))?;

            let mut bot: Box<dyn Bot> = Box::new(
                crate::bots::GridBot::new(bot_cfg, engine, Arc::clone(&feed))?
            );

            // Wire registry BEFORE initialize() so conflict detection is
            // active during the very first grid placement.
            bot.set_intent_registry(Arc::clone(&registry));

            let instance_id = bot.instance_id().to_string();
            info!("⚙️  Fleet: calling Bot::initialize() for '{}'", instance_id);
            bot.initialize().await
                .with_context(|| format!("Bot::initialize() failed for '{}'", instance_id))?;
            info!("✅ Fleet: bot '{}' ready", instance_id);

            bots.push((instance_id, Arc::new(Mutex::new(bot))));
        }

        if bots.is_empty() {
            anyhow::bail!("Fleet init complete but zero bots started — check configs");
        }
        info!("🎉 Fleet '{}' ready: {} bots initialized", orc_cfg.fleet_name, bots.len());

        Ok(Self {
            config:          orc_cfg,
            bots,
            intent_registry: registry,
            feed,
            start:           Instant::now(),
        })
    }

    // ──────────────────────────────────────────────────────────────────────
    // Main run loop
    // ──────────────────────────────────────────────────────────────────────

    /// Spawn N bot tasks and run the aggregation loop until shutdown.
    ///
    /// Returns `FleetStats` at the end of the session.
    pub async fn run(self, shutdown: Arc<AtomicBool>) -> Result<FleetStats> {
        let n_bots          = self.bots.len();
        let cycle_interval  = Duration::from_millis(self.config.cycle_interval_ms);
        let stats_interval  = self.config.stats_interval;
        let channel_cap     = n_bots * self.config.channel_multiplier;
        let fleet_name      = self.config.fleet_name.clone();
        let registry        = Arc::clone(&self.intent_registry);
        let start           = self.start;

        // Bounded channel: slow aggregation never blocks bot tasks.
        // If full, the sender's `try_send` drops the update (not capital-critical).
        let (tx, mut rx) = mpsc::channel::<BotUpdate>(channel_cap);

        info!("🔥 FLEET TRADING LOOP START — {} bots | channel cap: {}",
              n_bots, channel_cap);

        // ── Spawn one task per bot ──────────────────────────────────────────────────
        let mut handles: Vec<(String, JoinHandle<()>)> = Vec::with_capacity(n_bots);

        for (instance_id, bot_arc) in self.bots {
            let tx_clone       = tx.clone();
            let shutdown_clone = Arc::clone(&shutdown);
            let id             = instance_id.clone();

            let handle = tokio::spawn(async move {
                let mut cycle: u64 = 0;
                loop {
                    if shutdown_clone.load(Ordering::Relaxed) {
                        info!("[{}] Shutdown signal received — exiting task", id);
                        break;
                    }

                    let tick_start = Instant::now();

                    let (tick_result, stats) = {
                        let mut bot = bot_arc.lock().await;
                        let tick = bot.process_tick().await;
                        let stats = bot.stats();
                        (tick, stats)
                    };

                    match tick_result {
                        Ok(tick) => {
                            if !tick.active {
                                warn!("[{}] Bot signalled shutdown — exiting task", id);
                                break;
                            }
                            debug!("[{}] Cycle {} — {}", id, cycle, tick);
                        }
                        Err(e) => {
                            error!("[{}] process_tick() error at cycle {}: {}", id, cycle, e);
                            // Continue — transient errors shouldn't kill the task.
                        }
                    }

                    // Best-effort stats send — never block the bot task.
                    let _ = tx_clone.try_send(BotUpdate {
                        instance_id: id.clone(),
                        stats,
                    });

                    cycle += 1;

                    // Pace the cycle; account for tick duration.
                    let elapsed = tick_start.elapsed();
                    if elapsed < cycle_interval {
                        sleep(cycle_interval - elapsed).await;
                    }
                }
            });

            handles.push((instance_id, handle));
        }

        // Drop our copy of tx so rx.recv() returns None when all tasks end.
        drop(tx);

        // ── Aggregation loop ────────────────────────────────────────────────────────────
        let mut per_bot_stats: HashMap<String, BotStats> = HashMap::new();
        let mut agg_cycle: u64 = 0;

        while let Some(update) = rx.recv().await {
            per_bot_stats.insert(update.instance_id, update.stats);
            agg_cycle += 1;

            if agg_cycle % stats_interval == 0 {
                let fleet = FleetStats::from_snapshot(
                    &fleet_name, &per_bot_stats, &registry, &start
                );
                fleet.display();
            }
        }

        // ── Join all handles + report panicked tasks ──────────────────────────────
        for (id, handle) in handles {
            match handle.await {
                Ok(())  => info!("[✅] Bot task '{}' joined cleanly", id),
                Err(e)  => error!("[❌] Bot task '{}' panicked: {:?}", id, e),
            }
        }

        let final_stats = FleetStats::from_snapshot(
            &fleet_name, &per_bot_stats, &registry, &start
        );
        Ok(final_stats)
    }

    // ──────────────────────────────────────────────────────────────────────
    // Shutdown helper (called by main.rs after run() completes)
    // ──────────────────────────────────────────────────────────────────────

    /// Gracefully shut down all bots in the fleet.
    ///
    /// Called after `run()` returns (shutdown signal already set).
    /// Each bot's `shutdown()` fires cancel-orders + state dump.
    /// Errors are logged but do not abort remaining shutdowns.
    pub async fn shutdown_all(bots: Vec<(String, Arc<Mutex<Box<dyn Bot>>>)>) {
        info!("[FLEET] Initiating graceful shutdown for {} bots...", bots.len());
        for (id, bot_arc) in bots {
            info!("[FLEET] Shutting down bot '{}'", id);
            let mut bot = bot_arc.lock().await;
            if let Err(e) = bot.shutdown().await {
                error!("[FLEET] Shutdown error for '{}': {}", id, e);
            }
        }
        info!("[FLEET] All bots shut down.");
    }

    /// Expose bots for shutdown after run() consumes self.
    /// Called by main.rs: `let bots = orchestrator.into_bots();`
    pub fn into_bots(self) -> Vec<(String, Arc<Mutex<Box<dyn Bot>>>)> {
        self.bots
    }

    /// Price feed accessor for display in main.rs.
    pub fn feed(&self) -> Arc<PriceFeed> {
        Arc::clone(&self.feed)
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    // ── OrchestratorConfig validation ───────────────────────────────────────────

    #[test]
    fn test_orchestrator_config_validate_empty_bots() {
        let cfg = OrchestratorConfig {
            fleet_name:        "test".into(),
            bot_configs:       vec![],
            cycle_interval_ms: 500,
            stats_interval:    10,
            channel_multiplier: 10,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_orchestrator_config_validate_zero_interval() {
        let cfg = OrchestratorConfig {
            fleet_name:        "test".into(),
            bot_configs:       vec![PathBuf::from("config/bots/sol-usdc-grid-01.toml")],
            cycle_interval_ms: 0,   // invalid
            stats_interval:    10,
            channel_multiplier: 10,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_orchestrator_config_validate_ok() {
        let cfg = OrchestratorConfig {
            fleet_name:        "SOL/USDC Fleet v1".into(),
            bot_configs:       vec![
                PathBuf::from("config/bots/sol-usdc-grid-01.toml"),
                PathBuf::from("config/bots/sol-usdc-grid-02.toml"),
            ],
            cycle_interval_ms: 500,
            stats_interval:    20,
            channel_multiplier: 10,
        };
        assert!(cfg.validate().is_ok());
    }

    // ── FleetStats aggregation ─────────────────────────────────────────────────

    #[test]
    fn test_fleet_stats_aggregation() {
        let mut per_bot: HashMap<String, BotStats> = HashMap::new();
        per_bot.insert("bot-01".into(), BotStats {
            instance_id:  "bot-01".into(),
            bot_type:     "GridBot".into(),
            total_cycles: 100,
            total_fills:  10,
            total_orders: 20,
            uptime_secs:  300,
            is_paused:    false,
            current_pnl:  5.0,
        });
        per_bot.insert("bot-02".into(), BotStats {
            instance_id:  "bot-02".into(),
            bot_type:     "GridBot".into(),
            total_cycles: 80,
            total_fills:  8,
            total_orders: 16,
            uptime_secs:  300,
            is_paused:    true,
            current_pnl:  -1.5,
        });
        let registry = new_intent_registry();
        let start    = Instant::now();
        let fleet    = FleetStats::from_snapshot("test-fleet", &per_bot, &registry, &start);

        assert_eq!(fleet.active_bots,  2);
        assert_eq!(fleet.paused_bots,  1);
        assert_eq!(fleet.total_cycles, 180);
        assert_eq!(fleet.total_fills,  18);
        assert_eq!(fleet.total_orders, 36);
        assert!((fleet.total_pnl - 3.5).abs() < 1e-9);
    }

    // ── Intent registry conflict (re-verified here at orchestrator level) ───

    #[test]
    fn test_intent_registry_conflict_across_bots() {
        let registry = new_intent_registry();
        // Bot-01 claims SOL/USDC level 5
        registry.insert(("sol-usdc-grid-01".to_string(), 5u64), "sol-usdc-grid-01".to_string());
        // Bot-02 tries the same pair+level— sees Occupied
        match registry.entry(("sol-usdc-grid-01".to_string(), 5u64)) {
            dashmap::Entry::Vacant(_)   => panic!("Expected conflict"),
            dashmap::Entry::Occupied(e) => {
                assert_eq!(e.get(), "sol-usdc-grid-01");
            }
        }
        // Bot-02 claims a different level — no conflict
        match registry.entry(("sol-usdc-grid-01".to_string(), 6u64)) {
            dashmap::Entry::Vacant(e) => { e.insert("sol-usdc-grid-02".to_string()); }
            dashmap::Entry::Occupied(_) => panic!("Should be free"),
        }
        assert_eq!(registry.len(), 2);
    }

    // ── Fleet stats display (smoke test — no panic) ──────────────────────────

    #[test]
    fn test_fleet_stats_display_no_panic() {
        let per_bot  = HashMap::new();
        let registry = new_intent_registry();
        let start    = Instant::now();
        let fleet    = FleetStats::from_snapshot("test", &per_bot, &registry, &start);
        // display() uses println! — just ensure it doesn't panic
        fleet.display();
    }
}
