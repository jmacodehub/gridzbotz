#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🔥🏆 BATTLE ROYALE #3 - PARALLEL SHOWDOWN! 🏆🔥
# 
# Duration: 10 hours (ALL BOTS RUN SIMULTANEOUSLY!)
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
echo "🔥🏆 BATTLE ROYALE #3 - PARALLEL SHOWDOWN! 🏆🔥"
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
echo "🚀 STARTING PARALLEL BATTLE ROYALE #3"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "📋 Configuration:"
echo "   • Duration: 10 hours"
echo "   • Session ID: $SESSION_ID"
echo "   • Mode: PARALLEL (all bots run simultaneously!)"
echo "   • Contestants: 3 optimized configs"
echo ""
echo "🏆 CONTESTANTS:"
echo "   1. 🛡️  Conservative v4.0 (defending champion)"
echo "   2. 🧠 Multi-Strategy v4.0 'Conservative AI' (expected winner)"
echo "   3. ⚖️  Balanced v4.0 (dark horse)"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

# Array to store PIDs
PIDS=()

# Start Conservative v4.0 in background
echo "${PURPLE}🛡️  Launching Conservative v4.0...${NC}"
./target/release/solana-grid-bot --config "config/optimized/conservative_v4.toml" \
    --duration-hours 10 \
    --dry-run \
    > "logs/battle_royale_3/conservative_v4_${SESSION_ID}.log" 2>&1 &
PID_CONSERVATIVE=$!
PIDS+=($PID_CONSERVATIVE)
echo "   PID: $PID_CONSERVATIVE"
echo ""

sleep 2

# Start Multi-Strategy v4.0 in background
echo "${PURPLE}🧠 Launching Multi-Strategy v4.0...${NC}"
./target/release/solana-grid-bot --config "config/optimized/multi_strategy_v4_conservative_ai.toml" \
    --duration-hours 10 \
    --dry-run \
    > "logs/battle_royale_3/multi_strategy_v4_${SESSION_ID}.log" 2>&1 &
PID_MULTI=$!
PIDS+=($PID_MULTI)
echo "   PID: $PID_MULTI"
echo ""

sleep 2

# Start Balanced v4.0 in background
echo "${PURPLE}⚖️  Launching Balanced v4.0...${NC}"
./target/release/solana-grid-bot --config "config/optimized/balanced_v4.toml" \
    --duration-hours 10 \
    --dry-run \
    > "logs/battle_royale_3/balanced_v4_${SESSION_ID}.log" 2>&1 &
PID_BALANCED=$!
PIDS+=($PID_BALANCED)
echo "   PID: $PID_BALANCED"
echo ""

echo "════════════════════════════════════════════════════════════════════════════════"
echo "🎮 ALL 3 BOTS LAUNCHED!"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "${GREEN}✅ Conservative v4.0:     PID $PID_CONSERVATIVE${NC}"
echo "${GREEN}✅ Multi-Strategy v4.0:   PID $PID_MULTI${NC}"
echo "${GREEN}✅ Balanced v4.0:         PID $PID_BALANCED${NC}"
echo ""
echo "📋 Logs:"
echo "   • Conservative:   logs/battle_royale_3/conservative_v4_${SESSION_ID}.log"
echo "   • Multi-Strategy: logs/battle_royale_3/multi_strategy_v4_${SESSION_ID}.log"
echo "   • Balanced:       logs/battle_royale_3/balanced_v4_${SESSION_ID}.log"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "${YELLOW}⏱️  Running for 10 hours...${NC}"
echo ""
echo "👁️  Monitor progress:"
echo "   tail -f logs/battle_royale_3/conservative_v4_${SESSION_ID}.log"
echo "   tail -f logs/battle_royale_3/multi_strategy_v4_${SESSION_ID}.log"
echo "   tail -f logs/battle_royale_3/balanced_v4_${SESSION_ID}.log"
echo ""
echo "🚫 Stop all bots:"
echo "   kill $PID_CONSERVATIVE $PID_MULTI $PID_BALANCED"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

# Wait for all processes
echo "${CYAN}⏳ Waiting for all bots to complete...${NC}"
echo ""

# Track which bots finished
FINISHED_COUNT=0
TOTAL_BOTS=3

# Wait for each PID
for pid in "${PIDS[@]}"; do
    wait $pid
    EXIT_CODE=$?
    FINISHED_COUNT=$((FINISHED_COUNT + 1))
    
    if [ $EXIT_CODE -eq 0 ]; then
        echo "${GREEN}✅ Bot (PID $pid) completed successfully! ($FINISHED_COUNT/$TOTAL_BOTS)${NC}"
    else
        echo "${RED}❌ Bot (PID $pid) failed with exit code: $EXIT_CODE ($FINISHED_COUNT/$TOTAL_BOTS)${NC}"
    fi
done

echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🏁 BATTLE ROYALE #3 COMPLETE!"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "📋 RESULTS LOCATION:"
echo "   logs/battle_royale_3/"
echo ""
echo "📊 ANALYZE RESULTS:"
echo ""
echo "   # View Conservative results"
echo "   cat logs/battle_royale_3/conservative_v4_${SESSION_ID}.log | grep -A 20 'FINAL PERFORMANCE'"
echo ""
echo "   # View Multi-Strategy results"
echo "   cat logs/battle_royale_3/multi_strategy_v4_${SESSION_ID}.log | grep -A 20 'FINAL PERFORMANCE'"
echo ""
echo "   # View Balanced results"
echo "   cat logs/battle_royale_3/balanced_v4_${SESSION_ID}.log | grep -A 20 'FINAL PERFORMANCE'"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo "🔥 NEXT STEPS"
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""
echo "1. Compare ROI, trade count, and stability"
echo "2. Crown the winner! 👑"
echo "3. Deploy to mainnet with \$200 💰"
echo "4. Integrate MEV protection 🛡️"
echo "5. LFG! 🚀💎🔥"
echo ""
echo "════════════════════════════════════════════════════════════════════════════════"
echo ""

echo "${GREEN}🏆 Battle Royale #3 completed! Check logs for results.${NC}"
echo ""
