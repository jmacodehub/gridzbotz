#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════════
# 🏆 GRIDZBOTZ - ENHANCED MODULAR LAUNCH CONSOLE V2.1 FIXED
# 10 Gladiator Battle Royale with Pre-Flight & Modular Durations
# ═══════════════════════════════════════════════════════════════════════════

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

# ═══════════════════════════════════════════════════════════════════════════
# CONFIGURATION
# ═══════════════════════════════════════════════════════════════════════════

# Gladiator configs (Tier 1 + Tier 2)
declare -A GLADIATORS=(
    ["1:maxlevels"]="config/production/ultra_aggressive.toml"
    ["2:aggressive"]="config/overnight_aggressive.toml"
    ["3:balanced"]="config/overnight_balanced.toml"
    ["4:master"]="config/master.toml"
    ["5:conservative"]="config/overnight_conservative.toml"
    ["6:superagg"]="config/overnight_super_aggressive.toml"
    ["7:multistrat"]="config/overnight_multi_strategy.toml"
    ["8:ultraagg"]="config/overnight_ultra_aggressive.toml"
    ["9:prodbal"]="config/production/balanced.toml"
    ["10:prodcons"]="config/production/conservative.toml"
)

# Duration presets: "minutes:hours:label"
declare -A DURATIONS=(
    ["1"]="5:0:5min"       # 5 minutes
    ["2"]="15:0:15min"     # 15 minutes
    ["3"]="60:1:1h"        # 1 hour (use minutes to avoid decimal)
    ["4"]="0:8:8h"         # 8 hours
    ["5"]="0:12:12h"       # 12 hours
    ["6"]="0:20:20h"       # 20 hours
    ["7"]="custom:custom:custom"  # Custom duration
)

# ═══════════════════════════════════════════════════════════════════════════
# DISPLAY FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

show_banner() {
    clear
    echo -e "${CYAN}"
    cat << "EOF"
╔════════════════════════════════════════════════════════════════════════╗
║                                                                        ║
║   🏆 GRIDZBOTZ - 10 GLADIATOR BATTLE ROYALE V2.1                      ║
║                                                                        ║
║   Enhanced Modular Console | Pre-Flight Check | Fixed Duration Logic ║
║                                                                        ║
╚════════════════════════════════════════════════════════════════════════╝
EOF
    echo -e "${NC}"
}

show_main_menu() {
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              🎯 MAIN MENU${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}1${NC}  🚀 Launch ALL 10 Gladiators (with duration selection)"
    echo -e "${CYAN}2${NC}  🎮 Launch Single Config (with duration selection)"
    echo -e "${CYAN}3${NC}  ⚡ Quick Pre-Test Run (5 min validation)"
    echo -e "${CYAN}4${NC}  ✅ Pre-Flight Check (verify everything ready)"
    echo -e "${CYAN}5${NC}  📊 View Running Bots"
    echo -e "${CYAN}6${NC}  ⏹️  Stop All Bots"
    echo -e "${CYAN}7${NC}  📈 Quick Status"
    echo -e "${CYAN}8${NC}  🧹 Cleanup Old Runs"
    echo -e "${CYAN}9${NC}  ❌ Exit"
    echo ""
    echo -n -e "${YELLOW}Select option (1-9): ${NC}"
}

show_gladiators() {
    echo ""
    echo -e "${PURPLE}═══ TIER 1: PROVEN CHAMPIONS ═══${NC}"
    echo -e "${CYAN} 1${NC}  MaxLevels      👑  ${GREEN}(ultra_aggressive)${NC}"
    echo -e "${CYAN} 2${NC}  Aggressive     🔥  ${GREEN}(overnight_aggressive)${NC}"
    echo -e "${CYAN} 3${NC}  Balanced       ⚖️   ${GREEN}(overnight_balanced)${NC}"
    echo -e "${CYAN} 4${NC}  Master         🎮  ${GREEN}(master)${NC}"
    echo -e "${CYAN} 5${NC}  Conservative   🛡️   ${GREEN}(overnight_conservative)${NC}"
    echo ""
    echo -e "${PURPLE}═══ TIER 2: EXPERIMENTAL CHALLENGERS ═══${NC}"
    echo -e "${CYAN} 6${NC}  SuperAgg       ⚡  ${YELLOW}(overnight_super_aggressive)${NC}"
    echo -e "${CYAN} 7${NC}  MultiStrat     🧠  ${YELLOW}(overnight_multi_strategy)${NC}"
    echo -e "${CYAN} 8${NC}  UltraAgg       💥  ${YELLOW}(overnight_ultra_aggressive)${NC}"
    echo -e "${CYAN} 9${NC}  ProdBal        🏭  ${YELLOW}(production/balanced)${NC}"
    echo -e "${CYAN}10${NC}  ProdCons       🏰  ${YELLOW}(production/conservative)${NC}"
    echo ""
}

