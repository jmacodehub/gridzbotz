#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 🎮 PROJECT FLASH V3.5 - INTERACTIVE LAUNCH CONSOLE
# Production-grade bot launcher with beautiful TUI
# ═══════════════════════════════════════════════════════════════════════════

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
WHITE='\033[1;37m'
NC='\033[0m'

# ═══════════════════════════════════════════════════════════════════════════
# FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

show_banner() {
    clear
    echo -e "${CYAN}"
    cat << "EOF"
╔════════════════════════════════════════════════════════════════════════╗
║                                                                        ║
║   🚀 PROJECT FLASH V3.5 - INTERACTIVE LAUNCH CONSOLE                   ║
║                                                                        ║
║   Master Trading Bot Suite | Production Grade | 10Hz Performance      ║
║                                                                        ║
╚════════════════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
}

show_menu() {
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}  MAIN MENU${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  ${CYAN}1)${NC} 🚀 Quick Launch - Run All Configs (8 hours)"
    echo -e "  ${CYAN}2)${NC} 🎯 Select Single Config"
    echo -e "  ${CYAN}3)${NC} 🎨 Custom Multi-Bot Launch"
    echo -e "  ${CYAN}4)${NC} ⚡ Quick Test (5 minutes)"
    echo -e "  ${CYAN}5)${NC} 📊 View Running Bots"
    echo -e "  ${CYAN}6)${NC} 🛑 Stop All Bots"
    echo -e "  ${CYAN}7)${NC} 📈 Analyze Results"
    echo -e "  ${CYAN}8)${NC} ❌ Exit"
    echo ""
    echo -n -e "${YELLOW}Select option:${NC} "
}

list_configs() {
    echo ""
    echo -e "${WHITE}Available Configurations:${NC}"
    echo ""
    
    local i=1
    for config in config/overnight_*.toml; do
        if [ -f "$config" ]; then
            local name=$(basename "$config" .toml | sed 's/overnight_//')
            echo -e "  ${CYAN}$i)${NC} $name"
            ((i++))
        fi
    done
    
    echo -e "  ${CYAN}0)${NC} Back to main menu"
    echo ""
}

get_config_details() {
    local config=$1
    local spacing=$(grep "grid_spacing_percent" "$config" | awk '{print $3}')
    local levels=$(grep "^grid_levels" "$config" | awk '{print $3}')
    local regime=$(grep "enable_regime_gate" "$config" | awk '{print $3}')
    
    echo "Spacing: ${spacing} | Levels: ${levels} | Regime Gate: ${regime}"
}

launch_single_bot() {
    local config=$1
    local duration=$2
    local name=$(basename "$config" .toml | sed 's/overnight_//')
    
    echo ""
    echo -e "${BLUE}Launching ${name}...${NC}"
    
    # Build if needed
    cargo build --release --quiet 2>/dev/null
    
    # Create results dir
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    RESULTS_DIR="results/test_${TIMESTAMP}"
    mkdir -p "$RESULTS_DIR"
    
    # Launch
    nohup ./target/release/solana-grid-bot \
        --config "$config" \
        --duration-minutes "$duration" \
        > "$RESULTS_DIR/${name}.txt" 2>&1 &
    
    PID=$!
    echo "$PID" > "$RESULTS_DIR/${name}.pid"
    
    echo -e "${GREEN}✅ Bot launched!${NC}"
    echo -e "   PID: $PID"
    echo -e "   Duration: ${duration} minutes"
    echo -e "   Output: $RESULTS_DIR/${name}.txt"
    echo ""
    echo -e "${YELLOW}Monitor with:${NC} tail -f $RESULTS_DIR/${name}.txt"
    echo ""
}

quick_launch_all() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}  🚀 QUICK LAUNCH - ALL CONFIGS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    
    # Build
    echo -e "${BLUE}Building release binary...${NC}"
    cargo build --release
    
    if [ $? -ne 0 ]; then
        echo -e "${RED}❌ Build failed!${NC}"
        read -p "Press Enter to continue..."
        return
    fi
    
    echo -e "${GREEN}✅ Build complete${NC}"
    echo ""
    
    # Create results dir
    TIMESTAMP=$(date +%Y%m%d_%H%M%S)
    RESULTS_DIR="results/overnight_${TIMESTAMP}"
    mkdir -p "$RESULTS_DIR" logs
    
    # Launch each config
    declare -a PIDS
    declare -a NAMES
    
    for config in config/overnight_*.toml; do
        if [ -f "$config" ]; then
            name=$(basename "$config" .toml | sed 's/overnight_//')
            NAMES+=("$name")
            
            echo -e "${CYAN}Launching ${name}...${NC}"
            
            nohup ./target/release/solana-grid-bot \
                --config "$config" \
                > "$RESULTS_DIR/${name}.txt" 2>&1 &
            
            PID=$!
            PIDS+=("$PID")
            echo "$PID" > "$RESULTS_DIR/${name}.pid"
            echo -e "   ✅ PID: $PID"
            
            sleep 2
        fi
    done
    
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ All bots launched!${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${YELLOW}Results directory:${NC} $RESULTS_DIR"
    echo ""
    echo -e "${YELLOW}Monitor:${NC} ./scripts/monitor_suite.sh"
    echo -e "${YELLOW}Stop all:${NC} kill \$(cat $RESULTS_DIR/*.pid)"
    echo ""
    
    read -p "Press Enter to continue..."
}

