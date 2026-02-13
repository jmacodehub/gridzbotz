#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🔥🏆 BATTLE ROYALE V2.5 - 20-HOUR MARATHON SHOWDOWN! 🏆🔥
# 
# Version: 2.5 Enhanced (Parallel Default)
# Duration: 20 hours (EPIC MARATHON!)
# Configs: 3 V2.5 optimized strategies running SIMULTANEOUSLY
# Goal: Crown the champion & deploy to mainnet!
# 
# V2.5 Features Tested:
# ✅ Market Regime Gate (auto-pause)
# ✅ Smart Fee Filtering (dynamic limits)
# ✅ Order Lifecycle (auto-refresh)
# ✅ Dynamic Grid Spacing (volatility-based)
# 
# February 13, 2026 - 20-Hour Parallel Marathon! 🚀⏰
# ═══════════════════════════════════════════════════════════════════════════

set -eo pipefail  # Exit on error, pipe failures

# ───────────────────────────────────────────────────────────────────────────
# 🎨 COLORS & STYLING
# ───────────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
BOLD='\033[1m'
NC='\033[0m'

# ───────────────────────────────────────────────────────────────────────────
# ⚙️ CONFIGURATION - PARALLEL & 20 HOURS BY DEFAULT!
# ───────────────────────────────────────────────────────────────────────────
DURATION_HOURS=${1:-20}         # 🔥 DEFAULT: 20 HOURS!
PARALLEL=${PARALLEL:-true}      # 🔥 DEFAULT: PARALLEL MODE!
SESSION_ID=$(date +"%Y%m%d_%H%M%S")
LOG_DIR="logs/battle_royale_${SESSION_ID}"
RESULTS_DIR="results/battle_royale_${SESSION_ID}"

# Config paths (V2.5)
CONFIG_DIR="config/optimized"
CONFIG_MULTI="${CONFIG_DIR}/multi-v5-ai.toml"
CONFIG_BALANCED="${CONFIG_DIR}/balanced-v4.1.toml"
CONFIG_CONSERVATIVE="${CONFIG_DIR}/conservative-v4.1.toml"

# PIDs for parallel tracking
declare -A pids

# ───────────────────────────────────────────────────────────────────────────
# 🛠️ HELPER FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

print_header() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${WHITE}$1${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo ""
}

print_section() {
    echo ""
    echo -e "${PURPLE}───────────────────────────────────────────────────────────────────────────${NC}"
    echo -e "${BOLD}$1${NC}"
    echo -e "${PURPLE}───────────────────────────────────────────────────────────────────────────${NC}"
    echo ""
}

timestamp() {
    date +"%Y-%m-%d %H:%M:%S"
}

cleanup() {
    echo ""
    echo -e "${YELLOW}🛑 Caught interrupt signal - cleaning up...${NC}"
    
    # Kill all bot processes
    for pid in "${pids[@]}"; do
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${YELLOW}Stopping PID $pid...${NC}"
            kill -TERM $pid 2>/dev/null || true
        fi
    done
    
    # Wait a bit for graceful shutdown
    sleep 2
    
    # Force kill if still running
    for pid in "${pids[@]}"; do
        if ps -p $pid > /dev/null 2>&1; then
            echo -e "${RED}Force killing PID $pid...${NC}"
            kill -KILL $pid 2>/dev/null || true
        fi
    done
    
    echo -e "${GREEN}✅ Cleanup complete${NC}"
    exit 130
}

# Trap Ctrl+C and cleanup
trap cleanup SIGINT SIGTERM

# ───────────────────────────────────────────────────────────────────────────
# 🚀 BANNER
# ───────────────────────────────────────────────────────────────────────────

clear
print_header "🔥🏆 BATTLE ROYALE V2.5 - 20-HOUR MARATHON! 🏆🔥"

