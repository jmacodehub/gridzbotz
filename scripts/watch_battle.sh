#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 👀 LIVE BATTLE WATCHER - Auto-refreshing monitor
# Press Ctrl+C to exit
# ═══════════════════════════════════════════════════════════════════════════

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# Refresh interval (seconds)
REFRESH=${1:-30}

echo -e "${CYAN}👀 Starting live battle monitor (refresh every ${REFRESH}s)...${NC}"
echo -e "${YELLOW}⏸️  Press Ctrl+C to exit${NC}"
sleep 2

while true; do
    ./scripts/monitor_battle.sh
    
    echo ""
    echo -e "${CYAN}🔄 Refreshing in ${REFRESH}s... (Ctrl+C to exit)${NC}"
    sleep $REFRESH
done
