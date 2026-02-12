#!/bin/bash

# ═════════════════════════════════════════════════════════════════════════
# 🔥 BATTLE ROYALE #2 - 5 Production Configs Parallel Test
# Runs all configs simultaneously for extended testing
# Perfect for overnight/work-day runs!
# ═════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Duration (default 20 hours)
DURATION=${1:-20}

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  🔥 BATTLE ROYALE #2 - Production Config Showdown"
echo "  5 Configs Running in Parallel | ${DURATION} Hours Duration"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Pre-flight Checks
# ─────────────────────────────────────────────────────────────────────────

echo "🔍 Pre-flight checks..."

# Check configs exist
CONFIGS=(
    "config/production/balanced.toml"
    "config/production/conservative.toml"
    "config/production/aggressive.toml"
    "config/production/ultra_aggressive.toml"
    "config/production/multi_strategy.toml"
)

for config in "${CONFIGS[@]}"; do
    if [ ! -f "$config" ]; then
        echo -e "${RED}❌ Missing config: $config${NC}"
        exit 1
    fi
done

echo -e "${GREEN}✅ All 5 configs present${NC}"

# Create directories
mkdir -p logs results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="results/battle_royale_${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

echo -e "${GREEN}✅ Results directory: $RESULTS_DIR${NC}"

# Build
echo ""
echo "🔨 Building release binary..."
cargo build --release --quiet

if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build complete!${NC}"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Launch Bots in Parallel
# ─────────────────────────────────────────────────────────────────────────

echo "════════════════════════════════════════════════════════════════════════"
echo "🚀 Launching bots..."
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Bot 1: Balanced
echo -e "${GREEN}  1️⃣  Balanced Bot${NC}"
echo "     📊 35 levels @ 0.15% | Regime gate OFF"
nohup ./target/release/solana-grid-bot \
    --config config/production/balanced.toml \
    --duration-hours "$DURATION" \
    > "$RESULTS_DIR/balanced.txt" 2>&1 &
PID_BALANCED=$!
echo "     ✅ PID: $PID_BALANCED"
echo ""
sleep 2

# Bot 2: Conservative
echo -e "${BLUE}  2️⃣  Conservative Bot${NC}"
echo "     🛡️  20 levels @ 0.25% | Low risk"
nohup ./target/release/solana-grid-bot \
    --config config/production/conservative.toml \
    --duration-hours "$DURATION" \
    > "$RESULTS_DIR/conservative.txt" 2>&1 &
PID_CONSERVATIVE=$!
echo "     ✅ PID: $PID_CONSERVATIVE"
echo ""
sleep 2

# Bot 3: Aggressive (NEW!)
echo -e "${YELLOW}  3️⃣  Aggressive Bot (🆕)${NC}"
echo "     ⚡ 50 levels @ 0.10% | High frequency"
nohup ./target/release/solana-grid-bot \
    --config config/production/aggressive.toml \
    --duration-hours "$DURATION" \
    > "$RESULTS_DIR/aggressive.txt" 2>&1 &
PID_AGGRESSIVE=$!
echo "     ✅ PID: $PID_AGGRESSIVE"
echo ""
sleep 2

# Bot 4: Ultra Aggressive
echo -e "${RED}  4️⃣  Ultra-Aggressive Bot${NC}"
echo "     🔥 60 levels @ 0.08% | Maximum fills"
nohup ./target/release/solana-grid-bot \
    --config config/production/ultra_aggressive.toml \
    --duration-hours "$DURATION" \
    > "$RESULTS_DIR/ultra_aggressive.txt" 2>&1 &
PID_ULTRA=$!
echo "     ✅ PID: $PID_ULTRA"
echo ""
sleep 2

# Bot 5: Multi-Strategy (NEW!)
echo -e "${PURPLE}  5️⃣  Multi-Strategy Bot (🆕)${NC}"
echo "     🧠 30 levels @ 0.20% | Grid+Momentum+RSI"
nohup ./target/release/solana-grid-bot \
    --config config/production/multi_strategy.toml \
    --duration-hours "$DURATION" \
    > "$RESULTS_DIR/multi_strategy.txt" 2>&1 &
