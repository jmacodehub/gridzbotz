# 🤖 Multi-Bot Deployment Guide

**GridzBotz V3.6+** supports running **N independent bot instances** simultaneously, each with its own config, pair, strategy, and risk limits. No shared state — complete isolation.

---

## 🚀 Quick Start

### Launch Multiple Bots

```bash
# SOL/USDC aggressive
cargo run --release -- --config config/production/sol_usdc_aggressive.toml &

# SOL/USDC conservative (shadow copy for comparison)
cargo run --release -- --config config/production/sol_usdc_conservative.toml &

# BONK/USDC high-volatility
cargo run --release -- --config config/production/bonk_usdc.toml &
```

Each bot:
- Runs in its own process
- Has independent logs (`logs/sol_agg.log`, `logs/bonk_1.log`, ...)
- Reports to its own metrics port (`:9090`, `:9091`, ...)
- Isolated risk limits (one bot's kill-switch never affects others)

---

## ⚙️ Per-Bot Config Requirements

Every bot config **MUST** define these unique fields:

### 1. **bot.instance_id**
Unique label appearing in all logs and metrics.

```toml
[bot]
instance_id = "sol_agg"    # SOL aggressive bot
# instance_id = "bonk_1"   # BONK bot
# instance_id = "shadow_1" # Shadow copy for A/B testing
```

### 2. **Token Pair**
Define the trading pair and mint addresses.

```toml
# SOL/USDC example
base_mint = "So11111111111111111111111111111111111111112"  # SOL
quote_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v" # USDC
```

### 3. **Strategy Mode**
Each bot can run a different strategy.

```toml
[strategies]
active = ["grid"]           # Bot 1: Pure grid
# active = ["momentum"]     # Bot 2: Momentum-only
# active = ["grid", "rsi"]  # Bot 3: Multi-strategy consensus
```

### 4. **Independent Risk Limits**
No shared risk — one bot's drawdown never trips another's breaker.

```toml
[risk]
max_drawdown_pct = 10.0             # Bot 1: conservative
# max_drawdown_pct = 15.0           # Bot 2: aggressive
circuit_breaker_threshold_pct = 8.0
```

### 5. **RPC Configuration**
Bots can share RPC pools **OR** use dedicated endpoints.

```toml
[network]
rpc_url = "${CHAINSTACK_RPC_URL}"   # Shared primary (high rate limit)
# rpc_url = "https://my-dedicated-node.com"  # Dedicated for critical bots
```

---

## ✅ Isolation Guarantees

| Aspect | Guarantee |
|--------|----------|
| **State** | Zero shared mutable state between instances |
| **Risk** | Independent `max_drawdown`, `daily_loss_cap`, kill-switches |
| **Metrics** | Per-bot metrics port (`:9090`, `:9091`, ...) |
| **Logs** | Separate log files tagged with `bot.instance_id` |
| **RPC** | Optional: dedicated RPC pools per bot (no rate-limit bleed) |
| **Kill-Switch** | One bot's circuit breaker ≠ affects others |

**Result:** You can run aggressive + conservative bots side-by-side. If the aggressive bot trips its breaker, the conservative bot keeps trading.

---

## 📊 Monitoring

### View All Bot Logs

```bash
# Tail all logs simultaneously
tail -f logs/*.log

# Filter by bot_id
grep "sol_agg" logs/*.log
```

### Check Metrics (Prometheus)

```bash
# Bot 1 (default port 9090)
curl http://localhost:9090/metrics | grep bot_id

# Bot 2 (port 9091)
curl http://localhost:9091/metrics | grep bot_id
```

Each bot exposes:
- `fills_total{bot_id="sol_agg"}` — total fills
- `pnl_total{bot_id="sol_agg"}` — cumulative P&L
- `circuit_breaker_trips{bot_id="sol_agg"}` — breaker count

### Stop a Specific Bot

```bash
# Find the bot's PID
ps aux | grep gridzbotz

# Kill only that bot
kill <PID>
```

Other bots continue unaffected.

---

## 📁 Example: 3-Bot Fleet

### Config Files

```
config/production/
├── sol_usdc_aggressive.toml   # 35 levels, 0.15% spacing, 15% drawdown
├── sol_usdc_conservative.toml # 25 levels, 0.25% spacing, 8% drawdown
└── bonk_usdc.toml             # 50 levels, 0.10% spacing, 20% drawdown
```

### Launch Script (`scripts/launch_fleet.sh`)

```bash
#!/bin/bash
set -e

echo "🚀 Launching 3-bot fleet..."

# Bot 1: SOL aggressive
cargo run --release -- \
  --config config/production/sol_usdc_aggressive.toml \
  > logs/sol_agg.log 2>&1 &
echo "  ✅ Bot 1 (sol_agg) started: PID $!"

# Bot 2: SOL conservative
cargo run --release -- \
  --config config/production/sol_usdc_conservative.toml \
  > logs/sol_con.log 2>&1 &
echo "  ✅ Bot 2 (sol_con) started: PID $!"

# Bot 3: BONK high-vol
cargo run --release -- \
  --config config/production/bonk_usdc.toml \
  > logs/bonk_1.log 2>&1 &
echo "  ✅ Bot 3 (bonk_1) started: PID $!"

echo "✅ Fleet launched! Monitor with: tail -f logs/*.log"
```

### Daily P&L Summary

```bash
# Aggregate P&L across all bots
grep "Daily P&L" logs/*.log | awk '{sum+=$NF} END {print "Total: $" sum}'
```

---

## 🚨 Common Pitfalls

### ❌ **Don't: Share `instance_id`**

```toml
# BAD: Both bots use the same instance_id
# sol_usdc_1.toml
instance_id = "prod_bot"

# bonk_usdc_1.toml
instance_id = "prod_bot"   # ❌ COLLISION! Logs will be mixed
```

**Fix:** Unique `instance_id` per bot.

### ❌ **Don't: Reuse Metrics Ports**

```toml
# BAD: Both bots try to bind to port 9090
# Bot 1
[metrics]
port = 9090

# Bot 2
[metrics]
port = 9090   # ❌ PORT CONFLICT! Second bot will crash
```

**Fix:** Increment ports: `9090`, `9091`, `9092`, ...

### ❌ **Don't: Assume Shared State**

Bots **DO NOT** share:
- Grid levels
- Open orders
- Risk limits
- Circuit breaker state

Each bot is **fully independent**. If you want coordinated behavior (e.g., "pause all bots if total fleet drawdown > 20%"), you need external orchestration (future feature).

---

## 🔮 Future: Fleet Orchestration (Stage 5+)

**Planned features:**

- **Fleet-level risk:** Aggregate drawdown across all bots
- **Coordinated pauses:** One master kill-switch for N bots
- **Auto-rebalancing:** Shift capital from underperforming → hot bots
- **Central dashboard:** Single UI to monitor all instances

**Status:** Stubbed in `master.toml` RESERVED section. Coming in V4.0+.

---

## ✅ Best Practices

1. **Start small:** Run 1-2 bots in paper mode first
2. **Unique IDs:** Every bot gets a clear, descriptive `instance_id`
3. **Independent risk:** Don't exceed total capital = sum of all bot limits
4. **Monitor continuously:** Use `tail -f logs/*.log` during first 24h
5. **A/B test:** Run aggressive + conservative configs on the same pair to compare

---

## 📚 See Also

- [master.toml RESERVED section](../config/master.toml) — Future multi-bot features
- [CHANGELOG V3.6](../CHANGELOG_V3.6.md) — Per-bot instance_id added
- [Risk Management](./RISK_MANAGEMENT.md) — Independent circuit breakers

---

**LFG! 🚀** Deploy your fleet and make it rain.
