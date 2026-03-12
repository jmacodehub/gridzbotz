# 📲 Telegram Alerts Setup

GridzBotz fires real-time mobile alerts for every significant trading event.
Setup takes under 5 minutes. Zero code changes required — just two env vars.

---

## 1 — Create Your Bot (BotFather)

1. Open Telegram and search **@BotFather**
2. Send `/newbot`
3. Choose a name — e.g. `GridzBotz Alerts`
4. Choose a username — e.g. `gridzbotz_alerts_bot` (must end in `bot`)
5. BotFather replies with your **bot token**:
   ```
   7412345678:AAF_abc123XYZ...
   ```
   Copy it — you'll need it in Step 3.

---

## 2 — Get Your Chat ID

1. Start a conversation with your new bot — send it any message (e.g. `/start`)
2. Open this URL in your browser (replace `<TOKEN>` with your token):
   ```
   https://api.telegram.org/bot<TOKEN>/getUpdates
   ```
3. Find `"chat":{"id":XXXXXXXXX}` in the JSON response
4. Copy that number — that's your **chat ID**

> **Tip:** If the JSON is empty, send another message to the bot first, then refresh the URL.

---

## 3 — Set Environment Variables

### Linux / macOS (current session)
```bash
export GRIDZBOTZ_TELEGRAM_TOKEN=7412345678:AAF_abc123XYZ...
export GRIDZBOTZ_TELEGRAM_CHAT_ID=123456789
```

### Persist across sessions (add to `~/.bashrc` or `~/.zshrc`)
```bash
echo 'export GRIDZBOTZ_TELEGRAM_TOKEN=7412345678:AAF_abc123XYZ...' >> ~/.bashrc
echo 'export GRIDZBOTZ_TELEGRAM_CHAT_ID=123456789' >> ~/.bashrc
source ~/.bashrc
```

### Verify
```bash
echo $GRIDZBOTZ_TELEGRAM_TOKEN   # should print your token
echo $GRIDZBOTZ_TELEGRAM_CHAT_ID # should print your chat ID
```

---

## 4 — Test in Paper Mode

```bash
cargo build --release
cargo run --release
```

On startup you should see in logs:
```
[TELEGRAM] Enabled — alerts will fire to chat 123456789
```

And immediately receive on your phone:
```
🚀 GridzBot Started

Instance: sol-usdc-01
Pair:     SOL/USDC
Capital:  $1000.00
Spacing:  0.180%
WMA Gate: 0.65
Mode:     paper
```

If you see `[TELEGRAM] Disabled` — one of the env vars is missing or empty. Re-check Step 3.

---

## 5 — Alert Reference

| Alert | Trigger | Emoji |
|-------|---------|-------|
| **Bot Started** | Once on grid initialization | 🚀 |
| **Fill** | Every confirmed BUY or SELL | 💰 / 🛒 |
| **Circuit Breaker Tripped** | CB trips (drawdown / consecutive losses) | 🚨 |
| **Circuit Breaker Reset** | CB cooldown elapsed, trading resumed | ✅ |
| **Heartbeat** | Every `metrics.stats_interval` cycles | 📊 |
| **Shutdown** | Graceful bot shutdown (Ctrl+C) | 🏁 |

### Heartbeat cadence
Controlled by `metrics.stats_interval` in your TOML:
```toml
[metrics]
stats_interval = 100  # fires every 100 price cycles (~50s at 500ms tick)
```
Set to `0` to disable heartbeats.

---

## 6 — Sample Alert Messages

### 💰 Fill
```
💰 Fill #42 — sol-usdc-01

Side:  SELL
Price: $183.4210
Size:  0.1000 SOL
P&L:   +$0.0328
```

### 🚨 Circuit Breaker Tripped
```
🚨 CIRCUIT BREAKER TRIPPED 🚨

Instance:  sol-usdc-01
Reason:    ConsecutiveLosses
Drawdown:  4.20%
P&L:       $-8.50
Cooldown:  300s

Trading halted. Bot will resume after cooldown.
```

### 📊 Heartbeat
```
📊 Heartbeat — sol-usdc-01

SOL:      $183.4210
NAV:      $1050.00
P&L:      +$50.00
ROI:      5.00%
Fills:    42
Win Rate: 71.4%
CB:       ✅ OK
```

### 🏁 Shutdown
```
🏁 Bot Shutdown — sol-usdc-01

Uptime:   142m
Fills:    87
Orders:   312
P&L:      📈 $112.50
ROI:      11.25%
Win Rate: 73.6%
```

---

## 7 — Timeout & Failure Behaviour

All Telegram calls have a **5-second timeout** (PR #103). If the Telegram API
is slow or unreachable:
- The trading cycle is **never blocked** — alerts are fire-and-forget
- A `WARN [TELEGRAM] Send failed (non-fatal): ...` line appears in logs
- The bot continues trading normally

Bad credentials (wrong token or chat ID) produce:
```
WARN [TELEGRAM] Send failed (non-fatal): HTTP status client error (401 Unauthorized)
```
This means your token or chat ID is incorrect — re-run Step 3.

---

## 8 — Multi-Bot Setups

Each bot instance reads `GRIDZBOTZ_TELEGRAM_TOKEN` and `GRIDZBOTZ_TELEGRAM_CHAT_ID`
from the environment. All instances alert to the same chat by default.

To route different bots to different chats, launch each instance in its own
shell with its own exported `GRIDZBOTZ_TELEGRAM_CHAT_ID`:

```bash
# Terminal 1 — SOL/USDC bot
export GRIDZBOTZ_TELEGRAM_CHAT_ID=111111111
cargo run --release -- --config config/sol_usdc_production.toml

# Terminal 2 — JTO/USDC bot
export GRIDZBOTZ_TELEGRAM_CHAT_ID=222222222
cargo run --release -- --config config/jto_usdc_production.toml
```

---

## Quick Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `[TELEGRAM] Disabled` in logs | Env vars not set or empty | Re-export in current shell |
| `401 Unauthorized` warn | Wrong token | Re-copy from BotFather |
| `400 Bad Request` warn | Wrong chat ID | Re-run getUpdates |
| No startup message | Bot never messaged / chat not initiated | Send `/start` to bot first |
| Empty `getUpdates` | No messages sent to bot yet | Send any message to bot, refresh URL |
