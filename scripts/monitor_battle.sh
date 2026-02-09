#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 🔍 ELITE BATTLE ROYALE MONITOR V4.2
# Real-time monitoring for production config battles
# ═══════════════════════════════════════════════════════════════════════════

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
BOLD='\033[1m'
NC='\033[0m'

clear
echo "═══════════════════════════════════════════════════════════════════════════"
echo -e "  ${BOLD}🔍 ELITE BATTLE ROYALE MONITOR V4.2${NC}"
echo "  $(date '+%Y-%m-%d %H:%M:%S')"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""

# Find latest battle royale or quick test results
RESULTS_DIR=$(ls -td results/{battle_royale,quick_parallel}_* 2>/dev/null | head -1)

if [ -z "$RESULTS_DIR" ]; then
    echo -e "${RED}❌ No battle results found${NC}"
    echo ""
    echo "Expected directories:"
    echo "  - results/battle_royale_*"
    echo "  - results/quick_parallel_*"
    exit 1
fi

echo -e "${CYAN}📁 Monitoring:${NC} $RESULTS_DIR"
echo ""

# Stats
RUNNING=0
STOPPED=0
COMPLETED=0
TOTAL=0

echo "═══════════════════════════════════════════════════════════════════════════"
echo -e "  ${BOLD}📊 BOT STATUS${NC}"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""

# Check each production config
for bot in balanced conservative aggressive ultra_aggressive multi_strategy; do
    PID_FILE="$RESULTS_DIR/$bot.pid"
    OUTPUT_FILE="$RESULTS_DIR/${bot}.txt"
    
    if [ ! -f "$PID_FILE" ]; then
        continue
    fi
    
    ((TOTAL++))
    PID=$(cat "$PID_FILE")
    
    # Choose color and emoji
    case $bot in
        balanced) color="$GREEN"; emoji="⚖️" ;;
        conservative) color="$BLUE"; emoji="🛡️" ;;
        aggressive) color="$YELLOW"; emoji="⚡" ;;
        ultra_aggressive) color="$RED"; emoji="🔥" ;;
        multi_strategy) color="$PURPLE"; emoji="🧠" ;;
        *) color="$WHITE"; emoji="🤖" ;;
    esac
    
    # Format bot name
    BOT_NAME=$(echo "$bot" | sed 's/_/-/g' | awk '{for(i=1;i<=NF;i++)sub(/./,toupper(substr($i,1,1)),$i)}1')
    
    # Check if running
    if ps -p $PID > /dev/null 2>&1; then
        echo -e "${color}${emoji} ${BOT_NAME}${NC} (PID ${PID}) - ${GREEN}${BOLD}RUNNING${NC}"
        ((RUNNING++))
        
        if [ -f "$OUTPUT_FILE" ]; then
            # Get latest cycle info
            LAST_CYCLE=$(grep -E "Cycle [0-9]+/" "$OUTPUT_FILE" | tail -1)
            if [ -n "$LAST_CYCLE" ]; then
                echo "   📊 $LAST_CYCLE"
            fi
            
            # Get performance stats
            TRADES=$(grep "Successful Trades:" "$OUTPUT_FILE" | tail -1 | awk '{print $3}')
            ROI=$(grep "ROI:" "$OUTPUT_FILE" | tail -1 | awk '{print $2}' | tr -d '%')
            ORDERS=$(grep "Open Orders:" "$OUTPUT_FILE" | tail -1 | awk '{print $3}')
            
            if [ -n "$TRADES" ]; then
                echo -e "   💰 Trades: ${GREEN}$TRADES${NC}"
            fi
            
            if [ -n "$ROI" ]; then
                if (( $(echo "$ROI > 0" | bc -l 2>/dev/null || echo 0) )); then
                    echo -e "   📈 ROI: ${GREEN}${ROI}%${NC}"
                elif (( $(echo "$ROI < 0" | bc -l 2>/dev/null || echo 0) )); then
                    echo -e "   📉 ROI: ${RED}${ROI}%${NC}"
                else
                    echo -e "   📊 ROI: ${YELLOW}${ROI}%${NC}"
                fi
            fi
            
            if [ -n "$ORDERS" ]; then
                echo -e "   🎯 Open Orders: ${CYAN}$ORDERS${NC}"
            fi
            
            # Check grid status
            if grep -q "Grid not initialized" "$OUTPUT_FILE" 2>/dev/null; then
                echo -e "   ${YELLOW}⏳ Initializing grid...${NC}"
            elif grep -q "Initial grid placed" "$OUTPUT_FILE" 2>/dev/null; then
                echo -e "   ${GREEN}✅ Grid active${NC}"
            fi
            
            # Check for errors
            ERRORS=$(grep -c "ERROR" "$OUTPUT_FILE" 2>/dev/null || echo "0")
            if [ "$ERRORS" -gt 0 ]; then
                echo -e "   ${RED}⚠️  $ERRORS errors detected${NC}"
            fi
        fi
    else
        echo -e "${color}${emoji} ${BOT_NAME}${NC} (PID ${PID}) - ${RED}${BOLD}STOPPED${NC}"
        ((STOPPED++))
        
        if [ -f "$OUTPUT_FILE" ]; then
            if grep -q "Trading session completed successfully\|Session complete" "$OUTPUT_FILE"; then
                echo -e "   ${GREEN}✅ Completed successfully${NC}"
                ((COMPLETED++))
                
                # Get final stats
                FINAL_ROI=$(grep "ROI:" "$OUTPUT_FILE" | tail -1 | awk '{print $2}')
                FINAL_TRADES=$(grep "Successful Trades:" "$OUTPUT_FILE" | tail -1 | awk '{print $3}')
                
                if [ -n "$FINAL_ROI" ]; then
                    echo -e "   📊 Final ROI: ${CYAN}$FINAL_ROI${NC}"
                fi
                if [ -n "$FINAL_TRADES" ]; then
                    echo -e "   💰 Total Trades: ${CYAN}$FINAL_TRADES${NC}"
                fi
            else
                echo -e "   ${RED}⚠️  Incomplete - check logs${NC}"
            fi
        fi
    fi
    
    echo ""
