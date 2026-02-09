#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 🚀 PARALLEL QUICK TEST - 5 Configs Running Simultaneously
# Fast validation before launching long battles
# ═══════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# Duration (default 10 minutes)
DURATION=${1:-10}

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  🚀 PARALLEL QUICK TEST - All 5 Configs Simultaneously"
echo "  Duration: ${DURATION} Minutes | Parallel Execution"
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
RESULTS_DIR="results/quick_parallel_${TIMESTAMP}"
mkdir -p "$RESULTS_DIR"

echo -e "${GREEN}✅ Results directory: $RESULTS_DIR${NC}"

# Build
echo ""
echo "🔨 Building release binary..."
cargo build --release --quiet 2>&1 | grep -v "warning:" || true

if [ ${PIPESTATUS[0]} -ne 0 ]; then
    echo -e "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Build complete!${NC}"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Launch All Bots in Parallel
# ─────────────────────────────────────────────────────────────────────────

echo "════════════════════════════════════════════════════════════════════════"
echo "🚀 Launching all 5 bots in parallel..."
echo "════════════════════════════════════════════════════════════════════════"
echo ""

START_TIME=$(date +%s)

# Bot 1: Balanced
echo -e "${GREEN}  1️⃣  Balanced Bot${NC}"
echo "     📊 35 levels @ 0.15% | Launching..."
nohup ./target/release/solana-grid-bot \
    --config config/production/balanced.toml \
    --duration-minutes "$DURATION" \
    > "$RESULTS_DIR/balanced.txt" 2>&1 &
PID_BALANCED=$!
echo "     ✅ PID: $PID_BALANCED"
echo ""

# Bot 2: Conservative
echo -e "${BLUE}  2️⃣  Conservative Bot${NC}"
echo "     🛡️  20 levels @ 0.25% | Launching..."
nohup ./target/release/solana-grid-bot \
    --config config/production/conservative.toml \
    --duration-minutes "$DURATION" \
    > "$RESULTS_DIR/conservative.txt" 2>&1 &
PID_CONSERVATIVE=$!
echo "     ✅ PID: $PID_CONSERVATIVE"
echo ""

# Bot 3: Aggressive
echo -e "${YELLOW}  3️⃣  Aggressive Bot (🆕)${NC}"
echo "     ⚡ 50 levels @ 0.10% | Launching..."
nohup ./target/release/solana-grid-bot \
    --config config/production/aggressive.toml \
    --duration-minutes "$DURATION" \
    > "$RESULTS_DIR/aggressive.txt" 2>&1 &
PID_AGGRESSIVE=$!
echo "     ✅ PID: $PID_AGGRESSIVE"
echo ""

# Bot 4: Ultra Aggressive
echo -e "${RED}  4️⃣  Ultra-Aggressive Bot${NC}"
echo "     🔥 60 levels @ 0.08% | Launching..."
nohup ./target/release/solana-grid-bot \
    --config config/production/ultra_aggressive.toml \
    --duration-minutes "$DURATION" \
    > "$RESULTS_DIR/ultra_aggressive.txt" 2>&1 &
PID_ULTRA=$!
echo "     ✅ PID: $PID_ULTRA"
echo ""

# Bot 5: Multi-Strategy
echo -e "${PURPLE}  5️⃣  Multi-Strategy Bot (🆕)${NC}"
echo "     🧠 30 levels @ 0.20% | Launching..."
nohup ./target/release/solana-grid-bot \
    --config config/production/multi_strategy.toml \
    --duration-minutes "$DURATION" \
    > "$RESULTS_DIR/multi_strategy.txt" 2>&1 &
PID_MULTI=$!
echo "     ✅ PID: $PID_MULTI"
echo ""

# Save PIDs
echo "$PID_BALANCED" > "$RESULTS_DIR/balanced.pid"
echo "$PID_CONSERVATIVE" > "$RESULTS_DIR/conservative.pid"
echo "$PID_AGGRESSIVE" > "$RESULTS_DIR/aggressive.pid"
echo "$PID_ULTRA" > "$RESULTS_DIR/ultra_aggressive.pid"
echo "$PID_MULTI" > "$RESULTS_DIR/multi_strategy.pid"

END_TIME=$(date -d "+${DURATION} minutes" '+%Y-%m-%d %H:%M:%S' 2>/dev/null || date -v+${DURATION}M '+%Y-%m-%d %H:%M:%S')

echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}✅ ALL 5 BOTS LAUNCHED!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo "📊 Process IDs:"
echo -e "   ${GREEN}Balanced:${NC}         $PID_BALANCED"
echo -e "   ${BLUE}Conservative:${NC}     $PID_CONSERVATIVE"
echo -e "   ${YELLOW}Aggressive:${NC}       $PID_AGGRESSIVE"
echo -e "   ${RED}Ultra-Aggressive:${NC} $PID_ULTRA"
echo -e "   ${PURPLE}Multi-Strategy:${NC}   $PID_MULTI"
echo ""
echo "⏱️  Started: $(date)"
echo "⏱️  Expected completion: $END_TIME"
echo ""
echo "📊 LIVE MONITORING:"
echo "   Watch logs:  tail -f $RESULTS_DIR/*.txt"
echo "   Check PIDs:  ps aux | grep solana-grid-bot"
echo ""
echo "🛑 STOP ALL:"
echo "   kill \$(cat $RESULTS_DIR/*.pid)"
echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${CYAN}⏳ Tests running... (${DURATION} minutes)${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Wait and Monitor
# ─────────────────────────────────────────────────────────────────────────

echo "💤 Waiting for tests to complete..."
echo "   (You can Ctrl+C to exit monitoring without stopping bots)"
echo ""

# Wait for all processes
wait $PID_BALANCED $PID_CONSERVATIVE $PID_AGGRESSIVE $PID_ULTRA $PID_MULTI 2>/dev/null

ACTUAL_END_TIME=$(date +%s)
TOTAL_TIME=$((ACTUAL_END_TIME - START_TIME))

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}🏆 PARALLEL QUICK TEST COMPLETE!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo "⏱️  Total runtime: ${TOTAL_TIME}s (~$((TOTAL_TIME / 60))min)"
echo "📋 Results saved to: $RESULTS_DIR/"
echo ""

# Check which tests passed
echo "📊 Test Results:"
success_count=0
total_count=5

for config_file in balanced conservative aggressive ultra_aggressive multi_strategy; do
    if [ -f "$RESULTS_DIR/${config_file}.txt" ]; then
        if grep -q "Session complete" "$RESULTS_DIR/${config_file}.txt" 2>/dev/null; then
            echo -e "   ${GREEN}✅ ${config_file}${NC}"
            success_count=$((success_count + 1))
        else
            echo -e "   ${RED}❌ ${config_file}${NC}"
        fi
    else
        echo -e "   ${RED}❌ ${config_file} (no output)${NC}"
    fi
done

echo ""
if [ $success_count -eq $total_count ]; then
    echo -e "${GREEN}🎉 ALL $total_count TESTS PASSED!${NC}"
    echo ""
    echo "🚀 Ready to launch Battle Royale!"
    echo "   ./scripts/launch_battle_royale.sh 20"
else
    echo -e "${YELLOW}⚠️  $success_count/$total_count tests passed${NC}"
    echo "   Check logs in: $RESULTS_DIR/"
fi

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo ""