show_duration_menu() {
    echo ""
    echo -e "${WHITE}⏰ SELECT DURATION:${NC}"
    echo ""
    echo -e "${CYAN}1${NC}  5 minutes   ${YELLOW}(quick validation)${NC}"
    echo -e "${CYAN}2${NC}  15 minutes  ${YELLOW}(short test)${NC}"
    echo -e "${CYAN}3${NC}  1 hour      ${YELLOW}(standard test)${NC}"
    echo -e "${CYAN}4${NC}  8 hours     ${GREEN}(overnight)${NC}"
    echo -e "${CYAN}5${NC}  12 hours    ${GREEN}(extended)${NC}"
    echo -e "${CYAN}6${NC}  20 hours    ${GREEN}(battle royale)${NC}"
    echo -e "${CYAN}7${NC}  Custom      ${PURPLE}(enter hours/minutes)${NC}"
    echo ""
    echo -n -e "${YELLOW}Select duration (1-7): ${NC}"
}

# ═══════════════════════════════════════════════════════════════════════════
# UTILITY FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

check_binary() {
    if [ -f "./target/release/solana-grid-bot" ]; then
        return 0
    elif [ -f "./target/debug/solana-grid-bot" ]; then
        return 0
    else
        return 1
    fi
}

get_binary_path() {
    if [ -f "./target/release/solana-grid-bot" ]; then
        echo "./target/release/solana-grid-bot"
    elif [ -f "./target/debug/solana-grid-bot" ]; then
        echo "./target/debug/solana-grid-bot"
    else
        echo ""
    fi
}

build_binary() {
    echo -e "${BLUE}📦 Building release binary...${NC}"
    if cargo build --release 2>&1 | grep -q "error"; then
        echo -e "${RED}❌ Build failed!${NC}"
        return 1
    fi
    echo -e "${GREEN}✅ Build complete!${NC}"
    return 0
}

# NEW: Get duration as "minutes:hours:label"
get_duration_spec() {
    local choice=$1

    case $choice in
        1) echo "5:0:5min" ;;        # 5 minutes
        2) echo "15:0:15min" ;;      # 15 minutes
        3) echo "60:1:1h" ;;         # 1 hour (60 mins or 1 hour)
        4) echo "0:8:8h" ;;          # 8 hours
        5) echo "0:12:12h" ;;        # 12 hours
        6) echo "0:20:20h" ;;        # 20 hours
        7)
            echo ""
            read -p "Enter duration as (M)inutes or (H)ours? [M/H]: " unit
            if [[ "$unit" =~ ^[Hh]$ ]]; then
                read -p "Enter hours (integer): " hours
                echo "0:${hours}:${hours}h"
            else
                read -p "Enter minutes (integer): " mins
                local hours_equiv=$(echo "scale=1; $mins / 60" | bc)
                echo "${mins}:0:${mins}min"
            fi
            ;;
        *) echo "60:1:1h" ;;
    esac
}

# ═══════════════════════════════════════════════════════════════════════════
# PRE-FLIGHT CHECK
# ═══════════════════════════════════════════════════════════════════════════

