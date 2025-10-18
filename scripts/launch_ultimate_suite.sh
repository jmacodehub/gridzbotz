#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 🌙 PROJECT FLASH V3.5 - ULTIMATE TEST SUITE
# Runs ALL 8 configurations in parallel!
# ═══════════════════════════════════════════════════════════════════════════

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m'

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  🚀 PROJECT FLASH V3.5 - ULTIMATE TEST SUITE"
echo "  Running 7 Bots Simultaneously | Complete Strategy Coverage"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Pre-flight checks
echo "🔍 Pre-flight checks..."

CONFIGS=(
    "config/overnight_conservative.toml"
    "config/overnight_balanced.toml"
    "config/overnight_aggressive.toml"
    "config/overnight_super_aggressive.toml"
    "config/overnight_ultra_aggressive.toml"
    "config/overnight_testing.toml"
    "config/overnight_multi_strategy.toml"
)

for config in "${CONFIGS[@]}"; do
    if [ ! -f "$config" ]; then
        echo -e "${RED}❌ Missing: $config${NC}"
        exit 1
    fi
done

echo -e "${GREEN}✅ All configs present${NC}"

# Create directories
mkdir -p logs results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="results/ultimate_${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

echo -e "${GREEN}✅ Results: $RESULTS_DIR${NC}"
echo ""

# Build
echo "🔨 Building release binary..."
cargo build --release

if [ $? -ne 0 ]; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build complete!${NC}"
echo ""

# Launch bots
echo "🚀 Launching 7-bot suite..."
echo ""

# Initialize empty string for PIDs
PIDS=""

# Bot 1: Conservative
echo -e "${BLUE}  1️⃣  Conservative (0.30%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_conservative.toml \
    > "$RESULTS_DIR/conservative.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/conservative.pid"
echo "     PID: $PID"
sleep 2

# Bot 2: Balanced
echo -e "${GREEN}  2️⃣  Balanced (0.15%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_balanced.toml \
    > "$RESULTS_DIR/balanced.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/balanced.pid"
echo "     PID: $PID"
sleep 2

# Bot 3: Aggressive
echo -e "${YELLOW}  3️⃣  Aggressive (0.10%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_aggressive.toml \
    > "$RESULTS_DIR/aggressive.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/aggressive.pid"
echo "     PID: $PID"
sleep 2

# Bot 4: Super Aggressive
echo -e "${YELLOW}  4️⃣  Super Aggressive (0.07%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_super_aggressive.toml \
    > "$RESULTS_DIR/super_aggressive.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/super_aggressive.pid"
echo "     PID: $PID"
sleep 2

# Bot 5: Ultra Aggressive
echo -e "${RED}  5️⃣  Ultra Aggressive (0.03%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_ultra_aggressive.toml \
    > "$RESULTS_DIR/ultra_aggressive.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/ultra_aggressive.pid"
echo "     PID: $PID"
sleep 2

# Bot 6: Testing
echo -e "${PURPLE}  6️⃣  Testing (0.15% - No Safety)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_testing.toml \
    > "$RESULTS_DIR/testing.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/testing.pid"
echo "     PID: $PID"
sleep 2

# Bot 7: Multi-Strategy
echo -e "${CYAN}  7️⃣  Multi-Strategy (0.20%)${NC}"
./target/release/solana-grid-bot \
    --config config/overnight_multi_strategy.toml \
    > "$RESULTS_DIR/multi_strategy.txt" 2>&1 &
PID=$!
PIDS="$PIDS $PID"
echo "$PID" > "$RESULTS_DIR/multi_strategy.pid"
echo "     PID: $PID"

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ All 7 bots launched!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# Create summary
cat > "$RESULTS_DIR/SUITE_INFO.txt" << EOF
═══════════════════════════════════════════════════════════════════════════
🌙 ULTIMATE TEST SUITE V3.5
Started: $(date)
Duration: 8 hours
Total Bots: 7
═══════════════════════════════════════════════════════════════════════════

PIDs:$PIDS

MONITORING:
═══════════════════════════════════════════════════════════════════════════

Status:     ./scripts/monitor_suite.sh
Analyze:    ./scripts/analyze_results.sh
Logs:       tail -f logs/overnight_*.log

Stop all:   kill$PIDS

═══════════════════════════════════════════════════════════════════════════
EOF

echo "📝 Suite info: $RESULTS_DIR/SUITE_INFO.txt"
echo ""
echo "📊 MONITORING:"
echo "   ./scripts/monitor_suite.sh"
echo "   ./scripts/analyze_results.sh"
echo ""
echo "🛑 STOP ALL: kill$PIDS"
echo ""
echo "🌙 Expected completion: $(date -d '+8 hours' '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -v+8H '+%Y-%m-%d %H:%M:%S' 2>/dev/null)"
echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}🎉 ULTIMATE SUITE LAUNCHED! Sleep well! 💤${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
