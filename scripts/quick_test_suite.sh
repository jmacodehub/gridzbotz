#!/bin/bash

# ═════════════════════════════════════════════════════════════════════════
# 🚀 QUICK TEST SUITE - 5 Production Configs Validator
# Tests all configs with short duration for validation
# Perfect for pre-battle testing!
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

# Duration (default 5 minutes)
DURATION=${1:-5}

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  🚀 QUICK TEST SUITE - Production Config Validator"
echo "  Testing 5 Configs | ${DURATION} Minutes Each"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# Pre-flight Checks
# ─────────────────────────────────────────────────────────────────────────

echo "🔍 Pre-flight checks..."

# Check configs
CONFIGS=(
    "config/production/balanced.toml:Balanced:🔵"
    "config/production/conservative.toml:Conservative:🛡️"
    "config/production/aggressive.toml:Aggressive:⚡"
    "config/production/ultra_aggressive.toml:Ultra-Aggressive:🔥"
    "config/production/multi_strategy.toml:Multi-Strategy:🧠"
)

for config_info in "${CONFIGS[@]}"; do
    IFS=':' read -r config name emoji <<< "$config_info"
    if [ ! -f "$config" ]; then
        echo -e "${RED}❌ Missing config: $config${NC}"
        exit 1
    fi
done

echo -e "${GREEN}✅ All 5 configs present${NC}"

# Create directories
mkdir -p logs results
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
RESULTS_DIR="results/quick_test_${TIMESTAMP}"
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
# Sequential Testing
# ─────────────────────────────────────────────────────────────────────────

echo "════════════════════════════════════════════════════════════════════════"
echo "🚀 Starting sequential tests (${DURATION}min each)..."
echo "════════════════════════════════════════════════════════════════════════"
echo ""

test_count=0
success_count=0

for config_info in "${CONFIGS[@]}"; do
    IFS=':' read -r config name emoji <<< "$config_info"
    test_count=$((test_count + 1))
    
    echo -e "${CYAN}╭───────────────────────────────────────────────────────────────────╮${NC}"
    echo -e "${CYAN}│${NC} ${emoji} ${YELLOW}TEST ${test_count}/5: $name${NC}"
    echo -e "${CYAN}│${NC} Config: $config"
    echo -e "${CYAN}│${NC} Duration: ${DURATION} minutes"
    echo -e "${CYAN}╰───────────────────────────────────────────────────────────────────╯${NC}"
    echo ""
    
    # Clean name for files
    clean_name=$(echo "$name" | tr '[:upper:]' '[:lower:]' | tr ' -' '_')
    
    # Run test
    echo "🏃 Running..."
    START_TIME=$(date +%s)
    
    ./target/release/solana-grid-bot \
        --config "$config" \
        --duration-minutes "$DURATION" \
        > "$RESULTS_DIR/${clean_name}.txt" 2>&1
    
    EXIT_CODE=$?
    END_TIME=$(date +%s)
    ELAPSED=$((END_TIME - START_TIME))
    
    if [ $EXIT_CODE -eq 0 ]; then
        echo -e "${GREEN}✅ PASSED${NC} (${ELAPSED}s)"
        success_count=$((success_count + 1))
    else
        echo -e "${RED}❌ FAILED${NC} (exit code: $EXIT_CODE)"
    fi
    
    echo ""
    sleep 1
done

# ─────────────────────────────────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────────────────────────────────

echo "════════════════════════════════════════════════════════════════════════"
echo -e "${GREEN}🏆 QUICK TEST SUITE COMPLETE!${NC}"
echo "════════════════════════════════════════════════════════════════════════"
echo ""
echo "📊 Results: $success_count/$test_count passed"
echo "📋 Results saved to: $RESULTS_DIR/"
echo ""

if [ $success_count -eq $test_count ]; then
    echo -e "${GREEN}🎉 ALL TESTS PASSED! Ready for Battle Royale! 🚀${NC}"
    echo ""
    echo "Next step:"
    echo "  ./scripts/launch_battle_royale.sh 20"
else
    echo -e "${YELLOW}⚠️  Some tests failed. Check logs in $RESULTS_DIR/${NC}"
fi

echo ""
echo "════════════════════════════════════════════════════════════════════════"
