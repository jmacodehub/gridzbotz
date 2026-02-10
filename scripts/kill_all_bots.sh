#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🛑 KILL ALL BOTS - Emergency Stop Script
# ═══════════════════════════════════════════════════════════════════════════

set +e  # Don't exit on error

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🛑 KILLING ALL SOLANA-GRID-BOT PROCESSES"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

# Find all solana-grid-bot processes
PROCESSES=$(ps aux | grep '[s]olana-grid-bot' | awk '{print $2}')

if [ -z "$PROCESSES" ]; then
    echo "${GREEN}✅ No running bots found!${NC}"
    echo ""
    exit 0
fi

echo "${YELLOW}⚠️  Found running bots:${NC}"
echo ""
ps aux | grep '[s]olana-grid-bot' | awk '{print "   PID " $2 ": " $11 " " $12 " " $13 " " $14}'
echo ""

# Kill each process
for PID in $PROCESSES; do
    echo "${RED}🔫 Killing PID $PID...${NC}"
    kill $PID 2>/dev/null
    
    # Wait a moment
    sleep 1
    
    # Check if still running
    if ps -p $PID > /dev/null 2>&1; then
        echo "${RED}   ⚠️  Process still running, using SIGKILL...${NC}"
        kill -9 $PID 2>/dev/null
    fi
done

echo ""
sleep 1

# Verify all are dead
REMAINING=$(ps aux | grep '[s]olana-grid-bot' | awk '{print $2}')

if [ -z "$REMAINING" ]; then
    echo "${GREEN}✅ All bots successfully killed!${NC}"
    echo ""
    exit 0
else
    echo "${RED}❌ Some processes still running:${NC}"
    ps aux | grep '[s]olana-grid-bot'
    echo ""
    echo "${YELLOW}Try manually: kill -9 $REMAINING${NC}"
    echo ""
    exit 1
fi
