#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 📊 PROJECT FLASH V3.5 - SUITE MONITOR (macOS Compatible)
# ═══════════════════════════════════════════════════════════════════════════

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m'

clear
echo "════════════════════════════════════════════════════════════════════════"
echo "  📊 PROJECT FLASH V3.5 - SUITE MONITOR"
echo "  $(date)"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Find latest results dir
RESULTS_DIR=$(ls -td results/{overnight,ultimate}_* 2>/dev/null | head -1)

if [ -z "$RESULTS_DIR" ]; then
    echo -e "${RED}❌ No test results found${NC}"
    exit 1
fi

echo -e "${CYAN}📁 Monitoring:${NC} $RESULTS_DIR"
echo ""

# Check for bots
RUNNING=0
STOPPED=0
COMPLETED=0

# List of possible bots
for bot in conservative balanced aggressive super_aggressive ultra_aggressive testing multi_strategy; do
    PID_FILE="$RESULTS_DIR/$bot.pid"
    OUTPUT_FILE="$RESULTS_DIR/${bot}.txt"
    
    if [ ! -f "$PID_FILE" ]; then
        continue
    fi
    
    PID=$(cat "$PID_FILE")
    
    # Choose color
    case $bot in
        conservative) color="$BLUE" ;;
        balanced) color="$GREEN" ;;
        aggressive|super_aggressive|ultra_aggressive) color="$YELLOW" ;;
        testing) color="$PURPLE" ;;
        multi_strategy) color="$CYAN" ;;
        *) color="$WHITE" ;;
    esac
    
    # Format bot name
    BOT_NAME=$(echo "$bot" | sed 's/_/ /g' | awk '{for(i=1;i<=NF;i++)sub(/./,toupper(substr($i,1,1)),$i)}1')
    
    # Check if running
    if ps -p $PID > /dev/null 2>&1; then
        echo -e "${color}✅ ${BOT_NAME}${NC} (PID $PID) - ${GREEN}RUNNING${NC}"
        ((RUNNING++))
        
        if [ -f "$OUTPUT_FILE" ]; then
            # Get last cycle
            LAST_LINE=$(tail -20 "$OUTPUT_FILE" | grep "Cycle" | tail -1)
            if [ -n "$LAST_LINE" ]; then
                echo "   $LAST_LINE"
            fi
            
            # Check grid init
            if grep -q "Placed.*orders" "$OUTPUT_FILE" 2>/dev/null; then
                ORDERS=$(grep "Open Orders:" "$OUTPUT_FILE" | tail -1 | awk '{print $3}')
                echo -e "   ${GREEN}✅ Grid active: ${ORDERS} orders${NC}"
            fi
            
            # Count trades
            TRADES=$(grep -c "orders filled" "$OUTPUT_FILE" 2>/dev/null || echo "0")
            if [ "$TRADES" -gt 0 ]; then
                echo -e "   ${GREEN}💰 $TRADES trades executed${NC}"
            fi
        fi
    else
        echo -e "${color}❌ ${BOT_NAME}${NC} (PID $PID) - ${RED}STOPPED${NC}"
        ((STOPPED++))
        
        if [ -f "$OUTPUT_FILE" ]; then
            if grep -q "Trading session completed successfully" "$OUTPUT_FILE"; then
                echo -e "   ${GREEN}✅ Completed${NC}"
                ((COMPLETED++))
            else
                echo -e "   ${RED}⚠️  Check logs${NC}"
            fi
        fi
    fi
    
    echo ""
done

# Summary
echo "════════════════════════════════════════════════════════════════════════"
echo "  📈 SUMMARY"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo -e "Running:    ${GREEN}$RUNNING${NC}"
echo -e "Stopped:    ${YELLOW}$STOPPED${NC}"
echo -e "Completed:  ${GREEN}$COMPLETED${NC}"
echo ""

# Actions
echo "════════════════════════════════════════════════════════════════════════"
echo "  🛠️  QUICK ACTIONS"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo "📝 Logs:     tail -f $RESULTS_DIR/*.txt"
echo "🔄 Refresh:  ./scripts/monitor_suite.sh"

if [ $RUNNING -gt 0 ]; then
    ALL_PIDS=$(cat $RESULTS_DIR/*.pid 2>/dev/null | tr '\n' ' ')
    echo "🛑 Stop:     kill $ALL_PIDS"
fi

echo ""
