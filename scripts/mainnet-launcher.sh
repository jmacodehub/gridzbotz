#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🚀 GRIDZBOTZ MAINNET LAUNCHER V1.0
# 
# Production-grade single-bot launcher with safety checks
# Based on battle-tested launch_console.sh framework
# ═══════════════════════════════════════════════════════════════════════════

set -eo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
BOLD='\033[1m'
NC='\033[0m'

# Config
MAINNET_CONFIG="config/production/mainnet-sol-usdc-v1.toml"
LOG_DIR="logs/mainnet"
TIMESTAMP=$(date +%Y%m%d-%H%M%S)
LOG_FILE="$LOG_DIR/mainnet-$TIMESTAMP.log"
PID_FILE="$LOG_DIR/bot.pid"
STATUS_FILE="$LOG_DIR/status.txt"

# ═══════════════════════════════════════════════════════════════════════════
# BANNER
# ═══════════════════════════════════════════════════════════════════════════

show_banner() {
    clear
    echo -e "${CYAN}"
    cat << "BANNER"
╔════════════════════════════════════════════════════════════════════════╗
║                                                                        ║
║   🚀 GRIDZBOTZ MAINNET LAUNCHER V1.0                                  ║
║                                                                        ║
║   Production-Ready | Battle-Tested | Safety First                    ║
║                                                                        ║
╚════════════════════════════════════════════════════════════════════════╝
BANNER
    echo -e "${NC}"
}

# ═══════════════════════════════════════════════════════════════════════════
# PRE-FLIGHT CHECKS
# ═══════════════════════════════════════════════════════════════════════════