echo -e "${CYAN}Version:${NC} 2.5 Enhanced"
echo -e "${CYAN}Duration:${NC} ${BOLD}${YELLOW}${DURATION_HOURS} HOURS${NC} ⏰"
echo -e "${CYAN}Session ID:${NC} ${SESSION_ID}"
echo -e "${CYAN}Mode:${NC} ${BOLD}${GREEN}PARALLEL${NC} (all 3 bots simultaneously) ⚡"
echo -e "${CYAN}Start Time:${NC} $(timestamp)"
echo -e "${CYAN}Expected End:${NC} $(date -d "+${DURATION_HOURS} hours" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -v+${DURATION_HOURS}H '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "${DURATION_HOURS} hours from now")"
echo ""

print_section "🎯 CONTESTANTS (Running Simultaneously)"
echo -e "   🔥 ${BOLD}Multi V5 AI${NC}       - Aggressive (15 levels @ 0.8%)"
echo -e "   ⚖️  ${BOLD}Balanced V4.1${NC}    - All-Weather (10 levels @ 1.5%)"
echo -e "   🛡️  ${BOLD}Conservative V4.1${NC} - Safe (7 levels @ 2.5%)"
echo ""

print_section "✨ V2.5 FEATURES TESTED"
echo -e "   ✅ Market Regime Gate (auto-pause in bad conditions)"
echo -e "   ✅ Smart Fee Filtering (dynamic fee limits per regime)"
echo -e "   ✅ Order Lifecycle (auto-refresh stale orders)"
echo -e "   ✅ Dynamic Grid Spacing (volatility-adjusted)"
echo ""

# ───────────────────────────────────────────────────────────────────────────
# ✅ PRE-FLIGHT CHECKS
# ───────────────────────────────────────────────────────────────────────────

print_section "🔍 PRE-FLIGHT CHECKS"

# Check configs exist
echo -n "Checking V2.5 configs... "
if [ ! -f "$CONFIG_MULTI" ] || [ ! -f "$CONFIG_BALANCED" ] || [ ! -f "$CONFIG_CONSERVATIVE" ]; then
    echo -e "${RED}❌ FAILED${NC}"
    echo -e "${RED}Error: V2.5 configs not found in ${CONFIG_DIR}/${NC}"
    echo -e "${YELLOW}Tip: Run 'git pull origin main' to get latest configs${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC}"

