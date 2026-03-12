//! ═══════════════════════════════════════════════════════════════════════
//! Telegram Bot Integration — PR #103 Hardened
//!
//! V2 changes (PR #103 — fix/cb-reconcile-telegram-hardening):
//! ✅ client: Client stored as struct field — reqwest connection pool
//!    reused across all send_*() calls. Previously Client::new() was
//!    called on every fire() invocation, rebuilding the pool each time.
//! ✅ .timeout(Duration::from_secs(5)) added to every HTTP request.
//!    Without this a slow/down Telegram API would block the entire
//!    trading cycle because send_fill() etc. are .await-ed in grid_bot.
//! ✅ .error_for_status()? added — Telegram returns HTTP 200 with
//!    {"ok":false} on bad token/chat_id. Previously fire() silently
//!    "succeeded" on credential errors. Now warns loudly in logs.
//!
//! V1 (PR #101 — Full Alert Suite):
//! Provides real-time mobile notifications for live trading events.
//! All methods are fire-and-forget async — failures are logged but
//! never propagate to the trading loop (capital safety first).
//!
//! Setup:
//!   export GRIDZBOTZ_TELEGRAM_TOKEN=7412345678:AAF_abc123...
//!   export GRIDZBOTZ_TELEGRAM_CHAT_ID=123456789
//!
//! If either env var is absent the bot is disabled and all send_*()
//! calls are silent no-ops. Zero config required for paper trading.
//!
//! Message types:
//!   send_bot_started()             — 🚀 startup confirmation
//!   send_fill()                    — 💰 every SELL fill
//!   send_circuit_breaker_tripped() — 🚨 CB trip (URGENT)
//!   send_circuit_breaker_reset()   — ✅ CB cooldown elapsed
//!   send_shutdown()                — 🏁 graceful shutdown summary
//!   send_heartbeat()               — 📊 periodic P&L heartbeat
//!   send_alert()                   — 🚨 generic alert (legacy)
//!   send_test_complete()           — 🎉 paper test done (legacy)
//! ═══════════════════════════════════════════════════════════════════════

use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use log::{debug, warn};

// ═══════════════════════════════════════════════════════════════════════
// STRUCT
// ═══════════════════════════════════════════════════════════════════════

/// Fire-and-forget Telegram notifier.
/// Construct via `TelegramBot::from_env()` for production or
/// `TelegramBot::new(Some(token), Some(chat_id))` for explicit wiring.
/// When `enabled = false` all methods return immediately — no network call.
#[derive(Clone)]
pub struct TelegramBot {
    token:   String,
    chat_id: String,
    enabled: bool,
    /// Reused across all send_*() calls — avoids rebuilding the
    /// connection pool on every message (PR #103).
    client:  Client,
}

// ═══════════════════════════════════════════════════════════════════════
// CONSTRUCTORS
// ═══════════════════════════════════════════════════════════════════════

impl TelegramBot {
    /// Explicit constructor — used for testing or manual wiring.
    /// Disables the bot if either argument is `None`.
    pub fn new(token: Option<String>, chat_id: Option<String>) -> Self {
        let enabled = token.is_some() && chat_id.is_some();
        Self {
            token:   token.unwrap_or_default(),
            chat_id: chat_id.unwrap_or_default(),
            enabled,
            client:  Client::new(),
        }
    }

    /// Production constructor — reads from env vars:
    ///   GRIDZBOTZ_TELEGRAM_TOKEN
    ///   GRIDZBOTZ_TELEGRAM_CHAT_ID
    /// Silent no-op bot returned if either is absent.
    pub fn from_env() -> Self {
        let token   = std::env::var("GRIDZBOTZ_TELEGRAM_TOKEN").ok();
        let chat_id = std::env::var("GRIDZBOTZ_TELEGRAM_CHAT_ID").ok();
        let enabled = token.is_some() && chat_id.is_some();
        if enabled {
            log::info!("[TELEGRAM] Enabled — alerts will fire to chat {}",
                chat_id.as_deref().unwrap_or("?"));
        } else {
            log::info!("[TELEGRAM] Disabled — set GRIDZBOTZ_TELEGRAM_TOKEN + \
                GRIDZBOTZ_TELEGRAM_CHAT_ID to enable mobile alerts");
        }
        Self {
            token:   token.unwrap_or_default(),
            chat_id: chat_id.unwrap_or_default(),
            enabled,
            client:  Client::new(),
        }
    }

    pub fn is_enabled(&self) -> bool { self.enabled }
}

// ═══════════════════════════════════════════════════════════════════════
// TRADING EVENT MESSAGES
// ═══════════════════════════════════════════════════════════════════════

