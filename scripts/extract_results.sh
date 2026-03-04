#!/bin/bash
echo "════════════════════════════════════════════════════════════"
echo "🏆 GRIDZBOTZ 20-HOUR BATTLE ROYALE - FINAL RESULTS"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "📊 Session: battle_20h_20260208_2349"
echo "⏰ Started: Feb 08, 2026 @ 11:49 PM"
echo "🏁 Ended: Feb 09, 2026 @ ~7:20 PM"
echo ""
echo "════════════════════════════════════════════════════════════"

for log in logs/battle_20h_20260208_2349/*.log; do
    name=$(basename $log .log)
    echo ""
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📈 BOT: $(echo $name | tr '[:lower:]' '[:upper:]')"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Get final cycle
    cycles=$(grep "Total Cycles:" "$log" | tail -1 | awk '{print $3}')
    if [ -z "$cycles" ]; then
        cycles=$(grep "Cycle " "$log" | tail -1 | grep -oE "[0-9]+/[0-9]+" | head -1)
        echo "⚠️  Status: STILL RUNNING"
        echo "📊 Current: $cycles"
    else
        echo "✅ Status: COMPLETE"
        echo "📊 Cycles: $cycles"
        
        # Extract key metrics
        runtime=$(grep "Runtime:" "$log" | tail -1 | awk '{print $2}')
        success=$(grep "Successful:" "$log" | tail -1 | awk '{print $2, $3}')
        avg_time=$(grep "Average:" "$log" | grep -A1 "CYCLE PERFORMANCE" | tail -1 | awk '{print $2}')
        throughput=$(grep "Throughput:" "$log" | tail -1 | awk '{print $2, $3}')
        repos=$(grep "Grid Repositions:" "$log" | tail -1 | awk '{print $3}')
        errors=$(grep "Total Errors:" "$log" | tail -1 | awk '{print $3}')
        
        echo "⏱️  Runtime: $runtime"
        echo "✓  Success: $success"
        echo "⚡ Avg Time: $avg_time"
        echo "🔥 Throughput: $throughput"
        echo "🔄 Repositions: $repos"
        echo "❌ Errors: $errors"
    fi
done

echo ""
echo "════════════════════════════════════════════════════════════"
echo "🏆 CHAMPION SELECTION CRITERIA:"
echo "════════════════════════════════════════════════════════════"
echo "1. ✅ Completion (720,000 cycles)"
echo "2. ⚡ Lowest avg cycle time"
echo "3. 🔥 Highest throughput"
echo "4. ❌ Zero errors"
echo "5. 🔄 Fewest unnecessary repositions"
echo ""