select_single_config() {
    while true; do
        show_banner
        list_configs
        
        read -p "Select config (0 to go back): " choice
        
        if [ "$choice" == "0" ]; then
            return
        fi
        
        # Get selected config
        config=$(ls config/overnight_*.toml 2>/dev/null | sed -n "${choice}p")
        
        if [ -z "$config" ]; then
            echo -e "${RED}Invalid choice!${NC}"
            sleep 2
            continue
        fi
        
        # Get duration
        echo ""
        echo -e "${WHITE}Duration options:${NC}"
        echo "  1) 5 minutes (quick test)"
        echo "  2) 15 minutes"
        echo "  3) 30 minutes"
        echo "  4) 1 hour"
        echo "  5) 2 hours"
        echo "  6) 4 hours"
        echo "  7) 8 hours (overnight)"
        echo "  8) Custom"
        echo ""
        read -p "Select duration: " dur_choice
        
        case $dur_choice in
            1) duration=5 ;;
            2) duration=15 ;;
            3) duration=30 ;;
            4) duration=60 ;;
            5) duration=120 ;;
            6) duration=240 ;;
            7) duration=480 ;;
            8) 
                read -p "Enter duration in minutes: " duration
                ;;
            *)
                echo -e "${RED}Invalid choice!${NC}"
                sleep 2
                continue
                ;;
        esac
        
        launch_single_bot "$config" "$duration"
        read -p "Press Enter to continue..."
        return
    done
}

quick_test() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}  ⚡ QUICK TEST MODE (5 minutes)${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    
    list_configs
    read -p "Select config for quick test: " choice
    
    config=$(ls config/overnight_*.toml 2>/dev/null | sed -n "${choice}p")
    
    if [ -z "$config" ]; then
        echo -e "${RED}Invalid choice!${NC}"
        sleep 2
        return
    fi
    
    launch_single_bot "$config" 5
    read -p "Press Enter to continue..."
}

view_running() {
    show_banner
    
    if command -v watch &> /dev/null; then
        watch -n 2 ./scripts/monitor_suite.sh
    else
        while true; do
            ./scripts/monitor_suite.sh
            echo ""
            echo "Press Ctrl+C to exit..."
            sleep 5
        done
    fi
}

stop_all() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}  🛑 STOP ALL BOTS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    
    RESULTS_DIR=$(ls -td results/*/2>/dev/null | head -1)
    
    if [ -z "$RESULTS_DIR" ]; then
        echo -e "${YELLOW}No running bots found${NC}"
        read -p "Press Enter to continue..."
        return
    fi
    
    echo -e "${YELLOW}Found running suite in:${NC} $RESULTS_DIR"
    echo ""
    read -p "Stop all bots? (y/n): " confirm
    
    if [ "$confirm" == "y" ]; then
        for pid_file in $RESULTS_DIR/*.pid; do
            if [ -f "$pid_file" ]; then
                PID=$(cat "$pid_file")
                if ps -p $PID > /dev/null 2>&1; then
                    echo -e "${BLUE}Stopping PID $PID...${NC}"
                    kill $PID
                fi
            fi
        done
        
        echo ""
        echo -e "${GREEN}✅ All bots stopped${NC}"
    fi
    
    echo ""
    read -p "Press Enter to continue..."
}

# ═══════════════════════════════════════════════════════════════════════════
# MAIN LOOP
# ═══════════════════════════════════════════════════════════════════════════

while true; do
    show_banner
    show_menu
    read choice
    
    case $choice in
        1) quick_launch_all ;;
        2) select_single_config ;;
        3) echo "Feature coming soon!"; sleep 2 ;;
        4) quick_test ;;
        5) view_running ;;
        6) stop_all ;;
        7) ./scripts/analyze_results.sh 2>/dev/null || echo "Analyzer not yet created"; sleep 2 ;;
        8) 
            echo ""
            echo -e "${GREEN}Thanks for using Project Flash! LFG! 🚀${NC}"
            echo ""
            exit 0
            ;;
        *) 
            echo -e "${RED}Invalid option!${NC}"
            sleep 1
            ;;
    esac
done