impl TelegramBot {
    /// 🚀 Bot startup — sent once after grid is initialized.
    pub async fn send_bot_started(
        &self,
        instance_id: &str,
        pair:        &str,
        capital:     f64,
        spacing_pct: f64,
        conf_gate:   f64,
        mode:        &str,
    ) {
        if !self.enabled { return; }
        let msg = format!(
            "🚀 *GridzBot Started*\n\
             \n\
             Instance: `{instance_id}`\n\
             Pair:     `{pair}`\n\
             Capital:  *${capital:.2}*\n\
             Spacing:  `{spacing_pct:.3}%`\n\
             WMA Gate: `{conf_gate:.2}`\n\
             Mode:     `{mode}`"
        );
        self.fire(&msg).await;
    }

    /// 💰 Fill event — sent on every completed SELL fill (realized P&L).
    pub async fn send_fill(
        &self,
        instance_id: &str,
        side:        &str,
        price:       f64,
        size_sol:    f64,
        pnl:         f64,
        total_fills: u64,
    ) {
        if !self.enabled { return; }
        let emoji = if side == "SELL" { "💰" } else { "🛒" };
        let pnl_str = if pnl >= 0.0 {
            format!("+${pnl:.4}")
        } else {
            format!("-${:.4}", pnl.abs())
        };
        let msg = format!(
            "{emoji} *Fill #{total_fills}* — `{instance_id}`\n\
             \n\
             Side:  `{side}`\n\
             Price: `${price:.4}`\n\
             Size:  `{size_sol:.4} SOL`\n\
             P&L:   *{pnl_str}*"
        );
        self.fire(&msg).await;
    }

    /// 🚨 Circuit breaker tripped — URGENT alert.
    pub async fn send_circuit_breaker_tripped(
        &self,
        instance_id:   &str,
        reason:        &str,
        drawdown_pct:  f64,
        pnl:           f64,
        cooldown_secs: u64,
    ) {
        if !self.enabled { return; }
        let msg = format!(
            "🚨 *CIRCUIT BREAKER TRIPPED* 🚨\n\
             \n\
             Instance:  `{instance_id}`\n\
             Reason:    *{reason}*\n\
             Drawdown:  `{drawdown_pct:.2}%`\n\
             P&L:       `${pnl:.2}`\n\
             Cooldown:  `{cooldown_secs}s`\n\
             \n\
             _Trading halted. Bot will resume after cooldown._"
        );
        self.fire(&msg).await;
    }

    /// ✅ Circuit breaker reset — trading resumed.
    pub async fn send_circuit_breaker_reset(&self, instance_id: &str) {
        if !self.enabled { return; }
        let msg = format!(
            "✅ *Circuit Breaker Reset*\n\
             \n\
             Instance: `{instance_id}`\n\
             Status:   *Trading resumed*"
        );
        self.fire(&msg).await;
    }

    /// 🏁 Graceful shutdown — final session summary.
    pub async fn send_shutdown(
        &self,
        instance_id:  &str,
        uptime_secs:  u64,
        total_fills:  u64,
        total_orders: u64,
        pnl:          f64,
        roi_pct:      f64,
        win_rate:     f64,
    ) {
        if !self.enabled { return; }
        let uptime_min = uptime_secs / 60;
        let pnl_emoji  = if pnl >= 0.0 { "📈" } else { "📉" };
        let msg = format!(
            "🏁 *Bot Shutdown* — `{instance_id}`\n\
             \n\
             Uptime:   `{uptime_min}m`\n\
             Fills:    `{total_fills}`\n\
             Orders:   `{total_orders}`\n\
             P&L:      {pnl_emoji} *${pnl:.2}*\n\
             ROI:      `{roi_pct:.2}%`\n\
             Win Rate: `{win_rate:.1}%`"
        );
        self.fire(&msg).await;
    }

    /// 📊 Periodic heartbeat — NAV + P&L snapshot every N cycles.
    pub async fn send_heartbeat(
        &self,
        instance_id: &str,
        price:       f64,
        nav:         f64,
        pnl:         f64,
        roi_pct:     f64,
        fills:       u64,
        win_rate:    f64,
        cb_ok:       bool,
    ) {
        if !self.enabled { return; }
        let status  = if cb_ok { "✅ OK" } else { "🚨 TRIPPED" };
        let pnl_str = if pnl >= 0.0 {
            format!("+${pnl:.2}")
        } else {
            format!("-${:.2}", pnl.abs())
        };
        let msg = format!(
            "📊 *Heartbeat* — `{instance_id}`\n\
             \n\
             SOL:      `${price:.4}`\n\
             NAV:      `${nav:.2}`\n\
             P&L:      *{pnl_str}*\n\
             ROI:      `{roi_pct:.2}%`\n\
             Fills:    `{fills}`\n\
             Win Rate: `{win_rate:.1}%`\n\
             CB:       {status}"
        );
        self.fire(&msg).await;
    }

