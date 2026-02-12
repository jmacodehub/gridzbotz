#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🔥🏆 BATTLE ROYALE V2.5 - ULTIMATE SHOWDOWN! 🏆🔥
# 
# Version: 2.5 (Enhanced)
# Duration: Configurable (default 10 hours)
# Configs: 3 V2.5 optimized strategies with Jupiter V5.0 ready
# Goal: Crown the champion & deploy to mainnet!
# 
# V2.5 Features Tested:
# ✅ Market Regime Gate (auto-pause)
# ✅ Smart Fee Filtering (dynamic limits)
# ✅ Order Lifecycle (auto-refresh)
# ✅ Dynamic Grid Spacing (volatility-based)
# 
# February 13, 2026 - Enhanced Battle Royale! 🚀
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
# ⚙️ CONFIGURATION
# ───────────────────────────────────────────────────────────────────────────
DURATION_HOURS=${1:-10}         # Default 10 hours, override with arg
PARALLEL=${PARALLEL:-false}     # Set PARALLEL=true for parallel execution
SESSION_ID=$(date +"%Y%m%d_%H%M%S")
LOG_DIR="logs/battle_royale_${SESSION_ID}"
RESULTS_DIR="results/battle_royale_${SESSION_ID}"

# Config paths (V2.5)
CONFIG_DIR="config/optimized"
CONFIG_MULTI="${CONFIG_DIR}/multi-v5-ai.toml"
CONFIG_BALANCED="${CONFIG_DIR}/balanced-v4.1.toml"
CONFIG_CONSERVATIVE="${CONFIG_DIR}/conservative-v4.1.toml"

# ───────────────────────────────────────────────────────────────────────────
# 🛠️ HELPER FUNCTIONS
# ───────────────────────────────────────────────────────────────────────────