preflight_check() {
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              ✅ PRE-FLIGHT CHECKS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local checks_passed=0
    local checks_failed=0

    # Check 1: Config exists
    echo -e "${CYAN}[1/7] Checking mainnet config...${NC}"
    if [ ! -f "$MAINNET_CONFIG" ]; then
        echo -e "${RED}   ❌ Config not found: $MAINNET_CONFIG${NC}"
        ((checks_failed++))
        return 1
    fi
    echo -e "${GREEN}   ✅ Config found${NC}"
    ((checks_passed++))

    # Check 2: Wallet exists
    echo -e "${CYAN}[2/7] Checking wallet...${NC}"
    WALLET_PATH=$(grep "^wallet_path" "$MAINNET_CONFIG" | cut -d'"' -f2)
    WALLET_EXPANDED="${WALLET_PATH/#\~/$HOME}"
    if [ ! -f "$WALLET_EXPANDED" ]; then
        echo -e "${RED}   ❌ Wallet not found: $WALLET_PATH${NC}"
        ((checks_failed++))
        return 1
    fi
    echo -e "${GREEN}   ✅ Wallet found: $WALLET_PATH${NC}"
    ((checks_passed++))

    # Check 3: RPC configured
    echo -e "${CYAN}[3/7] Checking RPC endpoints...${NC}"
    if grep -q "YOUR_API_KEY" "$MAINNET_CONFIG"; then
        echo -e "${RED}   ❌ RPC URLs not configured${NC}"
        echo -e "${YELLOW}   Run: nano $MAINNET_CONFIG${NC}"
        ((checks_failed++))
        return 1
    fi
    RPC_URL=$(grep "^rpc_url" "$MAINNET_CONFIG" | cut -d'"' -f2)
    echo -e "${GREEN}   ✅ RPC configured: ${RPC_URL:0:40}...${NC}"
    ((checks_passed++))

    # Check 4: Not already running
    echo -e "${CYAN}[4/7] Checking for existing instance...${NC}"
    if [ -f "$PID_FILE" ]; then
        OLD_PID=$(cat "$PID_FILE")
        if ps -p "$OLD_PID" > /dev/null 2>&1; then
            echo -e "${RED}   ❌ Bot already running (PID: $OLD_PID)${NC}"
            echo -e "${YELLOW}   Stop it first: kill $OLD_PID${NC}"
            ((checks_failed++))
            return 1
        fi
        rm "$PID_FILE"
    fi
    echo -e "${GREEN}   ✅ No previous instance${NC}"
    ((checks_passed++))

    # Check 5: Binary exists or build
    echo -e "${CYAN}[5/7] Checking binary...${NC}"
    if [ -f "./target/release/solana-grid-bot" ]; then
        BINARY_PATH="./target/release/solana-grid-bot"
        echo -e "${GREEN}   ✅ Release binary found${NC}"
        ((checks_passed++))
    elif [ -f "./target/debug/solana-grid-bot" ]; then
        BINARY_PATH="./target/debug/solana-grid-bot"
        echo -e "${YELLOW}   ⚠️  Using debug binary (slower)${NC}"
        ((checks_passed++))
    else
        echo -e "${YELLOW}   ⚠️  No binary found, will build...${NC}"
        BINARY_PATH="./target/release/solana-grid-bot"
        ((checks_passed++))
    fi

    # Check 6: Verify execution mode
    echo -e "${CYAN}[6/7] Verifying execution mode...${NC}"
    EXEC_MODE=$(grep "^execution_mode" "$MAINNET_CONFIG" | cut -d'"' -f2)
    if [ "$EXEC_MODE" != "live" ]; then
        echo -e "${RED}   ❌ Not in live mode: $EXEC_MODE${NC}"
        echo -e "${YELLOW}   This config is for ${EXEC_MODE}, not mainnet!${NC}"
        ((checks_failed++))
        return 1
    fi
    echo -e "${GREEN}   ✅ Execution mode: ${BOLD}LIVE${NC}"
    ((checks_passed++))

    # Check 7: Verify mainnet cluster
    echo -e "${CYAN}[7/7] Verifying mainnet cluster...${NC}"
    CLUSTER=$(grep "^cluster" "$MAINNET_CONFIG" | cut -d'"' -f2)
    if [ "$CLUSTER" != "mainnet-beta" ]; then
        echo -e "${RED}   ❌ Wrong cluster: $CLUSTER${NC}"
        ((checks_failed++))
        return 1
    fi
    echo -e "${GREEN}   ✅ Cluster: ${BOLD}mainnet-beta${NC}"
    ((checks_passed++))

    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    
    if [ $checks_failed -eq 0 ]; then
        echo -e "${GREEN}🎉 ALL PRE-FLIGHT CHECKS PASSED! (${checks_passed}/${checks_passed})${NC}"
        return 0
    else
        echo -e "${RED}❌ PRE-FLIGHT FAILED (${checks_passed} passed, ${checks_failed} failed)${NC}"
        return 1
    fi
}

# ═══════════════════════════════════════════════════════════════════════════
# LAUNCH MAINNET BOT
# ═══════════════════════════════════════════════════════════════════════════

