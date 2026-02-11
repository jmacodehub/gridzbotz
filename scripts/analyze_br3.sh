#!/bin/bash

# Battle Royale 3 Results Analyzer
# Usage: ./scripts/analyze_br3.sh

LOGS_DIR="logs/battle_royale_3"
SESSION="20260211_011916"

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "═══════════════════════════════════════════════════════════════════════════════="
echo "🏆 BATTLE ROYALE #3 RESULTS ANALYSIS"
echo "═══════════════════════════════════════════════════════════════════════════════="
echo ""
echo "Session: $SESSION"
echo "Duration: 10 hours"
echo ""

# Function to analyze a single bot
analyze_bot() {
    local NAME=$1
    local FILE=$2
    local COLOR=$3
    
    echo -e "${COLOR}────────────────────────────────────────────────────────────────${NC}"
    echo -e "${COLOR}🤖 ${NAME}${NC}"
    echo -e "${COLOR}────────────────────────────────────────────────────────────────${NC}"
    
    if [ ! -f "$FILE" ]; then
        echo -e "${RED}❌ Log file not found: $FILE${NC}"
        return
    fi
    
    # Extract final stats (last 100 lines usually have summary)
    echo ""
    echo "📊 Final Statistics:"
    echo ""
    
    # Try to find session completion
    if grep -q "session completed successfully" "$FILE"; then
        echo -e "   Status: ${GREEN}✅ Completed Successfully${NC}"
    elif grep -q "Trading session completed" "$FILE"; then
        echo -e "   Status: ${GREEN}✅ Completed${NC}"
    else
        echo -e "   Status: ${YELLOW}⚠️  May have crashed - check manually${NC}"
    fi
    
    # Extract cycle count
    CYCLES=$(tail -100 "$FILE" | grep -o "Cycle [0-9]*" | tail -1 | awk '{print $2}')
    if [ -n "$CYCLES" ]; then
        echo "   Total Cycles: $(printf "%'d" $CYCLES)"
    fi
    
    # Extract final price
    PRICE=$(tail -50 "$FILE" | grep -o "SOL [0-9]*\.[0-9]*" | tail -1 | awk '{print $2}')
    if [ -n "$PRICE" ]; then
        echo "   Final Price: \$${PRICE}"
    fi
    
    # Extract total sells from Enhanced Metrics
    SELLS=$(tail -100 "$FILE" | grep "Total Sells:" | tail -1 | awk '{print $3}')
    if [ -n "$SELLS" ]; then
        echo "   Total Sells: $SELLS"
    fi
    
    # Extract trades per hour
    TRADES_HR=$(tail -100 "$FILE" | grep "Trades/Hour:" | tail -1 | awk '{print $2}')
    if [ -n "$TRADES_HR" ]; then
        echo "   Trades/Hour: $TRADES_HR"
    fi
    
    # Extract ROI from Portfolio section
    ROI=$(tail -100 "$FILE" | grep "ROI:" | grep -v "per" | tail -1 | awk '{print $2}' | tr -d '%')
    if [ -n "$ROI" ]; then
        echo "   ROI: ${ROI}%"
    fi
    
    # Extract P&L
    PNL=$(tail -100 "$FILE" | grep "P&L:" | tail -1 | awk '{print $2}' | tr -d '$')
    if [ -n "$PNL" ]; then
        echo "   P&L: \$${PNL}"
    fi
    
    # Extract Max Drawdown
    MAX_DD=$(tail -100 "$FILE" | grep "Max Drawdown:" | tail -1 | awk '{print $3}' | tr -d '%')
    if [ -n "$MAX_DD" ]; then
        echo "   Max Drawdown: ${MAX_DD}%"
    fi
    
    # Extract Grid Efficiency
    GRID_EFF=$(tail -100 "$FILE" | grep "Grid Efficiency:" | tail -1 | awk '{print $3}' | tr -d '%')
    if [ -n "$GRID_EFF" ]; then
        echo "   Grid Efficiency: ${GRID_EFF}%"
    fi
    
    # Extract Grid Levels Used
    GRID_LEVELS=$(tail -100 "$FILE" | grep "Grid Levels Used:" | tail -1 | awk '{print $4}')
    if [ -n "$GRID_LEVELS" ]; then
        echo "   Grid Levels: $GRID_LEVELS"
    fi
    
    # Count actual errors (from SESSION PERFORMANCE)
    ERRORS=$(tail -50 "$FILE" | grep "Total Errors:" | tail -1 | awk '{print $3}')
    if [ -n "$ERRORS" ]; then
        if [ "$ERRORS" = "0" ]; then
            echo -e "   Errors: ${GREEN}0 ✅${NC}"
        else
            echo -e "   Errors: ${YELLOW}${ERRORS} ⚠️${NC}"
        fi
    fi
    
    echo ""
}

# Analyze each bot
analyze_bot "🛡️  Conservative v4.0" \
    "$LOGS_DIR/conservative_v4_${SESSION}.log" \
    "$BLUE"

analyze_bot "🧠 Multi-Strategy v4.0 'Conservative AI'" \
    "$LOGS_DIR/multi_strategy_v4_${SESSION}.log" \
    "$GREEN"

analyze_bot "⚖️  Balanced v4.0" \
    "$LOGS_DIR/balanced_v4_${SESSION}.log" \
    "$YELLOW"

echo "═══════════════════════════════════════════════════════════════════════════════="
echo "📋 SUMMARY"
echo "═══════════════════════════════════════════════════════════════════════════════="
echo ""
echo "🏆 Winner: Balanced v4.0 (0.01% ROI, +\$0.37 P&L, 24 sells)"
echo "🥈 Runner-up: Conservative v4.0 (0.00% ROI, +\$0.06 P&L, 17 sells)"
echo "🥉 Third: Multi-Strategy v4.0 (0.00% ROI, +\$0.05 P&L, 16 sells)"
echo ""
echo "Full analysis: docs/analysis/BATTLE_ROYALE_3_RESULTS.md"
echo ""
echo "═══════════════════════════════════════════════════════════════════════════════="
