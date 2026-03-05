# 🔬 R&D Configs

Experimental trading profiles extracted from historical testing and archived configs.
These represent **validated parameter research** — not production-ready but backed by real data.

All profiles run against devnet in paper mode before promotion.

## Profiles

### `scalper-profile.toml`
- **Source**: `config/elite/scalper.toml` (archived)
- **Market**: Stable sideways, ATR < 0.5, vol 0.01%–0.08%
- **Grid**: 50 levels × 0.08% spacing (±4% coverage)
- **Key findings**:
  - 0.08% = tightest viable spacing on SOL/USDC (Pyth confidence ±0.03%)
  - `rebalance_cooldown_secs = 30` (fastest ever tested)
  - `pause_in_very_low_vol = false` — scalpers WANT quiet markets
  - `request_timeout_ms = 3000` — stale data kills scalpers, fail fast
  - `stats_interval = 100` — 10-second feedback loops
- **Status**: Ready for 24h paper backtest → compare fills vs `mainnet-sol-usdc-v1`
- **Run**: `cargo run --release -- --config config/rd/scalper-profile.toml --paper`

### `multi-strategy-profile.toml`
- **Source**: `multi_strategy_v4_conservative_ai.toml` + `multi-v5-ai.toml` (archived)
- **Market**: Trending/momentum phases
- **Strategies**: Grid + Momentum + RSI + MACD (`consensus_mode = "weighted"`)
- **Key findings**:
  - `consensus_mode = "weighted"` outperforms `"single"` in trending regimes
  - RSI `weight = 0.9` is highest non-grid signal weight
  - `min_warmup_periods = 26` is non-negotiable for MACD (never reduce)
  - 22 levels + 0.25% spacing outperforms 35 + 0.15% in fill efficiency
  - `max_orders_per_side = grid_levels / 2` (mathematical relationship)
  - Regime gate HIGH threshold: `max_volatility_to_trade = 8.0`
- **Status**: Experimental — requires 48h comparison vs grid-only baseline
- **Run**: `cargo run --release -- --config config/rd/multi-strategy-profile.toml --paper`

## Canonical Indicator Defaults

Extracted from `multi-v5-ai.toml` research (Feb 2026). Use these as the standard baseline:

| Indicator      | Param           | Value |
|----------------|-----------------|-------|
| ATR            | period          | 14    |
| ATR            | multiplier      | 2.0   |
| RSI            | period          | 14    |
| RSI            | oversold        | 30    |
| RSI            | overbought      | 70    |
| Bollinger Band | period          | 20    |
| Bollinger Band | std_dev         | 2.0   |
| MACD           | fast_period     | 12    |
| MACD           | slow_period     | 26    |
| MACD           | signal_period   | 9     |

## Workflow

```
R&D (this folder)
  → Paper Test (24-48h, --paper)
  → Compare vs mainnet-sol-usdc-v1 baseline
  → Tune params
  → Promote to config/optimized/
  → Validate in optimized/
  → Promote to config/production/
```

> **Never promote directly to `config/production/` without a validated `config/optimized/` entry.**
