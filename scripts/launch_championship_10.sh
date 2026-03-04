#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🏆 PROJECT FLASH V6.0 - CHAMPIONSHIP 10 LAUNCHER
# ═══════════════════════════════════════════════════════════════════════════
# The ULTIMATE 10-bot final test - Best of the Best!
# Usage:
#   ./launch_championship_10.sh 1200 [all|champions|specialists|darkhorse]
# Example (20-hour championship):
#   ./launch_championship_10.sh 1200 all
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail
DURATION_MINUTES=${1:-1200}
TEST_GROUP=${2:-all}
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
mkdir -p logs/v5

# ──────────────────────────────────────────────
# 🎨 COLORS
# ──────────────────────────────────────────────
RED="\033[0;31m"
GREEN="\033[0;32m"
YELLOW="\033[1;33m"
CYAN="\033[0;36m"
PURPLE="\033[0;35m"
BOLD="\033[1m"
RESET="\033[0m"

# ──────────────────────────────────────────────
# 🏆 CHAMPIONSHIP 10 BOT GROUPS
# ──────────────────────────────────────────────

# TIER S - THE CHAMPIONS (3)
CHAMPION_CONFIGS=(
  "config/production/v5.0_development/beta.toml"        # 🥇 $90M profit king
  "config/production/v5.0_development/flash.toml"       # ⚡ NEW hybrid legend
  "config/production/v5.0_development/micro.toml"       # 🥉 $44M solid performer
)

# TIER A - THE SPECIALISTS (3)
SPECIALIST_CONFIGS=(
  "config/production/v5.0_development/master_v2.toml"   # 💎 8.68 PT/Fill efficiency king
  "config/production/v5.0_development/opportunist.toml" # 🔥 8,465% ROI beast
  "config/production/v5.0_development/hornet.toml"      # ⚡ 4.55 PT/Fill balanced trader
)

# TIER B - THE DARK HORSES (4)
DARKHORSE_CONFIGS=(
  "config/production/v5.0_development/sniper.toml"            # 🎯 4.55 PT/Fill ultra-selective
  "config/production/v5.0_development/mean_reversion.toml"    # 🚀 $16M consistent
  "config/production/v5.0_development/momentum_breakout.toml" # 💪 $14M trend rider
  "config/production/v5.0_development/champion.toml"          # 🎲 Big-swing hunter
)

# ──────────────────────────────────────────────
# 🎯 CHOOSE TEST GROUP
# ──────────────────────────────────────────────
case $TEST_GROUP in
  champions)
    SELECTED_CONFIGS=("${CHAMPION_CONFIGS[@]}")
    GROUP_NAME="🏆 TIER S - THE CHAMPIONS (3 bots)"
    ;;
  specialists)
    SELECTED_CONFIGS=("${SPECIALIST_CONFIGS[@]}")
    GROUP_NAME="💎 TIER A - THE SPECIALISTS (3 bots)"
    ;;
  darkhorse)
    SELECTED_CONFIGS=("${DARKHORSE_CONFIGS[@]}")
    GROUP_NAME="🎲 TIER B - THE DARK HORSES (4 bots)"
    ;;
  all|*)
    SELECTED_CONFIGS=(
      "${CHAMPION_CONFIGS[@]}"
      "${SPECIALIST_CONFIGS[@]}"
      "${DARKHORSE_CONFIGS[@]}"
    )
    GROUP_NAME="🏆 CHAMPIONSHIP 10 - ULTIMATE SHOWDOWN"
    ;;
esac