done

if [ $TOTAL -eq 0 ]; then
    echo -e "${YELLOW}⚠️  No bots found in this directory${NC}"
    echo ""
fi

# Summary
echo "═══════════════════════════════════════════════════════════════════════════"
echo -e "  ${BOLD}📈 BATTLE SUMMARY${NC}"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
echo -e "${GREEN}${BOLD}✅ Running:${NC}    $RUNNING / $TOTAL"
echo -e "${YELLOW}${BOLD}⏸️  Stopped:${NC}    $STOPPED / $TOTAL"
echo -e "${GREEN}${BOLD}🏁 Completed:${NC}  $COMPLETED / $TOTAL"
echo ""

# Progress bar
if [ $TOTAL -gt 0 ]; then
    PROGRESS=$((RUNNING * 100 / TOTAL))
    echo -ne "Progress: ["
    for i in {1..20}; do
        if [ $((i * 5)) -le $PROGRESS ]; then
            echo -ne "${GREEN}█${NC}"
        else
            echo -ne "░"
        fi
    done
    echo -e "] ${PROGRESS}%"
    echo ""
fi

# System info
echo "═══════════════════════════════════════════════════════════════════════════"
echo -e "  ${BOLD}💻 SYSTEM INFO${NC}"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""

# Disk usage
if [ -d "$RESULTS_DIR" ]; then
    SIZE=$(du -sh "$RESULTS_DIR" 2>/dev/null | awk '{print $1}')
    echo -e "📂 Results size: ${CYAN}$SIZE${NC}"
fi

# Log sizes
if [ -d "logs" ]; then
    LOG_SIZE=$(du -sh logs 2>/dev/null | awk '{print $1}')
    echo -e "📝 Logs size: ${CYAN}$LOG_SIZE${NC}"
fi

# CPU check
if command -v top >/dev/null 2>&1; then
    GRID_CPU=$(ps aux | grep solana-grid-bot | grep -v grep | awk '{sum+=$3} END {printf "%.1f", sum}')
    if [ -n "$GRID_CPU" ] && [ "$GRID_CPU" != "0.0" ]; then
        echo -e "⚡ Bot CPU usage: ${CYAN}${GRID_CPU}%${NC}"
    fi
fi

echo ""

# Quick actions
echo "═══════════════════════════════════════════════════════════════════════════"
echo -e "  ${BOLD}🛠️  QUICK ACTIONS${NC}"
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
echo -e "📝 ${CYAN}View all logs:${NC}      tail -f $RESULTS_DIR/*.txt"
echo -e "📊 ${CYAN}View specific bot:${NC}  tail -f $RESULTS_DIR/balanced.txt"
echo -e "🔄 ${CYAN}Refresh monitor:${NC}    ./scripts/monitor_battle.sh"
echo -e "📈 ${CYAN}Analyze results:${NC}    ./scripts/analyze_results.sh"

if [ $RUNNING -gt 0 ]; then
    ALL_PIDS=$(cat $RESULTS_DIR/*.pid 2>/dev/null | tr '\n' ' ')
    echo -e "🛑 ${CYAN}Stop all bots:${NC}      kill $ALL_PIDS"
fi

echo ""
echo "═══════════════════════════════════════════════════════════════════════════"
echo ""