run_preflight_check() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              ✅ PRE-FLIGHT CHECK${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local checks_passed=0
    local checks_failed=0

    # Check binary
    echo -e "${CYAN}Checking binary...${NC}"
    if check_binary; then
        echo -e "${GREEN}✅ Binary found: $(get_binary_path)${NC}"
        ((checks_passed++))
    else
        echo -e "${RED}❌ Binary not found - building...${NC}"
        if build_binary; then
            ((checks_passed++))
        else
            ((checks_failed++))
        fi
    fi

    # Check configs
    echo ""
    echo -e "${CYAN}Checking configs...${NC}"
    local found_configs=0
    local missing_configs=()

    for key in "${!GLADIATORS[@]}"; do
        local config="${GLADIATORS[$key]}"
        if [ -f "$config" ]; then
            ((found_configs++))
        else
            local name=$(echo "$key" | cut -d: -f2)
            missing_configs+=("$name")
        fi
    done

    if [ ${#missing_configs[@]} -eq 0 ]; then
        echo -e "${GREEN}✅ All 10 configs found${NC}"
        ((checks_passed++))
    else
        echo -e "${YELLOW}⚠️  Found $found_configs/10 configs${NC}"
        echo -e "${YELLOW}   Missing: ${missing_configs[*]}${NC}"
        ((checks_passed++))
    fi

    # Check directories
    echo ""
    echo -e "${CYAN}Checking directories...${NC}"
    for dir in logs config results; do
        if [ -d "$dir" ]; then
            echo -e "${GREEN}✅ $dir/ exists${NC}"
            ((checks_passed++))
        else
            echo -e "${YELLOW}⚠️  $dir/ missing - creating...${NC}"
            mkdir -p "$dir"
            ((checks_passed++))
        fi
    done

    # Check disk space
    echo ""
    echo -e "${CYAN}Checking disk space...${NC}"
    local free_space=$(df -h . | tail -1 | awk '{print $4}')
    echo -e "${GREEN}✅ Free space: $free_space${NC}"
    ((checks_passed++))

    # Quick compile check
    echo ""
    echo -e "${CYAN}Testing compile...${NC}"
    if cargo check --quiet 2>/dev/null; then
        echo -e "${GREEN}✅ Code compiles${NC}"
        ((checks_passed++))
    else
        echo -e "${RED}❌ Compilation issues detected${NC}"
        ((checks_failed++))
    fi

    # Summary
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    if [ $checks_failed -eq 0 ]; then
        echo -e "${GREEN}🎉 ALL CHECKS PASSED! ($checks_passed checks)${NC}"
        echo ""
        echo -e "${CYAN}You're ready to launch!${NC}"
    else
        echo -e "${YELLOW}⚠️  $checks_failed issues found${NC}"
        echo -e "${CYAN}Passed: $checks_passed | Failed: $checks_failed${NC}"
    fi
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    read -p "Press Enter to continue..."
    return $checks_failed
}

# ═══════════════════════════════════════════════════════════════════════════
# LAUNCH FUNCTIONS (FIXED!)
# ═══════════════════════════════════════════════════════════════════════════

launch_single_bot() {
    local config=$1
    local name=$2
    local duration_spec=$3  # "minutes:hours:label"
    local battle_dir=$4

    echo -e "${CYAN}[$name]${NC} Launching..."

    local binary=$(get_binary_path)
    if [ -z "$binary" ]; then
        echo -e "${RED}   ✗ Binary not found!${NC}"
        return 1
    fi

    if [ ! -f "$config" ]; then
        echo -e "${RED}   ✗ Config not found: $config${NC}"
        return 1
    fi

    # Parse duration spec
    IFS=':' read -r minutes hours label <<< "$duration_spec"

    # Launch with appropriate flag
    if [ "$minutes" != "0" ] && [ "$hours" == "0" ]; then
        # Use --duration-minutes for sub-hour durations
        RUST_LOG=info nohup "$binary" \
            --config "$config" \
            --duration-minutes "$minutes" \
            > "$battle_dir/${name}.log" 2>&1 &
    else
        # Use --duration-hours for hour-based durations
        RUST_LOG=info nohup "$binary" \
            --config "$config" \
            --duration-hours "$hours" \
            > "$battle_dir/${name}.log" 2>&1 &
    fi

    local pid=$!
    echo "$pid" > "$battle_dir/${name}.pid"

    echo -e "${GREEN}   ✓${NC} PID: $pid | Duration: $label | Log: ${name}.log"

    sleep 1
    return 0
}

launch_all_gladiators() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}       🚀 LAUNCHING ALL 10 GLADIATORS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    # Select duration
    show_duration_menu
    read dur_choice

    local duration_spec=$(get_duration_spec "$dur_choice")
    IFS=':' read -r minutes hours label <<< "$duration_spec"

    echo ""
    echo -e "${CYAN}Selected duration: ${BOLD}$label${NC}"
    echo ""
    read -p "Confirm launch all 10 bots for $label? (y/n): " confirm

    if [ "$confirm" != "y" ]; then
        echo -e "${YELLOW}Launch cancelled${NC}"
        sleep 1
        return
    fi

    # Ensure binary exists
    if ! check_binary; then
        if ! build_binary; then
            read -p "Press Enter to continue..."
            return
        fi
    fi

    # Create battle directory
    local timestamp=$(date +%Y%m%d_%H%M)
    local battle_dir="logs/battle_${timestamp}"
    mkdir -p "$battle_dir"

    echo ""
    echo -e "${CYAN}📂 Battle directory: $battle_dir${NC}"
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local count=0
    local launched=0

    # Launch each gladiator
    for key in $(echo "${!GLADIATORS[@]}" | tr ' ' '\n' | sort -n); do
        local config="${GLADIATORS[$key]}"
        local name=$(echo "$key" | cut -d: -f2)
        ((count++))

        echo -e "${PURPLE}[$count/10]${NC}"
        if launch_single_bot "$config" "$name" "$duration_spec" "$battle_dir"; then
            ((launched++))
        fi
        echo ""
    done

    # Summary
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ LAUNCH COMPLETE: $launched/10 bots started${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${YELLOW}📊 Battle Details:${NC}"
    echo -e "   Directory:  $battle_dir"
    echo -e "   Duration:   $label"
    echo -e "   Started:    $(date '+%Y-%m-%d %H:%M:%S')"
    echo ""
    echo -e "${CYAN}Monitor Commands:${NC}"
    echo "  Status:      ./scripts/launch_console.sh (Option 5)"
    echo "  Tail logs:   tail -f $battle_dir/*.log"
    echo "  Stop all:    ./scripts/launch_console.sh (Option 6)"
    echo ""

    read -p "Press Enter to continue..."
}

launch_single_config() {
    show_banner
    show_gladiators

    echo -n -e "${YELLOW}Select config (1-10, 0 to cancel): ${NC}"
    read config_choice

    if [ "$config_choice" == "0" ]; then
        return
    fi

    # Find config
    local found_key=""
    for key in "${!GLADIATORS[@]}"; do
        local num=$(echo "$key" | cut -d: -f1)
        if [ "$num" == "$config_choice" ]; then
            found_key="$key"
            break
        fi
    done

    if [ -z "$found_key" ]; then
        echo -e "${RED}Invalid selection!${NC}"
        sleep 2
        return
    fi

    local config="${GLADIATORS[$found_key]}"
    local name=$(echo "$found_key" | cut -d: -f2)

    # Select duration
    show_duration_menu
    read dur_choice

    local duration_spec=$(get_duration_spec "$dur_choice")
    IFS=':' read -r minutes hours label <<< "$duration_spec"

    echo ""
    echo -e "${CYAN}Launching: ${BOLD}$name${NC} for ${BOLD}$label${NC}"
    echo ""

    # Ensure binary
    if ! check_binary; then
        if ! build_binary; then
            read -p "Press Enter to continue..."
            return
        fi
    fi

    # Create battle directory
    local timestamp=$(date +%Y%m%d_%H%M)
    local battle_dir="logs/single_${name}_${timestamp}"
    mkdir -p "$battle_dir"

    # Launch
    if launch_single_bot "$config" "$name" "$duration_spec" "$battle_dir"; then
        echo ""
        echo -e "${GREEN}✅ Bot launched successfully!${NC}"
        echo ""
        echo -e "${CYAN}Monitor: tail -f $battle_dir/${name}.log${NC}"
    else
        echo -e "${RED}❌ Launch failed!${NC}"
    fi

    echo ""
    read -p "Press Enter to continue..."
}

quick_pretest() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}       ⚡ QUICK PRE-TEST (5 MINUTE VALIDATION)${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}This will launch 3 configs for 5 minutes to verify everything works.${NC}"
    echo ""
    echo -e "${YELLOW}Testing: Balanced, Aggressive, Conservative${NC}"
    echo ""
    read -p "Start pre-test? (y/n): " confirm

    if [ "$confirm" != "y" ]; then
        return
    fi

    # Ensure binary
    if ! check_binary; then
        if ! build_binary; then
            read -p "Press Enter to continue..."
            return
        fi
    fi

    # Create test directory
    local timestamp=$(date +%Y%m%d_%H%M)
    local test_dir="logs/pretest_${timestamp}"
    mkdir -p "$test_dir"

    echo ""
    echo -e "${CYAN}📂 Test directory: $test_dir${NC}"
    echo ""

    # Launch 3 test bots - FIXED with proper duration spec
    local duration_spec="5:0:5min"

    launch_single_bot "${GLADIATORS["3:balanced"]}" "balanced" "$duration_spec" "$test_dir"
    echo ""
    launch_single_bot "${GLADIATORS["2:aggressive"]}" "aggressive" "$duration_spec" "$test_dir"
    echo ""
    launch_single_bot "${GLADIATORS["5:conservative"]}" "conservative" "$duration_spec" "$test_dir"
    echo ""

    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}✅ Pre-test launched! Running for 5 minutes...${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "${CYAN}Monitor: tail -f $test_dir/*.log${NC}"
    echo ""

    read -p "Press Enter to continue..."
}

