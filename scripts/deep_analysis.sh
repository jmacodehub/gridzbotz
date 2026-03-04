#!/bin/bash
echo "════════════════════════════════════════════════════════════"
echo "🏆 GRIDZBOTZ CHAMPIONSHIP - DEEP ANALYSIS"
echo "════════════════════════════════════════════════════════════"
echo ""

for log in logs/battle_20h_20260208_2349/*.log; do
    name=$(basename $log .log)
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📊 $(echo $name | tr '[:lower:]' '[:upper:]')"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Performance metrics
    echo "⚡ PERFORMANCE:"
    grep "Runtime:" "$log" | tail -1
    grep "Average:" "$log" | grep -A1 "CYCLE PERFORMANCE" | tail -1
    grep "Throughput:" "$log" | tail -1
    grep "Slow Cycles:" "$log" | tail -1
    
    echo ""
    echo "📈 TRADING:"
    grep "Grid Repositions:" "$log" | tail -1
    grep "Price Updates:" "$log" | tail -1
    grep "Regime Blocks:" "$log" | tail -1
    grep "Win Rate:" "$log" | tail -1
    grep "Total Fees:" "$log" | tail -1
    
    echo ""
    echo "💰 P&L:"
    grep "Initial USDC:" "$log" | tail -1
    grep "Initial SOL:" "$log" | tail -1
    grep "Final USDC:" "$log" | tail -1
    grep "Final SOL:" "$log" | tail -1
    grep "Net P&L:" "$log" | tail -1
    grep "ROI:" "$log" | tail -1
    
    echo ""
    echo "📊 PRICE DATA:"
    grep "Starting Price:" "$log" | tail -1
    grep "Final Price:" "$log" | tail -1 || grep "Current Price:" "$log" | tail -1
    grep "Price Range:" "$log" | tail -1
    
    echo ""
    echo "❌ RELIABILITY:"
    grep "Total Errors:" "$log" | tail -1
    grep "Failed Fetches:" "$log" | tail -1
    grep "Success Rate:" "$log" | tail -2 | head -1
    
    echo ""
done

echo "════════════════════════════════════════════════════════════"
