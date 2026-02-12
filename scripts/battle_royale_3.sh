#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🔥🏆 BATTLE ROYALE #3 - OPTIMIZED TOP 3 SHOWDOWN! 🏆🔥
# 
# Duration: 10 hours
# Configs: 3 optimized v4.0 versions
# Goal: Crown the ULTIMATE winner for $200 mainnet deploy!
# 
# Expected Winner: Multi-Strategy v4.0 "Conservative AI" 🧠💎
# ═══════════════════════════════════════════════════════════════════════════

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔥🏆 BATTLE ROYALE #3 - OPTIMIZED TOP 3 SHOWDOWN! 🏆🔥"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

# Check if optimized configs exist
if [ ! -f "config/optimized/conservative_v4.toml" ]; then
    echo "${RED}❌ Error: Optimized configs not found!${NC}"
    echo "Please create config/optimized/ directory first"
    exit 1
fi

# Build
echo "${CYAN}🔧 Building optimized bot...${NC}"
cargo build --release

if [ $? -ne 0 ]; then
    echo "${RED}❌ Build failed!${NC}"
    exit 1
fi

echo "${GREEN}✅ Build successful!${NC}"
echo ""

# Create logs directory
mkdir -p logs/battle_royale_3

# Session ID
SESSION_ID=$(date +"%Y%m%d_%H%M%S")

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🚀 STARTING BATTLE ROYALE #3"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "📋 Configuration:"
echo "   • Duration: 10 hours"
echo "   • Session ID: $SESSION_ID"
echo "   • Contestants: 3 optimized configs"
echo ""
echo "🏆 CONTESTANTS:"
echo "   1. 🛡️  Conservative v4.0 (defending champion)"
echo "   2. 🧠 Multi-Strategy v4.0 'Conservative AI' (expected winner)"
echo "   3. ⚖️  Balanced v4.0 (dark horse)"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

# Function to run a single config
run_config() {
    local config_name=$1
    local config_path=$2
    local emoji=$3
    
    echo ""
    echo "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo "${emoji} Starting: $config_name"
    echo "${PURPLE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    
    ./target/release/solana-grid-bot --config "$config_path" \
        --duration-hours 10 \
        --dry-run 2>&1 | tee "logs/battle_royale_3/${config_name}_${SESSION_ID}.log"
    
    local exit_code=$?
    
    if [ $exit_code -eq 0 ]; then
        echo ""
        echo "${GREEN}✅ $config_name completed successfully!${NC}"
    else
        echo ""
        echo "${RED}❌ $config_name failed with exit code: $exit_code${NC}"
    fi
    
    echo ""
    echo "${BLUE}📊 Results saved to: logs/battle_royale_3/${config_name}_${SESSION_ID}.log${NC}"
    echo ""
    
    return $exit_code
}

# Track results
declare -A results

# Run each config sequentially
echo "${YELLOW}⏱️  Starting 10-hour battle royale...${NC}"
echo ""

# 1. Conservative v4.0
run_config "conservative_v4" "config/optimized/conservative_v4.toml" "🛡️"
results[conservative]=$?

# 2. Multi-Strategy v4.0
run_config "multi_strategy_v4" "config/optimized/multi_strategy_v4_conservative_ai.toml" "🧠"
results[multi_strategy]=$?

# 3. Balanced v4.0
run_config "balanced_v4" "config/optimized/balanced_v4.toml" "⚖️"
results[balanced]=$?

# Summary
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🏁 BATTLE ROYALE #3 COMPLETE!"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "📊 RESULTS SUMMARY:"
echo ""

if [ ${results[conservative]} -eq 0 ]; then
    echo "   🛡️  Conservative v4.0:       ${GREEN}✅ SUCCESS${NC}"
else
    echo "   🛡️  Conservative v4.0:       ${RED}❌ FAILED${NC}"
fi

if [ ${results[multi_strategy]} -eq 0 ]; then
    echo "   🧠 Multi-Strategy v4.0:     ${GREEN}✅ SUCCESS${NC}"
else
    echo "   🧠 Multi-Strategy v4.0:     ${RED}❌ FAILED${NC}"
fi

if [ ${results[balanced]} -eq 0 ]; then
    echo "   ⚖️  Balanced v4.0:           ${GREEN}✅ SUCCESS${NC}"
else
    echo "   ⚖️  Balanced v4.0:           ${RED}❌ FAILED${NC}"
fi

echo ""
echo "📁 All logs saved in: logs/battle_royale_3/"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔥 NEXT STEPS"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "1. Analyze results with Claude/Perplexity"
echo "2. Crown the winner"
echo "3. Deploy to mainnet with \$200"
echo "4. LFG! 🚀💎🔥"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