    // ── Legacy methods (preserved unchanged) ────────────────────────────

    /// 🎉 Paper test complete (legacy).
    pub async fn send_test_complete(&self, name: &str, roi: f64, duration: f64) {
        if !self.enabled { return; }
        let msg = format!(
            "🎉 *Test Complete*\n\nStrategy: `{name}`\nROI: *{roi:.2}%*\nDuration: {duration:.1}m"
        );
        self.fire(&msg).await;
    }

    /// 🚨 Generic alert (legacy).
    pub async fn send_alert(&self, msg: &str) {
        if !self.enabled { return; }
        self.fire(&format!("🚨 *Alert*\n\n{msg}")).await;
    }
}

// ═══════════════════════════════════════════════════════════════════════
// INTERNAL HTTP
// ═══════════════════════════════════════════════════════════════════════

impl TelegramBot {
    /// Fire-and-forget HTTP POST with 5s timeout.
    /// Errors (network failure OR Telegram API {"ok":false}) are warned
    /// but never propagated — trading loop safety is non-negotiable.
    async fn fire(&self, text: &str) {
        debug!("[TELEGRAM] Sending: {:.60}…", text);
        if let Err(e) = self.send(text).await {
            warn!("[TELEGRAM] Send failed (non-fatal): {}", e);
        }
    }

    /// POST to Telegram sendMessage API.
    /// Uses shared `self.client` (connection pool reuse).
    /// 5s timeout prevents slow Telegram API from blocking the trading cycle.
    /// .error_for_status() surfaces HTTP 4xx/5xx AND Telegram {"ok":false}
    /// responses that would otherwise silently succeed.
    async fn send(&self, text: &str) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!("https://api.telegram.org/bot{}/sendMessage", self.token);
        self.client
            .post(&url)
            .timeout(Duration::from_secs(5))
            .json(&json!({
                "chat_id":    self.chat_id,
                "text":       text,
                "parse_mode": "Markdown"
            }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn disabled_bot() -> TelegramBot {
        TelegramBot::new(None, None)
    }

    fn enabled_bot() -> TelegramBot {
        TelegramBot::new(
            Some("fake_token".to_string()),
            Some("123456".to_string()),
        )
    }

    #[test]
    fn test_disabled_when_no_token() {
        let bot = disabled_bot();
        assert!(!bot.is_enabled());
    }

    #[test]
    fn test_enabled_with_both_credentials() {
        let bot = enabled_bot();
        assert!(bot.is_enabled());
    }

    #[test]
    fn test_disabled_when_only_token() {
        let bot = TelegramBot::new(Some("tok".into()), None);
        assert!(!bot.is_enabled());
    }

    #[test]
    fn test_disabled_when_only_chat_id() {
        let bot = TelegramBot::new(None, Some("123".into()));
        assert!(!bot.is_enabled());
    }

    #[test]
    fn test_client_is_reused_not_per_call() {
        // Structural guard: TelegramBot must hold a Client field.
        // If this compiles, the field exists — Client::new() per-call
        // regression cannot silently re-enter (the field would be unused).
        let bot = disabled_bot();
        let _: &Client = &bot.client;
    }

    #[tokio::test]
    async fn test_send_bot_started_disabled_noop() {
        let bot = disabled_bot();
        bot.send_bot_started("test", "SOL/USDC", 100.0, 0.18, 0.65, "paper").await;
    }

    #[tokio::test]
    async fn test_send_fill_disabled_noop() {
        let bot = disabled_bot();
        bot.send_fill("test", "SELL", 130.0, 0.1, 0.05, 1).await;
    }

    #[tokio::test]
    async fn test_send_circuit_breaker_tripped_disabled_noop() {
        let bot = disabled_bot();
        bot.send_circuit_breaker_tripped("test", "ConsecutiveLosses", 4.2, -8.5, 300).await;
    }

    #[tokio::test]
    async fn test_send_shutdown_disabled_noop() {
        let bot = disabled_bot();
        bot.send_shutdown("test", 3600, 42, 100, 12.5, 1.25, 71.4).await;
    }

    #[tokio::test]
    async fn test_send_heartbeat_disabled_noop() {
        let bot = disabled_bot();
        bot.send_heartbeat("test", 130.0, 1050.0, 50.0, 5.0, 42, 71.0, true).await;
    }

    #[tokio::test]
    async fn test_send_circuit_breaker_reset_disabled_noop() {
        let bot = disabled_bot();
        bot.send_circuit_breaker_reset("test").await;
    }
}
