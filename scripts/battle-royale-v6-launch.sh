#!/bin/bash
# Battle Royale V6 - Launch Script
# Starts 7 parallel bots with 5s stagger to prevent RPC overload

set -e

echo "="
echo "🔥 BATTLE ROYALE V6 — 18-HOUR MARATHON 🔥"
echo "="
echo "Started: $(date)"
echo ""

# Create logs directory
mkdir -p logs/battle-royale-v6

# Array of configs
CONFIGS=(
    "night-owl"
    "adaptive-hunter"
    "scalper-pro"
    "no-gate-yolo"
    "momentum-rider"
    "mean-reversion"
    "max-levels-titan"
)

echo "🚀 Launching 7 bots..."
echo ""

# Launch each bot in background
for CONFIG in "${CONFIGS[@]}"; do
    echo "📦 Starting: $CONFIG"
    
    cargo run --release -- \
        --config config/battle-royale-v6/${CONFIG}.toml \
        > logs/battle-royale-v6/${CONFIG}.log 2>&1 &
    
    PID=$!
    echo "   PID: $PID"
    echo "   Log: logs/battle-royale-v6/${CONFIG}.log"
    echo $PID > logs/battle-royale-v6/${CONFIG}.pid
    
    # Wait 5s between launches to avoid RPC spam
    if [ "$CONFIG" != "max-levels-titan" ]; then
        echo "   Waiting 5s before next launch..."
        sleep 5
    fi
    echo ""
done

echo ""
echo "="
echo "✅ All 7 bots launched!"
echo "="
echo ""
echo "📊 Monitor with:"
echo "   tail -f logs/battle-royale-v6/*.log"
echo "   ./scripts/battle-royale-v6-monitor.sh"
echo ""
echo "🛑 Stop all with:"
echo "   ./scripts/battle-royale-v6-stop.sh"
echo ""
echo "📈 View results after 18h:"
echo "   ./scripts/battle-royale-v6-results.sh"
echo ""
