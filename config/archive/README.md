# 🗄️ Archived Configs

These configs are superseded and archived here for historical reference.
All active development uses:
- `config/production/mainnet-sol-usdc-v1.toml` — live mainnet
- `config/optimized/tuned-mar2-2026.toml` — latest paper-validated params
- `config/master.toml` — canonical base template
- `config/rd/` — experimental R&D profiles

## What's Archived

| Config | Generation | Reason Archived |
|--------|------------|-----------------|
| `overnight_aggressive.toml` | v3.5 | Superseded by `production/mainnet-sol-usdc-v1.toml` |
| `overnight_balanced.toml` | v3.5 | Superseded |
| `overnight_conservative.toml` | v3.5 | Superseded |
| `overnight_multi_strategy.toml` | v3.5 | Superseded |
| `overnight_super_aggressive.toml` | v3.5 | Superseded |
| `overnight_testing.toml` | v3.5 | Superseded by `testing/smoke_test.toml` |
| `overnight_ultra_aggressive.toml` | v3.5 | Superseded |
| `elite/scalper.toml` | v5.3 | Params extracted → `config/rd/scalper-profile.toml` |
| `elite/night_owl.toml` | v5.3 | Superseded by `optimized/tuned-mar2-2026.toml` |
| `elite/momentum_hunter.toml` | v5.3 | Superseded |
| `elite/volatility_farmer.toml` | v5.3 | Superseded |
| `production/aggressive.toml` | v4 | Superseded |
| `production/balanced.toml` | v4 | Superseded |
| `production/conservative.toml` | v4 | Superseded |
| `production/multi_strategy.toml` | v4 | Superseded |
| `production/ultra_aggressive.toml` | v4 | Superseded |
| `optimized/multi-v5-ai.toml` | v5 | Old schema (incompatible). Params → `config/rd/` |
| `optimized/multi_strategy_v4_conservative_ai.toml` | v4 AI | Params → `config/rd/multi-strategy-profile.toml` |
| `optimized/balanced-v4.1.toml` | v4.1 | Superseded |
| `optimized/balanced_v4.toml` | v4.0 | Superseded |
| `optimized/conservative-v4.1.toml` | v4.1 | Superseded |
| `optimized/conservative_v4.toml` | v4.0 | Superseded |

## Key Learnings Extracted Before Archiving

The following params were promoted to `master.toml` and `config/rd/` configs:

1. **`max_volatility_to_trade = 8.0`** — from `multi-v5-ai.toml` (regime gate HIGH ceiling — was missing from master)
2. **`max_trade_size_usdc` in `[execution]`** — from `mainnet-sol-usdc-v1.toml` (was silently ignored in `[security]`)
3. **Scalper profile** — from `elite/scalper.toml`: 0.08% spacing, 30s rebalance cooldown, 5min lifecycle, `pause_in_very_low_vol=false`
4. **Multi-strategy consensus** — from `multi_strategy_v4_conservative_ai.toml`: `consensus_mode="weighted"`, 22 levels, RSI weight=0.9
5. **Indicator canonical defaults** — from `multi-v5-ai.toml`: ATR(14,2.0), RSI(14,30/70), BB(20,2.0), MACD(12,26,9)
6. **`max_orders_per_side = grid_levels/2`** mathematical relationship — documented in master.toml

## Git History

Full file content is preserved in git history. To recover any archived config:
```bash
git log --all --oneline -- config/archive/
git show <commit-sha>:config/overnight_aggressive.toml
```
