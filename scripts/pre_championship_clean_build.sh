#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🔧 PROJECT FLASH - PRE-CHAMPIONSHIP CLEAN & BUILD
# ═══════════════════════════════════════════════════════════════════════════
# Ensures all config changes are compiled into the binary
# ═══════════════════════════════════════════════════════════════════════════

set -euo pipefail

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
# 🎨 HEADER
# ──────────────────────────────────────────────
clear
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}    🔧 PRE-CHAMPIONSHIP CLEAN & BUILD 🔧${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${YELLOW}This will:${RESET}"
echo -e "  ${CYAN}1.${RESET} Clean all build artifacts"
echo -e "  ${CYAN}2.${RESET} Remove old binaries"
echo -e "  ${CYAN}3.${RESET} Rebuild from scratch (release mode)"
echo -e "  ${CYAN}4.${RESET} Verify configs"
echo -e "  ${CYAN}5.${RESET} Prepare for Championship 10 launch"
echo ""
echo -e "${YELLOW}⚠️  This will take 2-5 minutes depending on your CPU${RESET}"
echo ""
read -p "🔧 Ready to clean & rebuild? (y/n) " -n 1 -r
echo ""
[[ ! $REPLY =~ ^[Yy]$ ]] && echo -e "${RED}❌ Cancelled.${RESET}" && exit 0

# ──────────────────────────────────────────────
# 🧹 STEP 1: CLEAN
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}STEP 1/5: Cleaning build artifacts...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

# Kill any running bots first
echo -e "${YELLOW}🛑 Checking for running bots...${RESET}"
RUNNING_BOTS=$(pgrep -f "solana-grid-bot" || echo "")
if [[ -n "$RUNNING_BOTS" ]]; then
  echo -e "${YELLOW}⚠️  Found running bot processes: $RUNNING_BOTS${RESET}"
  read -p "   Kill them before continuing? (y/n) " -n 1 -r
  echo ""
  if [[ $REPLY =~ ^[Yy]$ ]]; then
    pkill -f "solana-grid-bot" || true
    sleep 2
    echo -e "${GREEN}✅ Killed running bots${RESET}"
  else
    echo -e "${RED}❌ Cannot proceed with bots running. Please stop them first.${RESET}"
    exit 1
  fi
else
  echo -e "${GREEN}✅ No running bots found${RESET}"
fi

echo ""
echo -e "${CYAN}🧹 Running cargo clean...${RESET}"
cargo clean

echo -e "${GREEN}✅ Build artifacts cleaned${RESET}"

# ──────────────────────────────────────────────
# 📝 STEP 2: VERIFY CONFIGS
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}STEP 2/5: Verifying Championship 10 configs...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

# Championship 10 configs
CHAMPIONSHIP_CONFIGS=(
  "config/production/v5.0_development/beta.toml"
  "config/production/v5.0_development/flash.toml"
  "config/production/v5.0_development/micro.toml"
  "config/production/v5.0_development/master_v2.toml"
  "config/production/v5.0_development/opportunist.toml"
  "config/production/v5.0_development/hornet.toml"
  "config/production/v5.0_development/sniper.toml"
  "config/production/v5.0_development/mean_reversion.toml"
  "config/production/v5.0_development/momentum_breakout.toml"
  "config/production/v5.0_development/champion.toml"
)

MISSING_COUNT=0
for config in "${CHAMPIONSHIP_CONFIGS[@]}"; do
  if [[ -f "$config" ]]; then
    echo -e "${GREEN}✅${RESET} $(basename "$config" .toml)"
  else
    echo -e "${RED}❌${RESET} $(basename "$config" .toml) ${RED}(MISSING!)${RESET}"
    MISSING_COUNT=$((MISSING_COUNT + 1))
  fi
done

echo ""
if [[ $MISSING_COUNT -gt 0 ]]; then
  echo -e "${RED}❌ $MISSING_COUNT config(s) missing!${RESET}"
  echo -e "${YELLOW}   Please create missing configs before launching${RESET}"
  exit 1
else
  echo -e "${GREEN}✅ All 10 configs verified!${RESET}"
fi

# Verify capital settings
echo ""
echo -e "${CYAN}🔍 Verifying capital settings (should all be $1000 USDC, 2 SOL)...${RESET}"
CAPITAL_ISSUES=0
for config in "${CHAMPIONSHIP_CONFIGS[@]}"; do
  name=$(basename "$config" .toml)
  usdc=$(grep "initial_usdc" "$config" | grep -v "#" | head -1 | grep -oE '[0-9]+(\.[0-9]+)?' || echo "MISSING")
  sol=$(grep "initial_sol" "$config" | grep -v "#" | head -1 | grep -oE '[0-9]+(\.[0-9]+)?' || echo "MISSING")

  if [[ "$usdc" == "1000.0" && "$sol" == "2.0" ]]; then
    echo -e "  ${GREEN}✅${RESET} $name: \$$usdc USDC, $sol SOL"
  else
    echo -e "  ${YELLOW}⚠️${RESET}  $name: \$$usdc USDC, $sol SOL ${YELLOW}(not standard!)${RESET}"
    CAPITAL_ISSUES=$((CAPITAL_ISSUES + 1))
  fi
done

echo ""
if [[ $CAPITAL_ISSUES -gt 0 ]]; then
  echo -e "${YELLOW}⚠️  $CAPITAL_ISSUES bot(s) have non-standard capital${RESET}"
  echo -e "${YELLOW}   This is OK if intentional (e.g., Opportunist test)${RESET}"
  read -p "   Continue anyway? (y/n) " -n 1 -r
  echo ""
  [[ ! $REPLY =~ ^[Yy]$ ]] && exit 1
