#!/bin/bash
# Battle Royale V6 - Real-Time Monitor
# Live leaderboard with 10s refresh

watch -n 10 '
echo "=================================================================="
echo "🏆 BATTLE ROYALE V6 — LIVE LEADERBOARD $(date +%H:%M:%S)"
echo "=================================================================="
echo ""

printf "%-20s | %-9s | %-10s | %-6s | %-6s | %-10s\n" \
    "BOT" "PRICE" "VOL" "FILLS" "REPOS" "STATUS"
echo "------------------------------------------------------------------"

for LOG in logs/battle-royale-v6/*.log; do
    BOT=$(basename "$LOG" .log)
    
    # Get latest stats from last 100 lines
    LATEST=$(tail -100 "$LOG" | grep "Cycle" | tail -1)
    
    if [ -n "$LATEST" ]; then
        PRICE=$(echo "$LATEST" | grep -oP "SOL \K[0-9.]+" || echo "N/A")
        VOL=$(echo "$LATEST" | grep -oP "Vol \K[0-9.]+" || echo "0.0000")
        FILLS=$(echo "$LATEST" | grep -oP "Fills \K[0-9]+" || echo "0")
        REPOS=$(echo "$LATEST" | grep -oP "Repos \K[0-9]+" || echo "0")
        STATUS=$(echo "$LATEST" | grep -oP "(Stable|Paused|Error)" || echo "Unknown")
        
        printf "%-20s | $%-8s | %9s%% | %6s | %6s | %-10s\n" \
            "$BOT" "$PRICE" "$VOL" "$FILLS" "$REPOS" "$STATUS"
    else
        printf "%-20s | %-9s | %-10s | %-6s | %-6s | %-10s\n" \
            "$BOT" "STARTING" "-" "-" "-" "Initializing"
    fi
done

echo ""
echo "=================================================================="
echo "Press Ctrl+C to exit"
'
