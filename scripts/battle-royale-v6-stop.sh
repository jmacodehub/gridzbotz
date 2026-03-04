#!/bin/bash
# Battle Royale V6 - Stop Script
# Gracefully stops all running bots

set -e

echo "🛑 Stopping Battle Royale V6..."
echo ""

STOPPED=0
ALREADY_STOPPED=0

for PID_FILE in logs/battle-royale-v6/*.pid; do
    if [ -f "$PID_FILE" ]; then
        PID=$(cat "$PID_FILE")
        CONFIG=$(basename "$PID_FILE" .pid)
        
        if kill -0 $PID 2>/dev/null; then
            echo "   Stopping: $CONFIG (PID: $PID)"
            kill $PID
            STOPPED=$((STOPPED + 1))
        else
            echo "   Already stopped: $CONFIG"
            ALREADY_STOPPED=$((ALREADY_STOPPED + 1))
        fi
        
        rm "$PID_FILE"
    fi
done

echo ""
echo "="
echo "✅ Battle Royale V6 stopped!"
echo "="
echo "   Stopped: $STOPPED bots"
echo "   Already stopped: $ALREADY_STOPPED bots"
echo ""
echo "📈 View results:"
echo "   ./scripts/battle-royale-v6-results.sh"
echo ""
