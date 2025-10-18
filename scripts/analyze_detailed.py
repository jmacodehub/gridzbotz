#!/usr/bin/env python3
"""
═══════════════════════════════════════════════════════════════════════════
📊 PROJECT FLASH V3.5 - COMPREHENSIVE RESULTS ANALYZER
Production-grade analysis with detailed metrics, ROI, and recommendations
Version: 3.5.1 - Fixed spacing display + ROI extraction
═══════════════════════════════════════════════════════════════════════════
"""

import re
from pathlib import Path
from datetime import datetime
from typing import Dict
import json

class BotAnalyzer:
    def __init__(self, results_dir: str):
        self.results_dir = Path(results_dir)
        self.bots = []
        
    def analyze_bot(self, txt_file: Path) -> Dict:
        """Extract comprehensive metrics from a bot's output file"""
        bot_name = txt_file.stem
        
        with open(txt_file, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
        
        # Basic metrics
        cycles = re.findall(r'Cycle\s+(\d+)/(\d+)', content)
        last_cycle = int(cycles[-1][0]) if cycles else 0
        total_cycles = int(cycles[-1][1]) if cycles else 0
        
        # Trading activity
        trades = content.count("orders filled")
        repos = content.count("🔄 Rebalanced") + content.count("Rebalanced")
        paused = content.count("🚫 Paused")
        
        # Grid info
        grid_init = "Placed" in content and "orders (" in content
        grid_orders = re.findall(r'Placed (\d+) orders', content)
        initial_orders = int(grid_orders[0]) if grid_orders else 0
        
        # Extract ROI from final status (if available)
        roi_matches = re.findall(r'ROI:\s+([-]?\d+\.\d+)%', content)
        roi = float(roi_matches[-1]) if roi_matches else 0.0
        
        # Extract P&L
        pnl_matches = re.findall(r'P&L:\s+\$?([-]?\d+\.\d+)', content)
        pnl = float(pnl_matches[-1]) if pnl_matches else 0.0
        
        # Extract Total Value
        value_matches = re.findall(r'Total Value:\s+\$?([-]?\d+\.\d+)', content)
        total_value = float(value_matches[-1]) if value_matches else 0.0
        
        # 🔥 FIXED: Correct spacing percentages
        spacing_map = {
            'conservative': 0.30,
            'balanced': 0.15,
            'aggressive': 0.10,
            'super_aggressive': 0.07,
            'ultra_aggressive': 0.03,
            'testing': 0.15,
            'multi_strategy': 0.20
        }
        spacing = spacing_map.get(bot_name, 0.15)
        
        # Performance metrics
        completion_pct = (last_cycle / total_cycles * 100) if total_cycles > 0 else 0
        trades_per_hour = (trades / 9.3) if last_cycle > 0 else 0
        cycles_per_hour = (last_cycle / 9.3) if last_cycle > 0 else 0
        
        # Efficiency
        trade_frequency = (trades / last_cycle * 100) if last_cycle > 0 else 0
        reposition_frequency = (repos / last_cycle * 1000) if last_cycle > 0 else 0
        
        # Calculate estimated fees (0.04% per trade)
        estimated_fees = trades * 0.0004 * 100  # Assuming $100 per trade
        
        return {
            'name': bot_name,
            'display_name': bot_name.replace('_', ' ').title(),
            'spacing': spacing,
            'cycle': last_cycle,
            'total': total_cycles,
            'completion': completion_pct,
            'trades': trades,
            'repos': repos,
            'paused': paused,
            'grid_init': grid_init,
            'initial_orders': initial_orders,
            'trades_per_hour': trades_per_hour,
            'cycles_per_hour': cycles_per_hour,
            'trade_frequency': trade_frequency,
            'reposition_frequency': reposition_frequency,
            'roi': roi,
            'pnl': pnl,
            'total_value': total_value,
            'estimated_fees': estimated_fees
        }
    
    def load_all_bots(self):
        """Load and analyze all bot results"""
        for txt_file in sorted(self.results_dir.glob("*.txt")):
            if txt_file.stem == "SUITE_INFO":
                continue
            bot_data = self.analyze_bot(txt_file)
            self.bots.append(bot_data)
    
    def generate_report(self):
        """Generate comprehensive analysis report"""
        
        # Header
        print("\n" + "="*110)
        print("  🚀 PROJECT FLASH V3.5 - COMPREHENSIVE OVERNIGHT TEST ANALYSIS")
        print("="*110)
        print(f"\n  📅 Test Date:     October 18, 2025")
        print(f"  ⏰ Duration:      ~9.3 hours (00:42 - 10:01)")
        print(f"  🤖 Bots Tested:   7 configurations")
        print(f"  📊 Data Points:   {sum(b['cycle'] for b in self.bots):,} total cycles")
        print(f"  💰 Total Trades:  {sum(b['trades'] for b in self.bots)} executions")
        print("\n" + "="*110)
        
        # Main Performance Table
        print("\n📊 PERFORMANCE OVERVIEW")
        print("-"*110)
        print(f"{'Bot':<25} {'Spacing':<10} {'Progress':<18} {'Trades':<8} {'T/Hour':<8} {'ROI':<10} {'P&L':<10}")
        print("-"*110)
        
        for bot in sorted(self.bots, key=lambda x: x['trades'], reverse=True):
            progress = f"{bot['cycle']:>6,}/{bot['total']:<6,}"
            roi_str = f"{bot['roi']:+.2f}%" if bot['roi'] != 0 else "N/A"
            pnl_str = f"${bot['pnl']:+.2f}" if bot['pnl'] != 0 else "N/A"
            
            print(f"{bot['display_name']:<25} "
                  f"{bot['spacing']:.2f}%     "
                  f"{progress:<18} "
                  f"{bot['trades']:<8} "
                  f"{bot['trades_per_hour']:<8.1f} "
                  f"{roi_str:<10} "
                  f"{pnl_str:<10}")
        
        # Detailed Metrics
        print("\n" + "="*110)
        print("📈 DETAILED METRICS")
        print("-"*110)
        
        for bot in sorted(self.bots, key=lambda x: x['trades'], reverse=True):
            print(f"\n🎯 {bot['display_name']} ({bot['spacing']:.2f}% spacing)")
            print(f"  ├─ Completion:      {bot['completion']:.1f}% ({bot['cycle']:,}/{bot['total']:,} cycles)")
            print(f"  ├─ Trades:          {bot['trades']} total ({bot['trades_per_hour']:.1f}/hour)")
            print(f"  ├─ Trade Freq:      {bot['trade_frequency']:.3f}% of cycles")
            print(f"  ├─ Repositions:     {bot['repos']} ({bot['reposition_frequency']:.2f} per 1000 cycles)")
            print(f"  ├─ Paused:          {bot['paused']} times")
            print(f"  ├─ Grid Orders:     {bot['initial_orders']} {'✅' if bot['grid_init'] else '❌'}")
            if bot['roi'] != 0:
                print(f"  ├─ ROI:             {bot['roi']:+.2f}%")
                print(f"  ├─ P&L:             ${bot['pnl']:+.2f}")
                print(f"  └─ Portfolio Value: ${bot['total_value']:.2f}")
            else:
                print(f"  └─ ROI:             Not yet calculated")
        
        # Rankings
        print("\n" + "="*110)
        print("🏆 RANKINGS & INSIGHTS")
        print("-"*110)
        
        print("\n🥇 Top 3 by Trades:")
        for i, bot in enumerate(sorted(self.bots, key=lambda x: x['trades'], reverse=True)[:3], 1):
            print(f"  {i}. {bot['display_name']}: {bot['trades']} trades ({bot['spacing']:.2f}% spacing)")
        
        print("\n📊 Top 3 by Trade Frequency:")
        for i, bot in enumerate(sorted(self.bots, key=lambda x: x['trade_frequency'], reverse=True)[:3], 1):
            print(f"  {i}. {bot['display_name']}: {bot['trade_frequency']:.3f}% ({bot['trades']}/{bot['cycle']:,} cycles)")
        
        # ROI Rankings (only non-zero)
        bots_with_roi = [b for b in self.bots if b['roi'] != 0]
        if bots_with_roi:
            print("\n💰 Top 3 by ROI:")
            for i, bot in enumerate(sorted(bots_with_roi, key=lambda x: x['roi'], reverse=True)[:3], 1):
                print(f"  {i}. {bot['display_name']}: {bot['roi']:+.2f}% (${bot['pnl']:+.2f})")
        
        print("\n🔄 Top 3 by Repositions:")
        for i, bot in enumerate(sorted(self.bots, key=lambda x: x['repos'], reverse=True)[:3], 1):
            print(f"  {i}. {bot['display_name']}: {bot['repos']} repositions")
        
        # Analysis
        print("\n" + "="*110)
        print("🔍 STRATEGIC ANALYSIS")
        print("-"*110)
        
        total_trades = sum(b['trades'] for b in self.bots)
        avg_trades = total_trades / len(self.bots)
        total_fees = sum(b['estimated_fees'] for b in self.bots)
        
        winner = max(self.bots, key=lambda x: x['trades'])
        most_efficient = max(self.bots, key=lambda x: x['trade_frequency'])
        most_stable = min(self.bots, key=lambda x: x['repos'])
        
        print(f"\n💡 Key Findings:")
        print(f"  • Total trades executed:      {total_trades}")
        print(f"  • Average per bot:            {avg_trades:.1f} trades")
        print(f"  • Estimated total fees:       ${total_fees:.2f}")
        print(f"  • Winner (most trades):       {winner['display_name']} with {winner['trades']} trades")
        print(f"  • Most efficient:             {most_efficient['display_name']} ({most_efficient['trade_frequency']:.3f}%)")
        print(f"  • Most stable (few repos):    {most_stable['display_name']} ({most_stable['repos']} repositions)")
        
        # Spacing vs Performance Correlation
        print(f"\n📉 Spacing vs Performance Correlation:")
        max_trades = max(b['trades'] for b in self.bots)
        for bot in sorted(self.bots, key=lambda x: x['spacing'], reverse=True):
            bar_length = int((bot['trades'] / max_trades) * 50)
            bar = '█' * bar_length
            print(f"  {bot['spacing']:.2f}% ({bot['display_name']:<20}): {bar} {bot['trades']}")
        
        # Recommendations
        print("\n" + "="*110)
        print("💡 RECOMMENDATIONS FOR PRODUCTION")
        print("-"*110)
        
        print("\n🎯 Best Overall Performance:")
        print(f"   {winner['display_name']} ({winner['spacing']:.2%} spacing)")
        print(f"   └─ Rationale: Highest trade count with {winner['trades']} trades ({winner['trades_per_hour']:.1f}/hour)")
        
        if winner['spacing'] < 0.05:
            print(f"\n⚠️  Warning: Ultra-tight spacing ({winner['spacing']:.2%}) may incur high fees!")
            print(f"   Estimated fees for {winner['display_name']}: ${winner['estimated_fees']:.2f}")
            balanced = next((b for b in self.bots if 'balanced' in b['name'].lower()), None)
            if balanced:
                print(f"   Consider: {balanced['display_name']} ({balanced['spacing']:.2%}) for better risk/reward")
                print(f"   Estimated fees for {balanced['display_name']}: ${balanced['estimated_fees']:.2f}")
        
        print("\n🏆 Production Strategy Recommendations:")
        
        # Find configs
        conservative = next((b for b in self.bots if 'conservative' in b['name']), None)
        balanced = next((b for b in self.bots if 'balanced' in b['name']), None)
        aggressive = next((b for b in self.bots if 'aggressive' in b['name'] and 'super' not in b['name'] and 'ultra' not in b['name']), None)
        
        print("\n  1️⃣  Conservative Portfolio (Low Risk):")
        if conservative:
            print(f"      Config: {conservative['display_name']} ({conservative['spacing']:.2%})")
            print(f"      Results: {conservative['trades']} trades, {conservative['repos']} repos")
            print(f"      Fees: ~${conservative['estimated_fees']:.2f}")
            print(f"      Use: 30% of capital for steady, low-risk gains")
        
        print("\n  2️⃣  Balanced Portfolio (Standard):")
        if balanced:
            print(f"      Config: {balanced['display_name']} ({balanced['spacing']:.2%})")
            print(f"      Results: {balanced['trades']} trades, {balanced['repos']} repos")
            print(f"      Fees: ~${balanced['estimated_fees']:.2f}")
            print(f"      Use: 50% of capital as primary strategy")
        
        print("\n  3️⃣  Aggressive Portfolio (High Frequency):")
        if aggressive:
            print(f"      Config: {aggressive['display_name']} ({aggressive['spacing']:.2%})")
            print(f"      Results: {aggressive['trades']} trades, {aggressive['repos']} repos")
            print(f"      Fees: ~${aggressive['estimated_fees']:.2f}")
            print(f"      Use: 20% of capital during volatile markets")
        
        # Next Steps
        print("\n" + "="*110)
        print("🚀 NEXT STEPS")
        print("-"*110)
        
        print("\n  Phase 1: Immediate (Today)")
        print("    • Run full 24-hour test with top 3 performers")
        print("    • Use: caffeinate -dims ./scripts/launch_ultimate_suite.sh")
        print("    • Monitor every 4 hours with: ./scripts/monitor_suite.sh")
        
        print("\n  Phase 2: This Week")
        print("    • Fine-tune winning config parameters")
        print("    • Test in different market conditions (volatile vs calm)")
        print("    • Validate with 3-day continuous run")
        print("    • Calculate actual ROI with real price movements")
        
        print("\n  Phase 3: Next Week")
        print("    • Implement real multi-strategy consensus (RSI + Momentum)")
        print("    • Add OpenBook DEX integration")
        print("    • Begin small position testing on devnet")
        print("    • Set up automated reporting")
        
        # Export
        print("\n" + "="*110)
        print("💾 DATA EXPORT")
        print("-"*110)
        
        csv_file = self.results_dir / f"detailed_analysis_{datetime.now().strftime('%Y%m%d_%H%M%S')}.csv"
        with open(csv_file, 'w') as f:
            f.write("Bot,Spacing,Cycles,Total,Completion,Trades,Repos,Paused,TradesPerHour,TradeFrequency,ROI,PNL,EstimatedFees\n")
            for bot in self.bots:
                f.write(f"{bot['name']},{bot['spacing']},{bot['cycle']},{bot['total']},"
                       f"{bot['completion']:.2f},{bot['trades']},{bot['repos']},{bot['paused']},"
                       f"{bot['trades_per_hour']:.2f},{bot['trade_frequency']:.4f},"
                       f"{bot['roi']:.2f},{bot['pnl']:.2f},{bot['estimated_fees']:.2f}\n")
        
        print(f"\n✅ Detailed CSV exported: {csv_file}")
        
        json_file = self.results_dir / f"analysis_{datetime.now().strftime('%Y%m%d_%H%M%S')}.json"
        with open(json_file, 'w') as f:
            json.dump({
                'test_date': '2025-10-18',
                'duration_hours': 9.3,
                'bots': self.bots,
                'summary': {
                    'total_trades': total_trades,
                    'avg_trades': avg_trades,
                    'total_fees': total_fees,
                    'winner': winner['name']
                }
            }, f, indent=2)
        
        print(f"✅ JSON data exported: {json_file}")
        
        print("\n" + "="*110)
        print("🎉 Analysis Complete! Project Flash V3.5 - LFG! 🚀")
        print("="*110 + "\n")


if __name__ == "__main__":
    analyzer = BotAnalyzer("results/ultimate_20251018_004203")
    analyzer.load_all_bots()
    analyzer.generate_report()
