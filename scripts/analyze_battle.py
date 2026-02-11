#!/usr/bin/env python3
"""═══════════════════════════════════════════════════════════════════════════
🤖 AI CONFIG BATTLE ANALYZER
═══════════════════════════════════════════════════════════════════════════

Analyzes battle results and picks the winning configuration based on:
- Total PnL (40% weight)
- Win Rate (20% weight)
- Max Drawdown (20% weight) - lower is better
- Sharpe Ratio (10% weight)
- Consistency Score (10% weight)

USAGE:
    python3 scripts/analyze_battle.py logs/battles/20260211_220000

═══════════════════════════════════════════════════════════════════════════
"""

import json
import sys
from pathlib import Path
from typing import Dict, List
import statistics

# ─────────────────────────────────────────────────────────────────────────────
# 🎨 COLORS
# ─────────────────────────────────────────────────────────────────────────────
class Colors:
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    BLUE = '\033[0;34m'
    MAGENTA = '\033[0;35m'
    CYAN = '\033[0;36m'
    BOLD = '\033[1m'
    RESET = '\033[0m'

# ─────────────────────────────────────────────────────────────────────────────
# 📊 METRICS SCORING
# ─────────────────────────────────────────────────────────────────────────────
WEIGHTS = {
    'pnl': 0.40,
    'win_rate': 0.20,
    'max_drawdown': 0.20,  # Inverted (lower is better)
    'sharpe_ratio': 0.10,
    'consistency': 0.10,
}

def normalize(value: float, min_val: float, max_val: float) -> float:
    """Normalize value to 0-100 range"""
    if max_val == min_val:
        return 50.0
    return ((value - min_val) / (max_val - min_val)) * 100.0

def calculate_consistency(trades: List[float]) -> float:
    """Calculate consistency score based on PnL variance"""
    if len(trades) < 2:
        return 0.0
    
    # Lower variance = higher consistency
    variance = statistics.variance(trades)
    mean_abs = statistics.mean([abs(t) for t in trades])
    
    if mean_abs == 0:
        return 0.0
    
    # Coefficient of variation (inverted)
    cv = (variance ** 0.5) / mean_abs
    consistency = max(0, 100 - (cv * 10))  # Scale to 0-100
    
    return min(100, consistency)

def analyze_config(metrics_file: Path) -> Dict:
    """Analyze a single config's metrics"""
    with open(metrics_file) as f:
        data = json.load(f)
    
    # Extract metrics
    pnl = data.get('total_pnl', 0.0)
    win_rate = data.get('win_rate', 0.0)
    max_drawdown = abs(data.get('max_drawdown', 0.0))  # Make positive
    sharpe = data.get('sharpe_ratio', 0.0)
    
    # Calculate consistency from trade history
    trades = data.get('trade_pnls', [])
    consistency = calculate_consistency(trades) if trades else 0.0
    
    return {
        'pnl': pnl,
        'win_rate': win_rate,
        'max_drawdown': max_drawdown,
        'sharpe_ratio': sharpe,
        'consistency': consistency,
        'total_trades': data.get('total_trades', 0),
        'winning_trades': data.get('winning_trades', 0),
        'losing_trades': data.get('losing_trades', 0),
    }

def calculate_score(metrics: Dict, normalized_ranges: Dict) -> float:
    """Calculate weighted score for a config"""
    score = 0.0
    
    # PnL (higher is better)
    pnl_norm = normalize(
        metrics['pnl'],
        normalized_ranges['pnl']['min'],
        normalized_ranges['pnl']['max']
    )
    score += pnl_norm * WEIGHTS['pnl']
    
    # Win Rate (higher is better)
    wr_norm = normalize(
        metrics['win_rate'],
        normalized_ranges['win_rate']['min'],
        normalized_ranges['win_rate']['max']
    )
    score += wr_norm * WEIGHTS['win_rate']
    
    # Max Drawdown (LOWER is better - invert)
    dd_norm = 100 - normalize(
        metrics['max_drawdown'],
        normalized_ranges['max_drawdown']['min'],
        normalized_ranges['max_drawdown']['max']
    )
    score += dd_norm * WEIGHTS['max_drawdown']
    
    # Sharpe Ratio (higher is better)
    sharpe_norm = normalize(
        metrics['sharpe_ratio'],
        normalized_ranges['sharpe_ratio']['min'],
        normalized_ranges['sharpe_ratio']['max']
    )
    score += sharpe_norm * WEIGHTS['sharpe_ratio']
    
    # Consistency (higher is better)
    consistency_norm = metrics['consistency']  # Already 0-100
    score += consistency_norm * WEIGHTS['consistency']
    
    return score