launch_mainnet() {
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              🚀 MAINNET LAUNCH${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    # Build if needed
    if [ ! -f "$BINARY_PATH" ]; then
        echo -e "${BLUE}📦 Building release binary...${NC}"
        if ! cargo build --release 2>&1 | tail -10; then
            echo -e "${RED}❌ Build failed!${NC}"
            return 1
        fi
        echo -e "${GREEN}✅ Build complete!${NC}"
        echo ""
    fi

    # Config summary
    CAPITAL=$(grep "^initial_usdc" "$MAINNET_CONFIG" | grep -oE "[0-9]+\.[0-9]+" | head -1)
    GRID_LEVELS=$(grep "^grid_levels" "$MAINNET_CONFIG" | grep -oE "[0-9]+" | head -1)
    SPACING=$(grep "^grid_spacing_percent" "$MAINNET_CONFIG" | grep -oE "[0-9]+\.[0-9]+" | head -1)
    
    echo -e "${YELLOW}📊 Configuration Summary:${NC}"
    echo -e "   Config:       $MAINNET_CONFIG"
    echo -e "   Wallet:       $WALLET_PATH"
    echo -e "   RPC:          ${RPC_URL:0:45}..."
    echo -e "   Capital:      \$$CAPITAL USDC"
    echo -e "   Grid Levels:  $GRID_LEVELS"
    echo -e "   Spacing:      $SPACING%"
    echo ""

    # FINAL CONFIRMATION
    echo -e "${RED}${BOLD}⚠️  WARNING: YOU ARE ABOUT TO LAUNCH WITH REAL MONEY! ⚠️${NC}"
    echo ""
    read -p "Type 'YES' in CAPITAL letters to confirm: " CONFIRM

    if [ "$CONFIRM" != "YES" ]; then
        echo -e "${YELLOW}Launch cancelled${NC}"
        return 1
    fi

    echo ""
    echo -e "${GREEN}🚀 LAUNCHING MAINNET BOT...${NC}"
    echo ""

    # Create log directory
    mkdir -p "$LOG_DIR"

    # Launch bot in background
    RUST_LOG=info nohup "$BINARY_PATH" --config "$MAINNET_CONFIG" > "$LOG_FILE" 2>&1 &
    BOT_PID=$!

    # Save PID
    echo "$BOT_PID" > "$PID_FILE"

    # Save status
    cat > "$STATUS_FILE" << STATUS
Started: $(date '+%Y-%m-%d %H:%M:%S')
PID: $BOT_PID
Config: $MAINNET_CONFIG
Log: $LOG_FILE
STATUS

    # Wait 5 seconds to verify startup
    echo -e "${CYAN}Waiting 5 seconds to verify startup...${NC}"
    sleep 5

    if ! ps -p $BOT_PID > /dev/null; then
        echo -e "${RED}❌ BOT FAILED TO START!${NC}"
        echo ""
        echo -e "${YELLOW}Last 20 log lines:${NC}"
        tail -20 "$LOG_FILE"
        return 1
    fi

    echo -e "${GREEN}✅ BOT STARTED SUCCESSFULLY!${NC}"
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}  MAINNET BOT IS LIVE! 🎉${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}Bot Details:${NC}"
    echo -e "   PID:      $BOT_PID"
    echo -e "   Started:  $(date)"
    echo -e "   Log:      $LOG_FILE"
    echo ""
    echo -e "${YELLOW}📊 Monitor Commands:${NC}"
    echo -e "   Live tail:    ${GREEN}tail -f $LOG_FILE${NC}"
    echo -e "   Fills only:   ${GREEN}tail -f $LOG_FILE | grep FILL_TRACK${NC}"
    echo -e "   Errors only:  ${GREEN}tail -f $LOG_FILE | grep ERROR${NC}"
    echo ""
    echo -e "${YELLOW}🛑 Stop Command:${NC}"
    echo -e "   ${GREEN}kill $BOT_PID${NC}"
    echo ""
    echo -e "${GREEN}🚀 GOOD LUCK! LET'S MAKE SOME GAINS! 💰${NC}"
    echo ""
}

# ═══════════════════════════════════════════════════════════════════════════
# STATUS & STOP FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

check_status() {
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              📊 BOT STATUS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    if [ ! -f "$PID_FILE" ]; then
        echo -e "${YELLOW}No bot running${NC}"
        return
    fi

    BOT_PID=$(cat "$PID_FILE")

    if ps -p "$BOT_PID" > /dev/null 2>&1; then
        echo -e "${GREEN}✅ Bot RUNNING${NC}"
        echo ""
        
        if [ -f "$STATUS_FILE" ]; then
            cat "$STATUS_FILE"
            echo ""
        fi

        LATEST_LOG=$(ls -t "$LOG_DIR"/mainnet-*.log 2>/dev/null | head -1)
        if [ -n "$LATEST_LOG" ]; then
            FILLS=$(grep -c "FILL_TRACK" "$LATEST_LOG" 2>/dev/null || echo 0)
            ERRORS=$(grep -c "ERROR" "$LATEST_LOG" 2>/dev/null || echo 0)
            LAST_LINE=$(tail -1 "$LATEST_LOG" | cut -c1-70)
            
            echo -e "${CYAN}Statistics:${NC}"
            echo -e "   Fills:   $FILLS"
            echo -e "   Errors:  $ERRORS"
            echo ""
            echo -e "${CYAN}Last Log:${NC}"
            echo -e "   $LAST_LINE"
        fi
    else
        echo -e "${RED}❌ Bot process DEAD (PID: $BOT_PID)${NC}"
        rm "$PID_FILE"
    fi
    echo ""
}

