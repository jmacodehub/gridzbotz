#!/bin/bash
# Battle Royale V6 - Results Script
# Post-run analytics and ranking

set -e

echo "="
echo "🏆 BATTLE ROYALE V6 — FINAL RESULTS"
echo "="
echo "Analysis Time: $(date)"
echo ""

if [ ! -d "logs/battle-royale-v6" ]; then
    echo "❌ Error: logs/battle-royale-v6/ not found"
    echo "Run ./scripts/battle-royale-v6-launch.sh first"
    exit 1
fi

echo "📊 Performance Summary"
echo "="
echo ""

printf "%-20s | %-6s | %-6s | %-7s | %-9s\n" \
    "BOT" "FILLS" "REPOS" "ERRORS" "WARNINGS"
echo "----------------------------------------------------------------"

for LOG in logs/battle-royale-v6/*.log; do
    BOT=$(basename "$LOG" .log)
    
    # Count key events
    FILLS=$(grep -c "FILLTRACK" "$LOG" 2>/dev/null || echo "0")
    REPOS=$(grep -c "REPOSITION" "$LOG" 2>/dev/null || echo "0")
    ERRORS=$(grep -c "ERROR" "$LOG" 2>/dev/null || echo "0")
    WARNINGS=$(grep -c "WARN" "$LOG" 2>/dev/null || echo "0")
    
    printf "%-20s | %6s | %6s | %7s | %9s\n" \
        "$BOT" "$FILLS" "$REPOS" "$ERRORS" "$WARNINGS"
done

echo ""
echo "="
echo "🔍 Detailed Stats"
echo "="
echo ""

for LOG in logs/battle-royale-v6/*.log; do
    BOT=$(basename "$LOG" .log)
    
    echo "📦 $BOT"
    
    # Get runtime stats
    FIRST_CYCLE=$(grep "Cycle" "$LOG" | head -1 | grep -oP "Cycle \K[0-9]+" || echo "0")
    LAST_CYCLE=$(grep "Cycle" "$LOG" | tail -1 | grep -oP "Cycle \K[0-9]+" || echo "0")
    RUNTIME_MS=$((LAST_CYCLE - FIRST_CYCLE))
    RUNTIME_HOURS=$(echo "scale=2; $RUNTIME_MS / 3600000" | bc)
    
    # Get latest price and vol
    LATEST=$(tail -100 "$LOG" | grep "Cycle" | tail -1)
    PRICE=$(echo "$LATEST" | grep -oP "SOL \K[0-9.]+" || echo "N/A")
    VOL=$(echo "$LATEST" | grep -oP "Vol \K[0-9.]+" || echo "0.0000")
    
    echo "   Runtime: ${RUNTIME_HOURS}h"
    echo "   Final Price: \$$PRICE"
    echo "   Final Vol: ${VOL}%"
    
    # Get avg cycle time
    AVG_CYCLE=$(grep "Avg" "$LOG" | tail -1 || echo "N/A")
    echo "   Avg Cycle: $AVG_CYCLE"
    
    echo ""
done

echo "="
echo "✅ Analysis complete!"
echo "="
echo ""
echo "📄 Full logs available in: logs/battle-royale-v6/"
echo ""