PID_MULTI=$!
echo "     ✅ PID: $PID_MULTI"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Save Info & Create Summary
# ─────────────────────────────────────────────────────────────────────────

# Save PIDs
echo "$PID_BALANCED" > "$RESULTS_DIR/balanced.pid"
echo "$PID_CONSERVATIVE" > "$RESULTS_DIR/conservative.pid"
echo "$PID_AGGRESSIVE" > "$RESULTS_DIR/aggressive.pid"
echo "$PID_ULTRA" > "$RESULTS_DIR/ultra_aggressive.pid"
echo "$PID_MULTI" > "$RESULTS_DIR/multi_strategy.pid"

# Create suite info
END_TIME=$(date -d "+${DURATION} hours" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -v+${DURATION}H '+%Y-%m-%d %H:%M:%S')

cat > "$RESULTS_DIR/BATTLE_INFO.txt" << EOF
════════════════════════════════════════════════════════════════════════
🔥 BATTLE ROYALE #2 - PRODUCTION CONFIG SHOWDOWN
Started:  $(date)
Duration: ${DURATION} hours
Expected End: $END_TIME
════════════════════════════════════════════════════════════════════════

BOTS RUNNING:
────────────────────────────────────────────────────────────────────────
1. Balanced          (PID: $PID_BALANCED)
   35 levels @ 0.15% | Regime gate OFF
   
2. Conservative      (PID: $PID_CONSERVATIVE)
   20 levels @ 0.25% | Low risk
   
3. Aggressive (🆕)   (PID: $PID_AGGRESSIVE)
   50 levels @ 0.10% | High frequency
   
4. Ultra-Aggressive  (PID: $PID_ULTRA)
   60 levels @ 0.08% | Maximum fills
   
5. Multi-Strategy (🆕) (PID: $PID_MULTI)
   30 levels @ 0.20% | Grid+Momentum+RSI

════════════════════════════════════════════════════════════════════════
MONITORING COMMANDS:
════════════════════════════════════════════════════════════════════════

Check status of all bots:
  ps aux | grep solana-grid-bot

View live logs:
  tail -f $RESULTS_DIR/*.txt

Stop all bots:
  kill $PID_BALANCED $PID_CONSERVATIVE $PID_AGGRESSIVE $PID_ULTRA $PID_MULTI

Or:
  kill \$(cat $RESULTS_DIR/*.pid)

════════════════════════════════════════════════════════════════════════
EOF

echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ ALL 5 BOTS LAUNCHED SUCCESSFULLY!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

echo "📊 Process IDs:"
echo -e "   ${GREEN}Balanced:${NC}         $PID_BALANCED"
echo -e "   ${BLUE}Conservative:${NC}     $PID_CONSERVATIVE"
echo -e "   ${YELLOW}Aggressive:${NC}       $PID_AGGRESSIVE"
echo -e "   ${RED}Ultra-Aggressive:${NC} $PID_ULTRA"
echo -e "   ${PURPLE}Multi-Strategy:${NC}   $PID_MULTI"
echo ""

echo "💾 PIDs saved to: $RESULTS_DIR/*.pid"
echo "📋 Battle info: $RESULTS_DIR/BATTLE_INFO.txt"
echo ""

echo "📊 MONITORING:"
echo "   Logs:    tail -f $RESULTS_DIR/*.txt"
echo "   Status:  ps aux | grep solana-grid-bot"
echo ""

echo "🛑 STOP ALL:"
echo "   kill \$(cat $RESULTS_DIR/*.pid)"
echo ""

echo "🔥 Battle will run for ${DURATION} hours."
echo "   Expected completion: $END_TIME"
echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}🎉 BATTLE ROYALE #2 LAUNCHED! LFG! 🚀🔥${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