# Check if binary exists or needs build
echo -n "Checking binary... "
if [ ! -f "target/release/solana-grid-bot" ]; then
    echo -e "${YELLOW}Not found, building...${NC}"
    print_section "🔧 BUILDING RELEASE BINARY"
    cargo build --release
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Build failed!${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Build successful!${NC}"
else
    echo -e "${GREEN}✓${NC}"
fi

# Create directories
echo -n "Creating directories... "
mkdir -p "$LOG_DIR" "$RESULTS_DIR"
echo -e "${GREEN}✓${NC}"

# Check V5.0 Jupiter readiness (optional)
echo -n "Checking Jupiter V5.0 integration... "
if grep -q "jupiter-dex" Cargo.toml; then
    echo -e "${GREEN}✓ Ready${NC}"
else
    echo -e "${YELLOW}⚠️  Not enabled (simulation only)${NC}"
fi

echo ""
echo -e "${GREEN}✅ All checks passed!${NC}"
echo ""

# ───────────────────────────────────────────────────────────────────────────
# 🎮 RUN BOT FUNCTION (Background with proper logging)
# ───────────────────────────────────────────────────────────────────────────

run_bot_parallel() {
    local bot_name=$1
    local config_path=$2
    local emoji=$3
    local log_file="${LOG_DIR}/${bot_name}.log"
    local result_file="${RESULTS_DIR}/${bot_name}_results.json"
    
    (
        echo "═══════════════════════════════════════════════════════════════════════════"
        echo "${emoji} ${bot_name} - STARTED"
        echo "═══════════════════════════════════════════════════════════════════════════"
        echo "Config:   $config_path"
        echo "Duration: ${DURATION_HOURS}h"
        echo "PID:      $$"
        echo "Start:    $(timestamp)"
        echo "═══════════════════════════════════════════════════════════════════════════"
        echo ""
        
        ./target/release/solana-grid-bot run \
            --config "$config_path" \
            --duration-hours "$DURATION_HOURS" 2>&1
        
        local exit_code=$?
        
        echo ""
        echo "═══════════════════════════════════════════════════════════════════════════"
        echo "${emoji} ${bot_name} - COMPLETED"
        echo "═══════════════════════════════════════════════════════════════════════════"
        echo "End:        $(timestamp)"
        echo "Exit Code:  $exit_code"
        echo "Results:    $result_file"
        echo "═══════════════════════════════════════════════════════════════════════════"
        
        exit $exit_code
    ) > "$log_file" 2>&1 &
    
    local pid=$!
    echo -e "${emoji} ${BOLD}${bot_name}${NC} started (PID: ${CYAN}${pid}${NC})"
    echo -e "   ${BLUE}Log:${NC} tail -f ${log_file}"
    
    return $pid
}

# ───────────────────────────────────────────────────────────────────────────
# 🚀 PARALLEL EXECUTION
# ───────────────────────────────────────────────────────────────────────────

print_header "🚀 LAUNCHING 20-HOUR PARALLEL BATTLE ROYALE"

echo -e "${GREEN}⚡ Running all 3 bots simultaneously!${NC}"
echo ""
echo -e "${CYAN}Expected completion:${NC} $(date -d "+${DURATION_HOURS} hours" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -v+${DURATION_HOURS}H '+%Y-%m-%d %H:%M:%S' 2>/dev/null || echo "${DURATION_HOURS} hours from now")"
echo ""

print_section "🎬 STARTING BOTS"

# Start all 3 bots in parallel
run_bot_parallel "multi-v5-ai" "$CONFIG_MULTI" "🔥"
pids[multi]=$?

run_bot_parallel "balanced-v4.1" "$CONFIG_BALANCED" "⚖️"
pids[balanced]=$?

run_bot_parallel "conservative-v4.1" "$CONFIG_CONSERVATIVE" "🛡️"
pids[conservative]=$?

echo ""
echo -e "${GREEN}✅ All 3 bots launched!${NC}"
echo ""

print_section "📊 LIVE MONITORING"
echo "Monitor individual bots with:"
echo ""
echo -e "  ${CYAN}# Multi V5 AI (aggressive)${NC}"
echo "  tail -f ${LOG_DIR}/multi-v5-ai.log"
echo ""
echo -e "  ${CYAN}# Balanced V4.1 (all-weather)${NC}"
echo "  tail -f ${LOG_DIR}/balanced-v4.1.log"
echo ""
echo -e "  ${CYAN}# Conservative V4.1 (safe)${NC}"
echo "  tail -f ${LOG_DIR}/conservative-v4.1.log"
echo ""
echo -e "  ${CYAN}# Monitor all at once with multitail (if installed)${NC}"
echo "  multitail ${LOG_DIR}/*.log"
echo ""

print_section "⏳ WAITING FOR COMPLETION (${DURATION_HOURS} hours)"
echo -e "${YELLOW}This will take a while... Grab a coffee (or 10) ☕☕☕${NC}"
echo -e "${CYAN}Press Ctrl+C to stop all bots gracefully${NC}"
echo ""

# Track results
declare -A results

# Wait for all to complete and track exit codes
echo -e "${CYAN}Waiting for: 🔥 Multi V5 AI...${NC}"
wait ${pids[multi]}
results[multi]=$?
echo -e "${emoji} ${GREEN}Multi V5 AI finished!${NC} (exit ${results[multi]})"
echo ""

echo -e "${CYAN}Waiting for: ⚖️ Balanced V4.1...${NC}"
wait ${pids[balanced]}
results[balanced]=$?
echo -e "⚖️ ${GREEN}Balanced V4.1 finished!${NC} (exit ${results[balanced]})"
echo ""

echo -e "${CYAN}Waiting for: 🛡️ Conservative V4.1...${NC}"
wait ${pids[conservative]}
results[conservative]=$?
echo -e "🛡️ ${GREEN}Conservative V4.1 finished!${NC} (exit ${results[conservative]})"
echo ""

# ───────────────────────────────────────────────────────────────────────────
# 🏆 RESULTS SUMMARY
# ───────────────────────────────────────────────────────────────────────────

print_header "🏁 20-HOUR BATTLE ROYALE COMPLETE!"

echo -e "${CYAN}End Time:${NC} $(timestamp)"
echo -e "${CYAN}Duration:${NC} ${DURATION_HOURS} hours"
echo -e "${CYAN}Session ID:${NC} ${SESSION_ID}"
echo ""

print_section "📊 COMPLETION STATUS"

if [ ${results[multi]} -eq 0 ]; then
    echo -e "   🔥 Multi V5 AI:        ${GREEN}✅ SUCCESS${NC}"
else
    echo -e "   🔥 Multi V5 AI:        ${RED}❌ FAILED (exit ${results[multi]})${NC}"
fi

if [ ${results[balanced]} -eq 0 ]; then
    echo -e "   ⚖️  Balanced V4.1:     ${GREEN}✅ SUCCESS${NC}"
else
    echo -e "   ⚖️  Balanced V4.1:     ${RED}❌ FAILED (exit ${results[balanced]})${NC}"
fi

if [ ${results[conservative]} -eq 0 ]; then
    echo -e "   🛡️  Conservative V4.1: ${GREEN}✅ SUCCESS${NC}"
else
    echo -e "   🛡️  Conservative V4.1: ${RED}❌ FAILED (exit ${results[conservative]})${NC}"
fi

echo ""

print_section "📁 OUTPUT FILES"
echo -e "   ${BLUE}Logs:${NC}    ${LOG_DIR}/"
echo -e "   ${BLUE}Results:${NC} ${RESULTS_DIR}/"
echo ""

print_section "🔍 QUICK ANALYSIS COMMANDS"
echo "Compare performance across all 3 bots:"
echo ""
echo -e "  ${CYAN}# Final P&L comparison${NC}"
echo "  grep -h 'Final PnL' ${LOG_DIR}/*.log | sort"
echo ""
echo -e "  ${CYAN}# Total trades per bot${NC}"
echo "  for log in ${LOG_DIR}/*.log; do echo \"\$(basename \$log): \$(grep -c 'Trade executed' \$log) trades\"; done"
echo ""
echo -e "  ${CYAN}# Regime pauses (V2.5 feature)${NC}"
echo "  for log in ${LOG_DIR}/*.log; do echo \"\$(basename \$log): \$(grep -c 'Regime.*PAUSED' \$log) pauses\"; done"
echo ""
echo -e "  ${CYAN}# Fee rejections (V2.5 feature)${NC}"
echo "  for log in ${LOG_DIR}/*.log; do echo \"\$(basename \$log): \$(grep -c 'Fee rejected' \$log) rejections\"; done"
echo ""
echo -e "  ${CYAN}# Order refreshes (V2.5 feature)${NC}"
echo "  for log in ${LOG_DIR}/*.log; do echo \"\$(basename \$log): \$(grep -c 'Order refreshed' \$log) refreshes\"; done"
echo ""

print_section "🎯 NEXT STEPS"
echo "1. Analyze logs for detailed performance metrics"
echo "2. Compare V2.5 feature effectiveness"
echo "3. Calculate Sharpe ratios and risk metrics"
echo "4. Crown the champion! 🏆"
echo "5. Deploy winner to mainnet with Jupiter V5.0 🚀"
echo ""

echo -e "${GREEN}🎉 Epic 20-hour battle royale complete! May the best bot win! 🏆${NC}"
echo ""
