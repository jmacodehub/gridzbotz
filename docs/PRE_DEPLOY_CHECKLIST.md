# Pre-Deployment Checklist v3.6+

Run these checks before **every mainnet deployment**:

## 🧪 Test Coverage
- [ ] All unit tests passing: `cargo test` (140/141 green, 1 ignored expected)
- [ ] Jupiter API integration: `./scripts/test_jupiter.sh` (live API check)
- [ ] Smoke test devnet: `cargo run --config config/master.toml --duration-minutes 1`

## ⚙️ Configuration
- [ ] `bot_id` validation enforced in `src/config/mod.rs`
- [ ] All production configs have unique `bot_id` set
- [ ] `master.toml` synced with latest defaults

## 🚨 Risk & Safety
- [ ] Circuit breaker thresholds correct (percentage-based)
- [ ] Max drawdown limits appropriate for capital
- [ ] Slippage ranges: 0.2%–5% (dynamic, config-driven)
- [ ] Kill-switch tested with manual breach scenario

## 📊 Observability
- [ ] Logs include `bot_id` in every entry
- [ ] Metrics port unique per bot instance
- [ ] Alert thresholds tuned for production

## 🔐 Security
- [ ] Private keys stored securely (never committed)
- [ ] RPC endpoints: Chainstack primary + QuickNode fallback
- [ ] Jito MEV protection enabled for trades >$500

## 🚀 Deployment
- [ ] All PRs merged and reviewed
- [ ] No compiler warnings (or suppressed with `#![allow(missing_docs)]`)
- [ ] README updated with launch commands
- [ ] Rollback plan documented

---

**✅ All checks passed?** → Ship it! 🔥