def main():
    if len(sys.argv) < 2:
        print(f"{Colors.RED}Usage: {sys.argv[0]} <battle_log_directory>{Colors.RESET}")
        sys.exit(1)
    
    log_dir = Path(sys.argv[1])
    if not log_dir.exists():
        print(f"{Colors.RED}Error: Directory not found: {log_dir}{Colors.RESET}")
        sys.exit(1)
    
    # Find all metrics files
    metrics_files = list(log_dir.glob('*_metrics.json'))
    
    if not metrics_files:
        print(f"{Colors.RED}Error: No metrics files found in {log_dir}{Colors.RESET}")
        sys.exit(1)
    
    print(f"{Colors.BOLD}{Colors.MAGENTA}")
    print("═══════════════════════════════════════════════════════════════")
    print("  🤖 AI BATTLE ANALYSIS 🤖")
    print("═══════════════════════════════════════════════════════════════")
    print(f"{Colors.RESET}\n")
    
    # Analyze all configs
    results = {}
    for mf in metrics_files:
        config_name = mf.stem.replace('_metrics', '')
        print(f"{Colors.CYAN}Analyzing: {config_name}...{Colors.RESET}")
        results[config_name] = analyze_config(mf)
    
    print()
    
    # Calculate normalized ranges for scoring
    all_metrics = list(results.values())
    normalized_ranges = {
        'pnl': {
            'min': min(m['pnl'] for m in all_metrics),
            'max': max(m['pnl'] for m in all_metrics),
        },
        'win_rate': {
            'min': min(m['win_rate'] for m in all_metrics),
            'max': max(m['win_rate'] for m in all_metrics),
        },
        'max_drawdown': {
            'min': min(m['max_drawdown'] for m in all_metrics),
            'max': max(m['max_drawdown'] for m in all_metrics),
        },
        'sharpe_ratio': {
            'min': min(m['sharpe_ratio'] for m in all_metrics),
            'max': max(m['sharpe_ratio'] for m in all_metrics),
        },
    }
    
    # Calculate scores
    scores = {}
    for name, metrics in results.items():
        scores[name] = calculate_score(metrics, normalized_ranges)
    
    # Sort by score
    ranked = sorted(scores.items(), key=lambda x: x[1], reverse=True)
    
    # ─────────────────────────────────────────────────────────────────────────
    # 📊 PRINT RESULTS
    # ─────────────────────────────────────────────────────────────────────────
    print(f"{Colors.BOLD}{Colors.BLUE}═══════════════════════════════════════════════════════════════{Colors.RESET}")
    print(f"{Colors.BOLD}{Colors.CYAN}  DETAILED RESULTS{Colors.RESET}")
    print(f"{Colors.BOLD}{Colors.BLUE}═══════════════════════════════════════════════════════════════{Colors.RESET}\n")
    
    for i, (name, score) in enumerate(ranked, 1):
        m = results[name]
        
        medal = "🥇" if i == 1 else "🥈" if i == 2 else "🥉" if i == 3 else "  "
        
        print(f"{Colors.BOLD}{medal} [{i}] {name}{Colors.RESET}")
        print(f"   {Colors.YELLOW}Overall Score: {score:.2f}/100{Colors.RESET}")
        print(f"   PnL:          ${m['pnl']:.2f}")
        print(f"   Win Rate:     {m['win_rate']:.1f}%")
        print(f"   Max Drawdown: {m['max_drawdown']:.2f}%")
        print(f"   Sharpe Ratio: {m['sharpe_ratio']:.2f}")
        print(f"   Consistency:  {m['consistency']:.1f}/100")
        print(f"   Trades:       {m['total_trades']} ({m['winning_trades']}W / {m['losing_trades']}L)")
        print()
    
    # ─────────────────────────────────────────────────────────────────────────
    # 🏆 WINNER
    # ─────────────────────────────────────────────────────────────────────────
    winner_name, winner_score = ranked[0]
    
    print(f"{Colors.BOLD}{Colors.GREEN}═══════════════════════════════════════════════════════════════{Colors.RESET}")
    print(f"{Colors.BOLD}{Colors.YELLOW}  🏆 WINNER: {winner_name} 🏆{Colors.RESET}")
    print(f"{Colors.BOLD}{Colors.YELLOW}  Score: {winner_score:.2f}/100{Colors.RESET}")
    print(f"{Colors.BOLD}{Colors.GREEN}═══════════════════════════════════════════════════════════════{Colors.RESET}\n")
    
    # ─────────────────────────────────────────────────────────────────────────
    # 📝 RECOMMENDATIONS
    # ─────────────────────────────────────────────────────────────────────────
    print(f"{Colors.BOLD}{Colors.CYAN}📝 RECOMMENDATIONS:{Colors.RESET}\n")
    
    winner_metrics = results[winner_name]
    
    if winner_metrics['pnl'] > 0:
        print(f"{Colors.GREEN}✅ Positive PnL - Good for production!{Colors.RESET}")
    else:
        print(f"{Colors.RED}⚠️  Negative PnL - Needs optimization{Colors.RESET}")
    
    if winner_metrics['win_rate'] >= 55:
        print(f"{Colors.GREEN}✅ Strong win rate (>55%){Colors.RESET}")
    elif winner_metrics['win_rate'] >= 50:
        print(f"{Colors.YELLOW}⚠️  Moderate win rate (50-55%) - Room for improvement{Colors.RESET}")
    else:
        print(f"{Colors.RED}⚠️  Low win rate (<50%) - Consider adjusting grid spacing{Colors.RESET}")
    
    if winner_metrics['max_drawdown'] < 5.0:
        print(f"{Colors.GREEN}✅ Low drawdown (<5%) - Safe for scaling{Colors.RESET}")
    elif winner_metrics['max_drawdown'] < 10.0:
        print(f"{Colors.YELLOW}⚠️  Moderate drawdown (5-10%) - Scale cautiously{Colors.RESET}")
    else:
        print(f"{Colors.RED}⚠️  High drawdown (>10%) - Reduce position sizes{Colors.RESET}")
    
    print(f"\n{Colors.CYAN}Next steps:{Colors.RESET}")
    print(f"  1. Test {winner_name} on devnet")
    print(f"  2. Paper trade on mainnet for 24-48h")
    print(f"  3. Start live with 0.05 SOL positions")
    print(f"  4. Scale up gradually if performance holds")
    
    print(f"\n{Colors.BOLD}{Colors.GREEN}🎉 Analysis complete! {Colors.RESET}\n")

if __name__ == '__main__':
    main()