else
  echo -e "${GREEN}✅ All bots have equal capital! Perfect for comparison!${RESET}"
fi

# ──────────────────────────────────────────────
# 🔨 STEP 3: BUILD RELEASE
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}STEP 3/5: Building release binary...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

echo -e "${CYAN}🔨 Running cargo build --release...${RESET}"
echo -e "${YELLOW}   This will take 2-5 minutes...${RESET}"
echo ""

START_TIME=$(date +%s)

# Build with progress
if cargo build --release 2>&1 | grep -E "(Compiling|Finished)"; then
  BUILD_SUCCESS=1
else
  BUILD_SUCCESS=0
fi

END_TIME=$(date +%s)
BUILD_DURATION=$((END_TIME - START_TIME))

echo ""
if [[ $BUILD_SUCCESS -eq 1 ]]; then
  echo -e "${GREEN}✅ Build completed in ${BUILD_DURATION}s!${RESET}"
else
  echo -e "${RED}❌ Build failed!${RESET}"
  exit 1
fi

# ──────────────────────────────────────────────
# ✅ STEP 4: VERIFY BINARY
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}STEP 4/5: Verifying binary...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

if [[ -f "target/release/solana-grid-bot" ]]; then
  BINARY_SIZE=$(ls -lh target/release/solana-grid-bot | awk '{print $5}')
  echo -e "${GREEN}✅ Binary exists: $BINARY_SIZE${RESET}"

  # Get build timestamp
  BINARY_TIME=$(stat -f "%Sm" -t "%Y-%m-%d %H:%M:%S" target/release/solana-grid-bot 2>/dev/null || \
                stat -c "%y" target/release/solana-grid-bot 2>/dev/null | cut -d. -f1)
  echo -e "${GREEN}✅ Build time: $BINARY_TIME${RESET}"
else
  echo -e "${RED}❌ Binary not found!${RESET}"
  exit 1
fi

# ──────────────────────────────────────────────
# 📊 STEP 5: FINAL CHECKLIST
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}STEP 5/5: Final checklist...${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""

CHECKS_PASSED=0
CHECKS_TOTAL=6

# Check 1: Binary exists
if [[ -f "target/release/solana-grid-bot" ]]; then
  echo -e "${GREEN}✅${RESET} Release binary exists"
  CHECKS_PASSED=$((CHECKS_PASSED + 1))
else
  echo -e "${RED}❌${RESET} Release binary missing"
fi

# Check 2: Configs exist
if [[ $MISSING_COUNT -eq 0 ]]; then
  echo -e "${GREEN}✅${RESET} All 10 configs verified"
  CHECKS_PASSED=$((CHECKS_PASSED + 1))
else
  echo -e "${RED}❌${RESET} Some configs missing"
fi

# Check 3: Log directory
if [[ -d "logs/v5" ]]; then
  echo -e "${GREEN}✅${RESET} Log directory exists"
  CHECKS_PASSED=$((CHECKS_PASSED + 1))
else
  mkdir -p logs/v5
  echo -e "${GREEN}✅${RESET} Log directory created"
  CHECKS_PASSED=$((CHECKS_PASSED + 1))
fi

# Check 4: No running bots
RUNNING=$(pgrep -f "solana-grid-bot" | wc -l)
if [[ $RUNNING -eq 0 ]]; then
  echo -e "${GREEN}✅${RESET} No running bots (clean slate)"
  CHECKS_PASSED=$((CHECKS_PASSED + 1))
else
  echo -e "${YELLOW}⚠️${RESET}  $RUNNING bot(s) still running"
fi

# Check 5: Disk space
DISK_FREE=$(df -h . | tail -1 | awk '{print $4}')
echo -e "${GREEN}✅${RESET} Disk space available: $DISK_FREE"
CHECKS_PASSED=$((CHECKS_PASSED + 1))

# Check 6: Memory
if command -v free &> /dev/null; then
  MEM_FREE=$(free -h | grep Mem | awk '{print $7}')
  echo -e "${GREEN}✅${RESET} Memory available: $MEM_FREE"
elif command -v vm_stat &> /dev/null; then
  MEM_FREE="$(vm_stat | grep 'Pages free' | awk '{print $3}' | sed 's/\.//')"
  MEM_FREE=$((MEM_FREE * 4096 / 1024 / 1024))
  echo -e "${GREEN}✅${RESET} Memory available: ~${MEM_FREE}MB"
else
  echo -e "${YELLOW}⚠️${RESET}  Memory check unavailable"
fi
CHECKS_PASSED=$((CHECKS_PASSED + 1))

# ──────────────────────────────────────────────
# 🎉 FINAL STATUS
# ──────────────────────────────────────────────
echo ""
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${GREEN}✅ PRE-CHAMPIONSHIP PREPARATION COMPLETE!${RESET}"
echo -e "${PURPLE}═══════════════════════════════════════════════════════════════════════════${RESET}"
echo ""
echo -e "${CYAN}📊 Summary:${RESET}"
echo -e "   Checks passed:  ${GREEN}$CHECKS_PASSED/$CHECKS_TOTAL${RESET}"
echo -e "   Binary size:    ${BINARY_SIZE}"
echo -e "   Build time:     ${BUILD_DURATION}s"
echo -e "   Configs ready:  ${GREEN}10/10${RESET}"
echo ""
echo -e "${BOLD}${GREEN}🏆 READY TO LAUNCH CHAMPIONSHIP 10! 🏆${RESET}"
echo ""
echo -e "${CYAN}Next steps:${RESET}"
echo -e "   ${GREEN}./scripts/launch_championship_10.sh 1200 all${RESET}"
echo ""
echo -e "${YELLOW}💡 Tip: Run in tmux or screen for 20-hour test!${RESET}"
echo ""