stop_bot() {
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              🛑 STOP BOT${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    if [ ! -f "$PID_FILE" ]; then
        echo -e "${YELLOW}No bot running${NC}"
        return
    fi

    BOT_PID=$(cat "$PID_FILE")

    if ! ps -p "$BOT_PID" > /dev/null 2>&1; then
        echo -e "${YELLOW}Bot already stopped${NC}"
        rm "$PID_FILE"
        return
    fi

    echo -e "${YELLOW}Stopping bot (PID: $BOT_PID)...${NC}"
    kill "$BOT_PID"

    # Wait up to 10 seconds
    for i in {1..10}; do
        if ! ps -p "$BOT_PID" > /dev/null 2>&1; then
            echo -e "${GREEN}✅ Bot stopped gracefully${NC}"
            rm "$PID_FILE"
            return
        fi
        sleep 1
    done

    echo -e "${YELLOW}Force killing...${NC}"
    kill -9 "$BOT_PID"
    echo -e "${GREEN}✅ Bot stopped${NC}"
    rm "$PID_FILE"
}

# ═══════════════════════════════════════════════════════════════════════════
# MAIN MENU
# ═══════════════════════════════════════════════════════════════════════════

main_menu() {
    while true; do
        show_banner
        echo ""
        echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
        echo -e "${WHITE}              MAIN MENU${NC}"
        echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
        echo ""
        echo -e "${CYAN}1${NC}  🚀 Launch Mainnet Bot"
        echo -e "${CYAN}2${NC}  ✅ Pre-Flight Check Only"
        echo -e "${CYAN}3${NC}  📊 Check Running Status"
        echo -e "${CYAN}4${NC}  🛑 Stop Bot"
        echo -e "${CYAN}5${NC}  📋 View Config"
        echo -e "${CYAN}6${NC}  📈 Tail Live Logs"
        echo -e "${CYAN}7${NC}  ❌ Exit"
        echo ""
        read -p "$(echo -e ${YELLOW}Select option [1-7]:${NC} )" choice

        case $choice in
            1)
                show_banner
                if preflight_check; then
                    launch_mainnet
                    read -p "Press Enter to continue..."
                else
                    echo ""
                    read -p "Press Enter to continue..."
                fi
                ;;
            2)
                show_banner
                preflight_check
                echo ""
                read -p "Press Enter to continue..."
                ;;
            3)
                show_banner
                check_status
                read -p "Press Enter to continue..."
                ;;
            4)
                show_banner
                stop_bot
                read -p "Press Enter to continue..."
                ;;
            5)
                show_banner
                echo ""
                echo -e "${CYAN}Config: $MAINNET_CONFIG${NC}"
                echo ""
                head -60 "$MAINNET_CONFIG"
                echo ""
                read -p "Press Enter to continue..."
                ;;
            6)
                LATEST_LOG=$(ls -t "$LOG_DIR"/mainnet-*.log 2>/dev/null | head -1)
                if [ -n "$LATEST_LOG" ]; then
                    tail -f "$LATEST_LOG"
                else
                    echo -e "${RED}No log file found${NC}"
                    sleep 2
                fi
                ;;
            7)
                echo ""
                echo -e "${GREEN}Thanks for using GridzBotz! LFG! 🚀${NC}"
                echo ""
                exit 0
                ;;
            *)
                echo -e "${RED}Invalid option${NC}"
                sleep 1
                ;;
        esac
    done
}

# ═══════════════════════════════════════════════════════════════════════════
# ENTRY POINT
# ═══════════════════════════════════════════════════════════════════════════

# If launched with --quick, skip menu and launch directly
if [ "$1" == "--quick" ]; then
    show_banner
    if preflight_check; then
        launch_mainnet
    fi
    exit $?
fi

# Otherwise show interactive menu
main_menu