BOT_COUNT=${#SELECTED_CONFIGS[@]}

# ──────────────────────────────────────────────
# 📊 CALCULATE ESTIMATES
# ──────────────────────────────────────────────
HOURS=$(echo "scale=1; $DURATION_MINUTES / 60" | bc)

# Cross-platform date calculation
if date -v+${DURATION_MINUTES}M "+%Y-%m-%d %H:%M:%S" >/dev/null 2>&1; then
  END_TIME=$(date -v+${DURATION_MINUTES}M "+%Y-%m-%d %H:%M:%S")
else
  END_TIME=$(date -d "+${DURATION_MINUTES} minutes" "+%Y-%m-%d %H:%M:%S" 2>/dev/null || echo "Unknown")
fi

# ──────────────────────────────────────────────
# 🎨 EPIC CHAMPIONSHIP HEADER
# ──────────────────────────────────────────────
clear
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}         🏆 PROJECT FLASH - CHAMPIONSHIP 10 🏆${RESET}"
echo -e "${BOLD}${YELLOW}           THE ULTIMATE FINAL SHOWDOWN${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${GREEN}📊 Championship Configuration:${RESET}"
echo -e "   Group:        ${YELLOW}${GROUP_NAME}${RESET}"
echo -e "   Bot Count:    ${CYAN}${BOT_COUNT} elite bots${RESET}"
echo -e "   Duration:     ${YELLOW}${DURATION_MINUTES} minutes (${HOURS} hours)${RESET}"
echo -e "   Session ID:   ${CYAN}${TIMESTAMP}${RESET}"
echo -e "   Start Time:   ${GREEN}$(date "+%Y-%m-%d %H:%M:%S")${RESET}"
echo -e "   Est. End:     ${GREEN}${END_TIME}${RESET}"
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"

# Show detailed bot lineup for "all"
if [[ "$TEST_GROUP" == "all" ]]; then
  echo -e "${BOLD}${YELLOW}🎯 THE CHAMPIONSHIP 10 LINEUP:${RESET}"
  echo ""
  echo -e "${BOLD}${GREEN}TIER S - THE CHAMPIONS (3):${RESET}"
  echo -e "   ${BOLD}#1 🥇 BETA${RESET}        - \$90M profit champion | 3.68 PT/Fill"
  echo -e "   ${BOLD}#2 ⚡ FLASH${RESET}       - NEW hybrid legend | Projected \$48M"
  echo -e "   ${BOLD}#3 🥉 MICRO${RESET}       - \$44M solid performer | 3.79 PT/Fill"
  echo ""
  echo -e "${BOLD}${CYAN}TIER A - THE SPECIALISTS (3):${RESET}"
  echo -e "   ${BOLD}#4 💎 MASTER_V2${RESET}   - 8.68 PT/Fill efficiency KING | Mainnet ready"
  echo -e "   ${BOLD}#5 🔥 OPPORTUNIST${RESET} - 8,465% ROI beast | \$42M on \$500"
  echo -e "   ${BOLD}#6 ⚡ HORNET${RESET}      - 4.55 PT/Fill balanced trader | \$3.8M"
  echo ""
  echo -e "${BOLD}${PURPLE}TIER B - THE DARK HORSES (4):${RESET}"
  echo -e "   ${BOLD}#7 🎯 SNIPER${RESET}      - 4.55 PT/Fill ultra-selective | 7% spacing"
  echo -e "   ${BOLD}#8 🚀 MEAN_REV${RESET}    - \$16M consistent performer | 2.38 PT/Fill"
  echo -e "   ${BOLD}#9 💪 MOMENTUM${RESET}    - \$14M trend rider | 2.65 PT/Fill"
  echo -e "   ${BOLD}#10 🎲 CHAMPION${RESET}   - Big-swing hunter | 4% spacing wildcard"
  echo ""
  echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
  echo ""
  echo -e "${BOLD}${YELLOW}🎯 CHAMPIONSHIP OBJECTIVES:${RESET}"
  echo -e "   ✅ Validate Flash's early performance"
  echo -e "   ✅ Find THE ultimate mainnet bot"
  echo -e "   ✅ Test all strategies in same conditions"
  echo -e "   ✅ Select top 3 for devnet deployment"
  echo -e "   ✅ Crown the CHAMPION! 👑"
  echo ""
  echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
fi