# ═══════════════════════════════════════════════════════════════════════════
# MONITOR & CONTROL FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

view_running_bots() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              📊 RUNNING BOTS STATUS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local battle_dir=$(ls -td logs/battle_* logs/single_* logs/pretest_* 2>/dev/null | head -1)

    if [ -z "$battle_dir" ]; then
        echo -e "${YELLOW}No active battles found.${NC}"
        echo ""
        read -p "Press Enter to continue..."
        return
    fi

    echo -e "${CYAN}Battle directory:${NC} $battle_dir"
    echo ""

    local running=0
    local total=0
    local completed=0

    for pidfile in "$battle_dir"/*.pid; do
        if [ -f "$pidfile" ]; then
            local pid=$(cat "$pidfile")
            local name=$(basename "$pidfile" .pid)
            local logfile="$battle_dir/${name}.log"
            ((total++))

            if ps -p $pid > /dev/null 2>&1; then
                local last_line=$(tail -1 "$logfile" 2>/dev/null | cut -c1-70)
                local cycles=$(grep -c "Cycle" "$logfile" 2>/dev/null || echo "0")

                echo -e "${GREEN}✓${NC} ${BOLD}$name${NC} (PID: $pid)"
                echo -e "   Cycles: $cycles"
                echo -e "   Last: $last_line"
                ((running++))
            else
                if grep -q "SESSION COMPLETE" "$logfile" 2>/dev/null; then
                    echo -e "${BLUE}✓${NC} ${BOLD}$name${NC} - ${GREEN}COMPLETED${NC}"
                    ((completed++))
                else
                    echo -e "${RED}✗${NC} ${BOLD}$name${NC} - ${RED}STOPPED${NC}"
                fi
            fi
            echo ""
        fi
    done

    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}Summary:${NC}"
    echo -e "  Total bots:     $total"
    echo -e "  Running:        ${GREEN}$running${NC}"
    echo -e "  Completed:      ${BLUE}$completed${NC}"
    echo -e "  Stopped/Failed: ${RED}$((total - running - completed))${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    read -p "Press Enter to continue..."
}

stop_all_bots() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              ⏹️  STOP ALL BOTS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local battle_dir=$(ls -td logs/battle_* logs/single_* logs/pretest_* 2>/dev/null | head -1)

    if [ -z "$battle_dir" ]; then
        echo -e "${YELLOW}No active battles found.${NC}"
        read -p "Press Enter to continue..."
        return
    fi

    echo -e "${YELLOW}Found battle: $battle_dir${NC}"
    echo ""

    local running=0
    for pidfile in "$battle_dir"/*.pid; do
        if [ -f "$pidfile" ]; then
            local pid=$(cat "$pidfile")
            if ps -p $pid > /dev/null 2>&1; then
                ((running++))
            fi
        fi
    done

    if [ $running -eq 0 ]; then
        echo -e "${YELLOW}No bots currently running.${NC}"
        echo ""
        read -p "Press Enter to continue..."
        return
    fi

    echo -e "${YELLOW}Found $running running bot(s)${NC}"
    echo ""
    read -p "Stop all running bots? (y/n): " confirm

    if [ "$confirm" != "y" ]; then
        echo -e "${YELLOW}Cancelled${NC}"
        sleep 1
        return
    fi

    echo ""
    for pidfile in "$battle_dir"/*.pid; do
        if [ -f "$pidfile" ]; then
            local pid=$(cat "$pidfile")
            local name=$(basename "$pidfile" .pid)

            if ps -p $pid > /dev/null 2>&1; then
                echo -e "${BLUE}Stopping $name (PID: $pid)...${NC}"
                kill $pid
                sleep 0.5
            fi
        fi
    done

    echo ""
    echo -e "${GREEN}✅ All bots stopped!${NC}"
    echo ""

    read -p "Press Enter to continue..."
}

quick_status() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              ⚡ QUICK STATUS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    local battle_dir=$(ls -td logs/battle_* logs/single_* logs/pretest_* 2>/dev/null | head -1)

    if [ -z "$battle_dir" ]; then
        echo -e "${YELLOW}No battles found${NC}"
    else
        echo -e "${CYAN}Latest battle:${NC} $battle_dir"

        local running=0
        local total=0

        for pidfile in "$battle_dir"/*.pid; do
            if [ -f "$pidfile" ]; then
                ((total++))
                local pid=$(cat "$pidfile")
                if ps -p $pid > /dev/null 2>&1; then
                    ((running++))
                fi
            fi
        done

        echo -e "${CYAN}Bots:${NC} $running/$total running"

        if [ $running -gt 0 ]; then
            echo -e "${CYAN}Status:${NC} ${GREEN}ACTIVE${NC}"
        elif [ $total -gt 0 ]; then
            echo -e "${CYAN}Status:${NC} ${BLUE}COMPLETED${NC}"
        else
            echo -e "${CYAN}Status:${NC} ${YELLOW}IDLE${NC}"
        fi
    fi

    echo ""
    read -p "Press Enter to continue..."
}

cleanup_old_runs() {
    show_banner
    echo ""
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${WHITE}              🧹 CLEANUP OLD RUNS${NC}"
    echo -e "${WHITE}═══════════════════════════════════════════════════════════════════════${NC}"
    echo ""

    echo -e "${CYAN}Scanning for old battles...${NC}"
    echo ""

    local count=0

    for dir in logs/battle_* logs/single_* logs/pretest_*; do
        if [ -d "$dir" ]; then
            local has_running=0

            for pidfile in "$dir"/*.pid; do
                if [ -f "$pidfile" ]; then
                    local pid=$(cat "$pidfile")
                    if ps -p $pid > /dev/null 2>&1; then
                        has_running=1
                        break
                    fi
                fi
            done

            if [ $has_running -eq 0 ]; then
                local dir_size=$(du -sh "$dir" 2>/dev/null | cut -f1)
                echo -e "${YELLOW}$dir${NC} ($dir_size)"
                ((count++))
            fi
        fi
    done

    if [ $count -eq 0 ]; then
        echo -e "${GREEN}No old runs to clean up!${NC}"
        echo ""
        read -p "Press Enter to continue..."
        return
    fi

    echo ""
    echo -e "${YELLOW}Found $count old run(s)${NC}"
    echo ""
    read -p "Archive these to logs/archive/? (y/n): " confirm

    if [ "$confirm" != "y" ]; then
        echo -e "${YELLOW}Cancelled${NC}"
        sleep 1
        return
    fi

    mkdir -p logs/archive

    echo ""
    for dir in logs/battle_* logs/single_* logs/pretest_*; do
        if [ -d "$dir" ]; then
            local has_running=0

            for pidfile in "$dir"/*.pid; do
                if [ -f "$pidfile" ]; then
                    local pid=$(cat "$pidfile")
                    if ps -p $pid > /dev/null 2>&1; then
                        has_running=1
                        break
                    fi
                fi
            done

            if [ $has_running -eq 0 ]; then
                echo -e "${BLUE}Archiving $(basename "$dir")...${NC}"
                mv "$dir" logs/archive/
            fi
        fi
    done

    echo ""
    echo -e "${GREEN}✅ Cleanup complete!${NC}"
    echo ""

    read -p "Press Enter to continue..."
}

# ═══════════════════════════════════════════════════════════════════════════
# MAIN LOOP
# ═══════════════════════════════════════════════════════════════════════════

while true; do
    show_banner
    show_main_menu
    read choice

    case $choice in
        1) launch_all_gladiators ;;
        2) launch_single_config ;;
        3) quick_pretest ;;
        4) run_preflight_check ;;
        5) view_running_bots ;;
        6) stop_all_bots ;;
        7) quick_status ;;
        8) cleanup_old_runs ;;
        9)
            echo ""
            echo -e "${GREEN}Thanks for using GridzBotz! LFG! 🚀${NC}"
            echo ""
            exit 0
            ;;
        *)
            echo -e "${RED}Invalid option!${NC}"
            sleep 1
            ;;
    esac
done
