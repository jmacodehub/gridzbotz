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

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🏆 BATTLE ROYALE #3 RESULTS ANALYSIS"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "Session: $SESSION"
echo "Duration: 10 hours"
echo ""

# Function to analyze a single bot
analyze_bot() {
    local NAME=$1
    local FILE=$2
    local COLOR=$3
    
    echo -e "${COLOR}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${COLOR}🤖 ${NAME}${NC}"
    echo -e "${COLOR}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    
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
    
    # Count trades
    TRADES=$(grep -c "Trade executed\|Executed trade\|TRADE:" "$FILE" 2>/dev/null || echo "0")
    echo "   Total Trades: $TRADES"
    
    # Calculate trades per hour (10-hour run)
    if [ "$TRADES" != "0" ]; then
        TRADES_PER_HOUR=$(echo "scale=2; $TRADES / 10" | bc)
        echo "   Trades/Hour: $TRADES_PER_HOUR"
    fi
    
    # Extract ROI if present
    ROI=$(tail -100 "$FILE" | grep -o "ROI[: ]*[-0-9.]*%" | tail -1 | grep -o "[-0-9.]*")
    if [ -n "$ROI" ]; then
        echo "   ROI: ${ROI}%"
    else
        # Try alternative patterns
        ROI=$(tail -100 "$FILE" | grep -i "return\|profit" | grep -o "[-0-9.]*%" | tail -1 | tr -d '%')
        if [ -n "$ROI" ]; then
            echo "   ROI: ${ROI}%"
        fi
    fi
    
    # Count errors (exclude false positives)
    ERRORS=$(grep -i "ERROR" "$FILE" | grep -v "0 errors" | grep -v "ERRORS: 0" | wc -l | tr -d ' ')
    if [ "$ERRORS" = "0" ]; then
        echo -e "   Errors: ${GREEN}0 ✅${NC}"
    else
        echo -e "   Errors: ${YELLOW}${ERRORS} ⚠️${NC}"
    fi
    
    # Extract grid info
    GRID_LEVELS=$(tail -100 "$FILE" | grep -o "Grid Levels [0-9]*" | tail -1 | awk '{print $3}')
    if [ -n "$GRID_LEVELS" ]; then
        echo "   Grid Levels: $GRID_LEVELS"
    fi
    
    # Extract repositions
    REPOS=$(tail -100 "$FILE" | grep -o "Repos [0-9]*" | tail -1 | awk '{print $2}')
    if [ -n "$REPOS" ]; then
        echo "   Repositions: $REPOS"
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

echo "════════════════════════════════════════════════════════════════════════════════"
echo "📋 COMPARISON TABLE"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "Run this to generate CSV:"
echo "  ./scripts/analyze_br3.sh --csv > battle_royale_3_results.csv"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"

