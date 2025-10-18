#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 🌙 PROJECT FLASH V3.5 - OVERNIGHT TEST SUITE LAUNCHER
# Runs 5 bots in parallel with different strategies
# October 17, 2025 - LFG!!!
# ═══════════════════════════════════════════════════════════════════════════

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  🌙 PROJECT FLASH V3.5 - OVERNIGHT TEST SUITE"
echo "  Running 5 Bots in Parallel | 8 Hour Duration"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Pre-flight Checks
# ─────────────────────────────────────────────────────────────────────────

echo "🔍 Pre-flight checks..."

# Check if configs exist
CONFIGS=(
    "config/overnight_conservative.toml"
    "config/overnight_balanced.toml"
    "config/overnight_aggressive.toml"
    "config/overnight_testing.toml"
    "config/overnight_multi_strategy.toml"
)

for config in "${CONFIGS[@]}"; do
    if [ ! -f "$config" ]; then
        echo -e "${RED}❌ Missing config: $config${NC}"
        exit 1
    fi
done

echo -e "${GREEN}✅ All configs present${NC}"

# Create directories
echo "📁 Creating directories..."
mkdir -p logs
mkdir -p results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="results/overnight_${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

echo -e "${GREEN}✅ Results directory: $RESULTS_DIR${NC}"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Build Release Binary
# ─────────────────────────────────────────────────────────────────────────

echo "🔨 Building release binary..."
cargo build --release --quiet

if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build complete!${NC}"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Launch Bots
# ─────────────────────────────────────────────────────────────────────────

echo "🚀 Launching bots..."
echo ""

# Bot 1: Conservative
echo -e "${BLUE}  1️⃣  Conservative Bot${NC}"
echo "     📊 0.30% spacing | 20 levels | Low risk"
echo "     🛡️  Regime gate: ON | High volatility threshold"
nohup ./target/release/solana-grid-bot \
    --config config/overnight_conservative.toml \
    > "$RESULTS_DIR/conservative.txt" 2>&1 &
PID_CONSERVATIVE=$!
echo "     ✅ PID: $PID_CONSERVATIVE"
echo ""
sleep 2

# Bot 2: Balanced
echo -e "${GREEN}  2️⃣  Balanced Bot${NC}"
echo "     📊 0.15% spacing | 35 levels | Balanced risk"
echo "     ⚖️  Regime gate: OFF | Trades freely"
nohup ./target/release/solana-grid-bot \
    --config config/overnight_balanced.toml \
    > "$RESULTS_DIR/balanced.txt" 2>&1 &
PID_BALANCED=$!
echo "     ✅ PID: $PID_BALANCED"
echo ""
sleep 2

# Bot 3: Aggressive
echo -e "${YELLOW}  3️⃣  Aggressive Bot${NC}"
echo "     📊 0.10% spacing | 50 levels | High frequency"
echo "     ⚡ Regime gate: ON | Low volatility threshold"
nohup ./target/release/solana-grid-bot \
    --config config/overnight_aggressive.toml \
    > "$RESULTS_DIR/aggressive.txt" 2>&1 &
PID_AGGRESSIVE=$!
echo "     ✅ PID: $PID_AGGRESSIVE"
echo ""
sleep 2

# Bot 4: Testing
echo -e "${PURPLE}  4️⃣  Testing Bot${NC}"
echo "     📊 0.15% spacing | 35 levels | No restrictions"
echo "     🧪 Regime gate: OFF | All safety OFF"
nohup ./target/release/solana-grid-bot \
    --config config/overnight_testing.toml \
    > "$RESULTS_DIR/testing.txt" 2>&1 &
PID_TESTING=$!
echo "     ✅ PID: $PID_TESTING"
echo ""
sleep 2

# Bot 5: Multi-Strategy
echo -e "${CYAN}  5️⃣  Multi-Strategy Bot${NC}"
echo "     📊 0.20% spacing | 30 levels | Weighted consensus"
echo "     🧠 Grid + Momentum + RSI (experimental)"
nohup ./target/release/solana-grid-bot \
    --config config/overnight_multi_strategy.toml \
    > "$RESULTS_DIR/multi_strategy.txt" 2>&1 &
PID_MULTI=$!
echo "     ✅ PID: $PID_MULTI"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Save PIDs and Create Summary
# ─────────────────────────────────────────────────────────────────────────

echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ All 5 bots launched successfully!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Save PIDs
echo "$PID_CONSERVATIVE" > "$RESULTS_DIR/conservative.pid"
echo "$PID_BALANCED" > "$RESULTS_DIR/balanced.pid"
echo "$PID_AGGRESSIVE" > "$RESULTS_DIR/aggressive.pid"
echo "$PID_TESTING" > "$RESULTS_DIR/testing.pid"
echo "$PID_MULTI" > "$RESULTS_DIR/multi_strategy.pid"

# Create summary file
cat > "$RESULTS_DIR/SUITE_INFO.txt" << EOF
═══════════════════════════════════════════════════════════════════════════
🌙 OVERNIGHT TEST SUITE V3.5
Started: $(date)
Duration: 8 hours
═══════════════════════════════════════════════════════════════════════════

BOTS RUNNING:
─────────────────────────────────────────────────────────────────────────
1. Conservative  (PID: $PID_CONSERVATIVE)
   - Spacing: 0.30% | Levels: 20 | Regime Gate: ON
   
2. Balanced      (PID: $PID_BALANCED)
   - Spacing: 0.15% | Levels: 35 | Regime Gate: OFF
   
3. Aggressive    (PID: $PID_AGGRESSIVE)
   - Spacing: 0.10% | Levels: 50 | Regime Gate: ON
   
4. Testing       (PID: $PID_TESTING)
   - Spacing: 0.15% | Levels: 35 | All Safety OFF
   
5. Multi-Strategy (PID: $PID_MULTI)
   - Spacing: 0.20% | Levels: 30 | Weighted Consensus

═══════════════════════════════════════════════════════════════════════════
MONITORING COMMANDS:
═══════════════════════════════════════════════════════════════════════════

Check status:
  ./scripts/monitor_suite.sh

View logs:
  tail -f logs/overnight_*.log

Stop all bots:
  kill $PID_CONSERVATIVE $PID_BALANCED $PID_AGGRESSIVE $PID_TESTING $PID_MULTI

═══════════════════════════════════════════════════════════════════════════
EOF

echo "📊 Process IDs:"
echo -e "   ${BLUE}Conservative:${NC}    $PID_CONSERVATIVE"
echo -e "   ${GREEN}Balanced:${NC}        $PID_BALANCED"
echo -e "   ${YELLOW}Aggressive:${NC}      $PID_AGGRESSIVE"
echo -e "   ${PURPLE}Testing:${NC}         $PID_TESTING"
echo -e "   ${CYAN}Multi-Strategy:${NC}  $PID_MULTI"
echo ""

echo "💾 PIDs saved to: $RESULTS_DIR/*.pid"
echo "📝 Suite info: $RESULTS_DIR/SUITE_INFO.txt"
echo ""

echo "📊 MONITORING:"
echo "   Status:  ./scripts/monitor_suite.sh"
echo "   Logs:    tail -f logs/overnight_*.log"
echo ""

echo "🛑 STOP ALL BOTS:"
echo "   kill \$(cat $RESULTS_DIR/*.pid)"
echo ""

echo "🌙 Tests will run for 8 hours."
echo "   Expected completion: $(date -d '+8 hours' '+%Y-%m-%d %H:%M:%S')"
echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}🎉 SUITE LAUNCHED! Good night! 💤${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
