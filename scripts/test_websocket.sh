#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# PROJECT FLASH - WEBSOCKET TEST SCRIPT
# ═══════════════════════════════════════════════════════════════════════════

set -e

echo "🚀 Project Flash - WebSocket Test Suite"
echo "═══════════════════════════════════════════════════════════"
echo

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Step 1: Clean build
echo -e "${CYAN}📦 Step 1: Clean build${NC}"
cargo clean
echo

# Step 2: Build with websockets
echo -e "${CYAN}🔨 Step 2: Building with WebSocket support${NC}"
cargo build --features websockets --release
echo

# Step 3: Run debug tool
echo -e "${CYAN}🔍 Step 3: Running WebSocket debug tool${NC}"
echo
cargo run --example debug_websocket --features websockets --release
echo

# Step 4: Run paper trading demo
echo -e "${CYAN}📝 Step 4: Running paper trading demo${NC}"
echo
cargo run --example paper_trading_demo --features websockets,paper-trading --release
echo

echo -e "${GREEN}✅ All tests completed successfully!${NC}"
echo
echo "Next steps:"
echo "  1. Integrate WebSocket feed into your main bot"
echo "  2. Add dynamic ATR grid repositioning"
echo "  3. Enable multi-strategy consensus voting"
echo "  4. Deploy to paper trading for 24h testing"
echo
