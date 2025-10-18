#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 📊 PROJECT FLASH V3.5 - RESULTS ANALYZER
# Comprehensive analysis of overnight test results
# ═══════════════════════════════════════════════════════════════════════════

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m'

clear

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  📊 PROJECT FLASH V3.5 - RESULTS ANALYZER"
echo "  $(date)"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Find latest results directory
RESULTS_DIR=$(ls -td results/overnight_* 2>/dev/null | head -1)

if [ -z "$RESULTS_DIR" ]; then
    echo -e "${RED}❌ No results found in results/overnight_*${NC}"
    echo ""
    echo "Available results:"
    ls -1d results/* 2>/dev/null || echo "  (none)"
    echo ""
    exit 1
fi

echo -e "${CYAN}Analyzing:${NC} $RESULTS_DIR"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# EXTRACT METRICS FROM EACH BOT
# ═══════════════════════════════════════════════════════════════════════════

declare -A bot_cycles
declare -A bot_trades
declare -A bot_repos
declare -A bot_blocks
declare -A bot_roi
declare -A bot_pnl
declare -A bot_winrate
declare -A bot_status

for output_file in "$RESULTS_DIR"/*.txt; do
    if [ -f "$output_file" ]; then
        bot_name=$(basename "$output_file" .txt)
        
        # Check if bot completed
        if grep -q "Trading session completed successfully" "$output_file" 2>/dev/null; then
            bot_status[$bot_name]="${GREEN}✅ COMPLETE${NC}"
        elif grep -q "Ctrl+C received" "$output_file" 2>/dev/null; then
            bot_status[$bot_name]="${YELLOW}⚠️  STOPPED${NC}"
        else
            # Check if still running
            pid_file="$RESULTS_DIR/${bot_name}.pid"
            if [ -f "$pid_file" ]; then
                pid=$(cat "$pid_file")
                if ps -p $pid > /dev/null 2>&1; then
                    bot_status[$bot_name]="${BLUE}🔄 RUNNING${NC}"
                else
                    bot_status[$bot_name]="${RED}❌ CRASHED${NC}"
                fi
            else
                bot_status[$bot_name]="${RED}❌ UNKNOWN${NC}"
            fi
        fi
        
        # Extract metrics from SESSION PERFORMANCE SUMMARY
        if grep -q "SESSION PERFORMANCE SUMMARY" "$output_file"; then
            # Total cycles
            cycles=$(grep "Total Cycles:" "$output_file" | tail -1 | awk '{print $3}')
            bot_cycles[$bot_name]="${cycles:-0}"
            
            # Successful trades
            trades=$(grep "Successful Trades:" "$output_file" | tail -1 | awk '{print $3}')
            bot_trades[$bot_name]="${trades:-0}"
            
            # Grid repositions
            repos=$(grep "Grid Repositions:" "$output_file" | tail -1 | awk '{print $3}')
            bot_repos[$bot_name]="${repos:-0}"
            
            # Regime blocks
            blocks=$(grep "Regime Blocks:" "$output_file" | tail -1 | awk '{print $3}')
            bot_blocks[$bot_name]="${blocks:-0}"
            
            # ROI
            roi=$(grep "ROI:" "$output_file" | tail -1 | awk '{print $2}' | tr -d '%')
            bot_roi[$bot_name]="${roi:-0.00}"
            
            # P&L
            pnl=$(grep "P&L:" "$output_file" | tail -1 | awk '{print $2}' | tr -d '$')
            bot_pnl[$bot_name]="${pnl:-0.00}"
            
            # Win Rate
            winrate=$(grep "Win Rate:" "$output_file" | tail -1 | awk '{print $3}' | tr -d '%')
            bot_winrate[$bot_name]="${winrate:-0.00}"
        else
            # No complete summary yet - try to get partial data
            bot_cycles[$bot_name]="N/A"
            bot_trades[$bot_name]="N/A"
            bot_repos[$bot_name]="N/A"
            bot_blocks[$bot_name]="N/A"
            bot_roi[$bot_name]="0.00"
            bot_pnl[$bot_name]="0.00"
            bot_winrate[$bot_name]="0.00"
        fi
    fi
done

# ═══════════════════════════════════════════════════════════════════════════
# DISPLAY SUMMARY TABLE
# ═══════════════════════════════════════════════════════════════════════════

echo "════════════════════════════════════════════════════════════════════════"
echo "  📈 PERFORMANCE COMPARISON"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

printf "%-25s %-12s %-10s %-8s %-8s %-10s\n" "Bot" "Status" "Cycles" "Trades" "Repos" "ROI %"
echo "────────────────────────────────────────────────────────────────────────"

for bot in "${!bot_status[@]}" | sort; do
    printf "%-25s " "$bot"
    printf "%-12b " "${bot_status[$bot]}"
    printf "%-10s " "${bot_cycles[$bot]}"
    printf "%-8s " "${bot_trades[$bot]}"
    printf "%-8s " "${bot_repos[$bot]}"
    
    # Color code ROI
    roi="${bot_roi[$bot]}"
    if (( $(echo "$roi > 5.0" | bc -l 2>/dev/null || echo 0) )); then
        printf "${GREEN}%-10s${NC}\n" "$roi"
    elif (( $(echo "$roi > 0.0" | bc -l 2>/dev/null || echo 0) )); then
        printf "${YELLOW}%-10s${NC}\n" "$roi"
    else
        printf "${RED}%-10s${NC}\n" "$roi"
    fi
done

echo ""

# ═══════════════════════════════════════════════════════════════════════════
# RANKINGS
# ═══════════════════════════════════════════════════════════════════════════

echo "════════════════════════════════════════════════════════════════════════"
echo "  🏆 RANKINGS"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Best ROI
echo -e "${YELLOW}🥇 Best ROI:${NC}"
for bot in "${!bot_roi[@]}"; do
    echo "${bot_roi[$bot]} $bot"
done | sort -rn | head -3 | nl
echo ""

# Most Trades
echo -e "${YELLOW}📊 Most Active (Trades):${NC}"
for bot in "${!bot_trades[@]}"; do
    trades="${bot_trades[$bot]}"
    if [ "$trades" != "N/A" ]; then
        echo "$trades $bot"
    fi
done | sort -rn | head -3 | nl
echo ""

# Most Repositions
echo -e "${YELLOW}🔄 Most Repositions:${NC}"
for bot in "${!bot_repos[@]}"; do
    repos="${bot_repos[$bot]}"
    if [ "$repos" != "N/A" ]; then
        echo "$repos $bot"
    fi
done | sort -rn | head -3 | nl
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# DETAILED STATS
# ═══════════════════════════════════════════════════════════════════════════

echo "════════════════════════════════════════════════════════════════════════"
echo "  📋 DETAILED STATISTICS"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

for bot in $(echo "${!bot_status[@]}" | tr ' ' '\n' | sort); do
    echo -e "${CYAN}━━━ ${bot} ━━━${NC}"
    printf "  Status:          %b\n" "${bot_status[$bot]}"
    printf "  Cycles:          %s\n" "${bot_cycles[$bot]}"
    printf "  Trades:          %s\n" "${bot_trades[$bot]}"
    printf "  Repositions:     %s\n" "${bot_repos[$bot]}"
    printf "  Regime Blocks:   %s\n" "${bot_blocks[$bot]}"
    printf "  ROI:             %s%%\n" "${bot_roi[$bot]}"
    printf "  P&L:             \$%s\n" "${bot_pnl[$bot]}"
    printf "  Win Rate:        %s%%\n" "${bot_winrate[$bot]}"
    echo ""
done

# ═══════════════════════════════════════════════════════════════════════════
# RECOMMENDATIONS
# ═══════════════════════════════════════════════════════════════════════════

echo "════════════════════════════════════════════════════════════════════════"
echo "  💡 RECOMMENDATIONS"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Find best performer
best_bot=""
best_roi=0
for bot in "${!bot_roi[@]}"; do
    roi="${bot_roi[$bot]}"
    if (( $(echo "$roi > $best_roi" | bc -l 2>/dev/null || echo 0) )); then
        best_roi=$roi
        best_bot=$bot
    fi
done

if [ -n "$best_bot" ]; then
    echo -e "${GREEN}✅ Best Performer:${NC} $best_bot (${best_roi}% ROI)"
    echo ""
    echo "   Consider using this config for production trading!"
    echo ""
fi

# Check for issues
for bot in "${!bot_status[@]}"; do
    if [[ "${bot_status[$bot]}" == *"CRASHED"* ]]; then
        echo -e "${RED}⚠️  Warning:${NC} $bot crashed - check logs:"
        echo "   tail -100 $RESULTS_DIR/${bot}.txt"
        echo ""
    fi
done

# ═══════════════════════════════════════════════════════════════════════════
# EXPORT TO CSV
# ═══════════════════════════════════════════════════════════════════════════

CSV_FILE="$RESULTS_DIR/analysis_$(date +%Y%m%d_%H%M%S).csv"

echo "Bot,Status,Cycles,Trades,Repositions,Blocks,ROI,PnL,WinRate" > "$CSV_FILE"
for bot in $(echo "${!bot_status[@]}" | tr ' ' '\n' | sort); do
    status=$(echo "${bot_status[$bot]}" | sed 's/\x1b\[[0-9;]*m//g')
    echo "$bot,$status,${bot_cycles[$bot]},${bot_trades[$bot]},${bot_repos[$bot]},${bot_blocks[$bot]},${bot_roi[$bot]},${bot_pnl[$bot]},${bot_winrate[$bot]}" >> "$CSV_FILE"
done

echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo -e "${GREEN}✅ Analysis complete!${NC}"
echo -e "${CYAN}📁 Results exported to:${NC} $CSV_FILE"
echo ""