echo ""
echo -e "${YELLOW}⚠️  This will launch ${BOT_COUNT} bots in parallel for ${HOURS} hours!${RESET}"
echo -e "${YELLOW}⚠️  Estimated memory usage: ~${BOT_COUNT}GB RAM${RESET}"
echo -e "${YELLOW}⚠️  CPU usage will be HIGH during startup${RESET}"
echo -e "${YELLOW}⚠️  Ensure you have enough disk space for logs!${RESET}"
echo ""
read -p "🚀 Ready to launch THE CHAMPIONSHIP 10? (y/n) " -n 1 -r
echo ""
[[ ! $REPLY =~ ^[Yy]$ ]] && echo -e "${RED}❌ Championship cancelled.${RESET}" && exit 0

# ──────────────────────────────────────────────
# 🔨 BUILD CHECK
# ──────────────────────────────────────────────
echo ""
echo -e "${CYAN}🔨 Checking binary...${RESET}"
if [[ ! -f "target/release/solana-grid-bot" ]]; then
  echo -e "${YELLOW}⚠️  Binary not found. Building release version...${RESET}"
  cargo build --release
  echo -e "${GREEN}✅ Build complete!${RESET}"
else
  echo -e "${GREEN}✅ Binary exists ($(ls -lh target/release/solana-grid-bot | awk '{print $5}'))${RESET}"
fi

# ──────────────────────────────────────────────
# 🚀 CHAMPIONSHIP LAUNCH SEQUENCE
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}🚀 INITIATING CHAMPIONSHIP 10 LAUNCH SEQUENCE...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

declare -a PIDS=()
declare -a BOT_NAMES=()
LAUNCH_COUNT=0

for config in "${SELECTED_CONFIGS[@]}"; do
  name=$(basename "$config" .toml)
  logfile="logs/v5/${name}_${TIMESTAMP}.log"

  LAUNCH_COUNT=$((LAUNCH_COUNT + 1))

  # Assign tier emoji
  case $name in
    beta|flash|micro)
      tier="🥇"
      ;;
    master_v2|opportunist|hornet)
      tier="💎"
      ;;
    sniper|mean_reversion|momentum_breakout|champion)
      tier="🎲"
      ;;
    *)
      tier="🤖"
      ;;
  esac

  echo -e "${CYAN}[${LAUNCH_COUNT}/${BOT_COUNT}]${RESET} ${tier} ${GREEN}▶️  Launching${RESET} ${BOLD}${name}${RESET}"
  echo -e "       Config: ${config}"
  echo -e "       Log:    ${logfile}"

  # Verify config exists
  if [[ ! -f "$config" ]]; then
    echo -e "       ${RED}❌ Config not found! Skipping...${RESET}"
    echo ""
    continue
  fi

  # Launch bot using prebuilt binary
  ./target/release/solana-grid-bot \
    --config "$config" \
    --duration-minutes "$DURATION_MINUTES" >"$logfile" 2>&1 &

  pid=$!
  if [[ -n "${pid:-}" && "$pid" -gt 0 ]]; then
    echo -e "       ${GREEN}✅ PID: ${pid}${RESET}"
    PIDS+=("$pid")
    BOT_NAMES+=("$name")
  else
    echo -e "       ${RED}❌ Failed to launch${RESET}"
  fi

  # Stagger launches to avoid resource spike
  sleep 3
  echo ""
done

