//! Bot module — V5.8 Multi-Bot Orchestration
//!
//! PR #86: Orchestrator + OrchestratorConfig exported for main.rs dispatch.

pub mod bot_trait;
pub mod grid_bot;
pub mod orchestrator;

pub use bot_trait::{
    Bot, BotStats, TickResult,
    IntentRegistry, OrchestratorStats, new_intent_registry,
};
pub use grid_bot::{GridBot, GridBotStats};
pub use orchestrator::{Orchestrator, OrchestratorConfig};