print_header() {
    echo ""
    echo -e "${CYAN}════════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${BOLD}${WHITE}$1${NC}"
    echo -e "${CYAN}════════════════════════════════════════════════════════════════════════════${NC}"
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

# ───────────────────────────────────────────────────────────────────────────
# 🚀 BANNER
# ───────────────────────────────────────────────────────────────────────────

clear
print_header "🔥🏆 BATTLE ROYALE V2.5 - ULTIMATE SHOWDOWN! 🏆🔥"

echo -e "${CYAN}Version:${NC} 2.5 Enhanced"
echo -e "${CYAN}Duration:${NC} ${DURATION_HOURS} hours"
echo -e "${CYAN}Session ID:${NC} ${SESSION_ID}"
echo -e "${CYAN}Mode:${NC} $([ "$PARALLEL" = "true" ] && echo "Parallel" || echo "Sequential")"
echo -e "${CYAN}Start Time:${NC} $(timestamp)"
echo ""

print_section "🎯 CONTESTANTS"
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
# 🎮 RUN BOT FUNCTION
# ───────────────────────────────────────────────────────────────────────────

run_bot() {
    local bot_name=$1
    local config_path=$2
    local emoji=$3
    local log_file="${LOG_DIR}/${bot_name}.log"
    local result_file="${RESULTS_DIR}/${bot_name}_results.json"
    
    echo ""
    echo -e "${PURPLE}───────────────────────────────────────────────────────────────────────────${NC}"
    echo -e "${emoji} ${BOLD}Starting: ${bot_name}${NC}"
    echo -e "${PURPLE}───────────────────────────────────────────────────────────────────────────${NC}"
    echo -e "${CYAN}Config:${NC} $config_path"
    echo -e "${CYAN}Duration:${NC} ${DURATION_HOURS}h"
    echo -e "${CYAN}Log:${NC} $log_file"
    echo -e "${CYAN}Start:${NC} $(timestamp)"
    echo ""
    
    # Run bot with proper error handling
    set +e  # Don't exit on error for this command
    ./target/release/solana-grid-bot run \
        --config "$config_path" \
        --duration-hours "$DURATION_HOURS" \
        2>&1 | tee "$log_file"
    
    local exit_code=$?
    set -e
    
    echo ""
    echo -e "${CYAN}End:${NC} $(timestamp)"
    
    if [ $exit_code -eq 0 ]; then
        echo -e "${emoji} ${GREEN}✅ ${bot_name} completed successfully!${NC}"
    else
        echo -e "${emoji} ${RED}❌ ${bot_name} failed (exit code: $exit_code)${NC}"
    fi
    
    echo -e "${BLUE}📊 Results: ${result_file}${NC}"
    echo ""
    
    return $exit_code
}

# ───────────────────────────────────────────────────────────────────────────
# 🚀 EXECUTION
# ───────────────────────────────────────────────────────────────────────────

print_header "🚀 LAUNCHING BATTLE ROYALE"

echo -e "${YELLOW}⏱️  Expected completion: $(date -d "+${DURATION_HOURS} hours" 2>/dev/null || date -v+${DURATION_HOURS}H 2>/dev/null || echo "${DURATION_HOURS} hours from now")${NC}"
echo ""

# Track results
declare -A results
declare -A pids

if [ "$PARALLEL" = "true" ]; then
    # ☀️ PARALLEL MODE
    echo -e "${CYAN}⚡ Running all bots in parallel...${NC}"
    echo ""
    
    run_bot "multi-v5-ai" "$CONFIG_MULTI" "🔥" &
    pids[multi]=$!
    
    run_bot "balanced-v4.1" "$CONFIG_BALANCED" "⚖️" &
    pids[balanced]=$!
    
    run_bot "conservative-v4.1" "$CONFIG_CONSERVATIVE" "🛡️" &
    pids[conservative]=$!
    
    # Wait for all to complete
    echo -e "${CYAN}🕒 Waiting for all bots to complete...${NC}"
    echo ""
    
    wait ${pids[multi]}
    results[multi]=$?
    
    wait ${pids[balanced]}
    results[balanced]=$?
    
    wait ${pids[conservative]}
    results[conservative]=$?
else
    # 🔄 SEQUENTIAL MODE (Default)
    echo -e "${CYAN}🔄 Running bots sequentially...${NC}"
    echo ""
    
    run_bot "multi-v5-ai" "$CONFIG_MULTI" "🔥"
    results[multi]=$?
    
    run_bot "balanced-v4.1" "$CONFIG_BALANCED" "⚖️"
    results[balanced]=$?
    
    run_bot "conservative-v4.1" "$CONFIG_CONSERVATIVE" "🛡️"
    results[conservative]=$?
fi

# ───────────────────────────────────────────────────────────────────────────
# 🏆 RESULTS SUMMARY
# ───────────────────────────────────────────────────────────────────────────

print_header "🏁 BATTLE ROYALE COMPLETE!"

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

print_section "🔍 NEXT STEPS"
echo "1. Review detailed logs in ${LOG_DIR}/"
echo "2. Analyze results (PnL, trades, regime changes)"
echo "3. Compare V2.5 features performance"
echo "4. Crown the champion! 🏆"
echo "5. Deploy winner to mainnet with Jupiter V5.0"
echo ""

print_section "📊 QUICK ANALYSIS"
echo "Run these commands to analyze results:"
echo ""
echo -e "  ${CYAN}# View logs${NC}"
echo "  tail -f ${LOG_DIR}/multi-v5-ai.log"
echo ""
echo -e "  ${CYAN}# Compare final P&L${NC}"
echo "  grep 'Final PnL' ${LOG_DIR}/*.log"
echo ""
echo -e "  ${CYAN}# Count regime pauses${NC}"
echo "  grep 'Regime Gate: PAUSED' ${LOG_DIR}/*.log | wc -l"
echo ""
echo -e "  ${CYAN}# Count fee rejections${NC}"
echo "  grep 'Fee rejected' ${LOG_DIR}/*.log | wc -l"
echo ""

echo -e "${GREEN}🎉 Battle Royale V2.5 Complete! Good luck analyzing! 🚀${NC}"
echo ""