# ──────────────────────────────────────────────
# 📊 CHAMPIONSHIP SUMMARY
# ──────────────────────────────────────────────
LAUNCHED_COUNT=${#PIDS[@]}

echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}✅ ${LAUNCHED_COUNT}/${BOT_COUNT} BOTS LAUNCHED SUCCESSFULLY!${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${CYAN}📊 Championship Session Info:${RESET}"
echo -e "   Session ID:  ${BOLD}${TIMESTAMP}${RESET}"
echo -e "   Active Bots: ${BOLD}${BOT_NAMES[*]}${RESET}"
echo -e "   PIDs:        ${PIDS[*]}"
echo -e "   Logs:        logs/v5/*_${TIMESTAMP}.log"
echo ""
echo -e "${CYAN}🛠️  Management Commands:${RESET}"
echo -e "   Monitor:        ${GREEN}./monitoring/monitor_v9.sh${RESET}"
echo -e "   Deep Dive:      ${GREEN}./analysis/deep_dive_analysis.sh ${TIMESTAMP}${RESET}"
echo -e "   Comparison:     ${GREEN}./analysis/analyze_ultimate.sh full ${TIMESTAMP}${RESET}"
echo -e "   Stop All:       ${YELLOW}kill ${PIDS[*]}${RESET}"
echo -e "   Emergency Kill: ${RED}pkill -f solana-grid-bot${RESET}"
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"

# ──────────────────────────────────────────────
# 📊 AUTO-ATTACH MONITOR (Optional)
# ──────────────────────────────────────────────
echo ""
read -p "🖥️  Launch live monitor? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
  echo -e "${CYAN}💎 Starting live monitor for Championship 10...${RESET}"
  sleep 2

  # Launch monitor in background
  if [[ -f "monitoring/monitor_v9.sh" ]]; then
    nohup bash monitoring/monitor_v9.sh >/tmp/championship10_monitor_${TIMESTAMP}.log 2>&1 &
    MON_PID=$!
    echo -e "${GREEN}✅ Monitor launched (PID: ${MON_PID})${RESET}"
    echo -e "   Monitor log: /tmp/championship10_monitor_${TIMESTAMP}.log"
  else
    echo -e "${YELLOW}⚠️  Monitor script not found. Skipping...${RESET}"
  fi
fi

# ──────────────────────────────────────────────
# 🎯 FINAL CHAMPIONSHIP MESSAGE
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}        🏆 CHAMPIONSHIP 10 IS NOW LIVE! 🏆${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${YELLOW}💡 Pro Tips:${RESET}"
echo -e "   • Track progress:  ${GREEN}tail -f logs/v5/*_${TIMESTAMP}.log${RESET}"
echo -e "   • Check processes: ${GREEN}ps aux | grep solana-grid-bot | wc -l${RESET}"
echo -e "   • Live stats:      ${GREEN}watch -n 10 'grep -h \"Profit\\|PT\" logs/v5/*_${TIMESTAMP}.log | tail -20'${RESET}"
echo -e "   • Monitor system:  ${GREEN}htop${RESET} or ${GREEN}top${RESET}"
echo ""
echo -e "${CYAN}⏰ The championship will run for ${BOLD}${HOURS} hours${RESET}${CYAN} and stop automatically.${RESET}"
echo -e "${CYAN}📊 Results will be saved to ${BOLD}logs/v5/${RESET}${CYAN} when complete.${RESET}"
echo ""
echo -e "${BOLD}${YELLOW}🎯 THE GOAL:${RESET}"
echo -e "   ${GREEN}• Find THE ultimate bot${RESET}"
echo -e "   ${GREEN}• Validate Flash's potential${RESET}"
echo -e "   ${GREEN}• Select top 3 for devnet${RESET}"
echo -e "   ${GREEN}• Crown the MAINNET CHAMPION! 👑${RESET}"
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}              LET THE CHAMPIONSHIP BEGIN! 🔥💯🚀${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${BOLD}${CYAN}💎 May the best bot win! LFG BRO!!! ⚡🏆💎${RESET}"
echo ""

# Save session info for later analysis
cat > "logs/v5/championship10_${TIMESTAMP}_info.txt" << EOF
Championship 10 Session Info
═══════════════════════════════════════════════════════════════════════════

Session ID:    ${TIMESTAMP}
Start Time:    $(date "+%Y-%m-%d %H:%M:%S")
Duration:      ${DURATION_MINUTES} minutes (${HOURS} hours)
Est. End Time: ${END_TIME}

Bot Count:     ${LAUNCHED_COUNT}/${BOT_COUNT}
Active Bots:   ${BOT_NAMES[*]}
PIDs:          ${PIDS[*]}

Test Group:    ${TEST_GROUP}
Group Name:    ${GROUP_NAME}

═══════════════════════════════════════════════════════════════════════════
EOF

echo -e "${GREEN}✅ Session info saved to: logs/v5/championship10_${TIMESTAMP}_info.txt${RESET}"
echo ""
