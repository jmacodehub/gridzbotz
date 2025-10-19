#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# 💎 PROJECT FLASH V3.5 - FLEXIBLE MULTI-BOT LAUNCH SUITE (FIXED) 💎
# ═══════════════════════════════════════════════════════════════════════════
#
# FIX: Changed --duration to --duration-hours (matches your bot's CLI)
# Version: 3.5.1 (FIXED)
# Created: 2025-10-19
#
# ═══════════════════════════════════════════════════════════════════════════

set -e  # Exit on any error

# ═══════════════════════════════════════════════════════════════════════════
# 🎯 CONFIGURATION & DEFAULTS
# ═══════════════════════════════════════════════════════════════════════════

# Default values
DEFAULT_DURATION="24h"
DEFAULT_CONFIG_DIR="config/production/v3.5_preproduction"
PROJECT_NAME="project_flash"

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# ═══════════════════════════════════════════════════════════════════════════
# 📋 PARSE COMMAND LINE ARGUMENTS
# ═══════════════════════════════════════════════════════════════════════════

DURATION="${1:-$DEFAULT_DURATION}"
CONFIG_DIR="${2:-$DEFAULT_CONFIG_DIR}"

# ═══════════════════════════════════════════════════════════════════════════
# 🔧 HELPER FUNCTIONS
# ═══════════════════════════════════════════════════════════════════════════

# Convert duration string (1h, 12h, 24h) to hours (for --duration-hours)
duration_to_hours() {
    local dur="$1"

    if [[ $dur =~ ^([0-9]+)h$ ]]; then
        echo "${BASH_REMATCH[1]}"
    elif [[ $dur =~ ^([0-9]+)m$ ]]; then
        # Convert minutes to hours (decimal)
        local minutes="${BASH_REMATCH[1]}"
        echo "$(echo "scale=2; $minutes / 60" | bc)"
    elif [[ $dur =~ ^([0-9]+)s$ ]]; then
        # Convert seconds to hours (decimal)
        local seconds="${BASH_REMATCH[1]}"
        echo "$(echo "scale=2; $seconds / 3600" | bc)"
    elif [[ $dur =~ ^([0-9]+)$ ]]; then
        # If just a number, assume it's hours
        echo "$dur"
    else
        echo "Error: Invalid duration format. Use: 1h, 30m, 3600s, or 1" >&2
        exit 1
    fi
}

# Format duration for display
format_duration() {
    local hours=$1

    # Check if hours is a decimal
    if [[ $hours =~ \. ]]; then
        # Has decimal, show in appropriate unit
        local total_minutes=$(echo "$hours * 60" | bc | cut -d. -f1)
        if [ $total_minutes -lt 60 ]; then
            echo "${total_minutes}m"
        else
            echo "${hours}h"
        fi
    else
        # Integer hours
        if [ $hours -eq 1 ]; then
            echo "1 hour"
        else
            echo "${hours} hours"
        fi
    fi
}

# Print colored message
print_header() {
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}  $1${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════════════════${NC}"
}

print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

print_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

# ═══════════════════════════════════════════════════════════════════════════
# 🎯 MAIN SETUP
# ═══════════════════════════════════════════════════════════════════════════

# Convert duration to hours (for --duration-hours argument)
DURATION_HOURS=$(duration_to_hours "$DURATION")
DURATION_DISPLAY=$(format_duration $DURATION_HOURS)

# Generate test name with timestamp
TEST_NAME="${PROJECT_NAME}_5bot_${DURATION}_$(date +%Y%m%d_%H%M%S)"
RESULTS_DIR="results/$TEST_NAME"

# Validate config directory
if [ ! -d "$CONFIG_DIR" ]; then
    print_error "Config directory not found: $CONFIG_DIR"
    exit 1
fi

# Check if configs exist
CONFIGS=(
    "$CONFIG_DIR/1_micro_aggressive.toml"
    "$CONFIG_DIR/2_ultra_aggressive.toml"
    "$CONFIG_DIR/3_super_aggressive.toml"
    "$CONFIG_DIR/4_balanced_optimized.toml"
    "$CONFIG_DIR/5_conservative.toml"
)

for config in "${CONFIGS[@]}"; do
    if [ ! -f "$config" ]; then
        print_error "Config file not found: $config"
        exit 1
    fi
done

# ═══════════════════════════════════════════════════════════════════════════
# 🚀 LAUNCH SEQUENCE
# ═══════════════════════════════════════════════════════════════════════════

print_header "💎 PROJECT FLASH V3.5 - 5-BOT LAUNCH SUITE 💎"

echo ""
print_info "Test Configuration:"
echo "   Test Name:    $TEST_NAME"
echo "   Duration:     $DURATION_DISPLAY ($DURATION_HOURS hours)"
echo "   Config Dir:   $CONFIG_DIR"
echo "   Results Dir:  $RESULTS_DIR"
echo ""

# Create results directory
mkdir -p "$RESULTS_DIR"
print_success "Created results directory: $RESULTS_DIR"

# Create metadata file
cat > "$RESULTS_DIR/test_metadata.txt" << EOF
# Project Flash V3.5 - Test Metadata
Test Name: $TEST_NAME
Start Time: $(date)
Duration: $DURATION_DISPLAY ($DURATION_HOURS hours)
Config Directory: $CONFIG_DIR
Expected End Time: $(date -d "+${DURATION_HOURS} hours" 2>/dev/null || date -v +${DURATION_HOURS}H)

Bots Launched:
1. Micro Aggressive (0.015 spacing)
2. Ultra Aggressive (0.03 spacing) 🥇
3. Super Aggressive (0.05 spacing) 🥈
4. Balanced Optimized (0.10 spacing) 🔧
5. Conservative (0.20 spacing) 🛡️

Expected Total Trades: 405-610 (3x more than previous tests)
EOF

echo ""
print_header "🚀 Launching 5-Bot Suite..."
echo ""

# Bot 1: Micro Aggressive
cargo run --release -- \
  --config "$CONFIG_DIR/1_micro_aggressive.toml" \
  --duration-hours $DURATION_HOURS \
  > "$RESULTS_DIR/micro_aggressive.txt" 2>&1 &
PID1=$!
print_success "Bot 1: Micro Aggressive     (PID: $PID1) | Spacing: 0.015 🔬"

sleep 1  # Stagger launches slightly

# Bot 2: Ultra Aggressive
cargo run --release -- \
  --config "$CONFIG_DIR/2_ultra_aggressive.toml" \
  --duration-hours $DURATION_HOURS \
  > "$RESULTS_DIR/ultra_aggressive.txt" 2>&1 &
PID2=$!
print_success "Bot 2: Ultra Aggressive     (PID: $PID2) | Spacing: 0.03  🥇"

sleep 1

# Bot 3: Super Aggressive
cargo run --release -- \
  --config "$CONFIG_DIR/3_super_aggressive.toml" \
  --duration-hours $DURATION_HOURS \
  > "$RESULTS_DIR/super_aggressive.txt" 2>&1 &
PID3=$!
print_success "Bot 3: Super Aggressive     (PID: $PID3) | Spacing: 0.05  🥈"

sleep 1

# Bot 4: Balanced Optimized
cargo run --release -- \
  --config "$CONFIG_DIR/4_balanced_optimized.toml" \
  --duration-hours $DURATION_HOURS \
  > "$RESULTS_DIR/balanced_optimized.txt" 2>&1 &
PID4=$!
print_success "Bot 4: Balanced Optimized   (PID: $PID4) | Spacing: 0.10  🔧"

sleep 1

# Bot 5: Conservative
cargo run --release -- \
  --config "$CONFIG_DIR/5_conservative.toml" \
  --duration-hours $DURATION_HOURS \
  > "$RESULTS_DIR/conservative.txt" 2>&1 &
PID5=$!
print_success "Bot 5: Conservative         (PID: $PID5) | Spacing: 0.20  🛡️"

# Save PIDs to file
echo "$PID1 $PID2 $PID3 $PID4 $PID5" > "$RESULTS_DIR/pids.txt"
print_success "PIDs saved to: $RESULTS_DIR/pids.txt"

# ═══════════════════════════════════════════════════════════════════════════
# 📊 EXPECTED PERFORMANCE
# ═══════════════════════════════════════════════════════════════════════════

echo ""
print_header "📊 Expected Performance (based on 12h test analysis)"
echo ""
echo "   Bot                     | Expected Trades | Purpose"
echo "   ───────────────────────────────────────────────────────────────────"
echo "   Micro Aggressive        | 200-300        | Low vol micro-movements"
echo "   Ultra Aggressive 🥇     | 100-150        | PROVEN WINNER baseline"
echo "   Super Aggressive 🥈     | 50-80          | Higher volatility"
echo "   Balanced Optimized 🔧   | 35-50          | Trending markets"
echo "   Conservative 🛡️         | 20-30          | Safety net"
echo "   ───────────────────────────────────────────────────────────────────"
echo "   TOTAL                   | 405-610        | 3x more data!"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# 🛠️  QUICK ACTIONS & MONITORING
# ═══════════════════════════════════════════════════════════════════════════

print_header "🛠️  Quick Actions"
echo ""
echo "   📊 Monitor Live:"
echo "      watch -n 5 'tail -n 20 $RESULTS_DIR/*.txt'"
echo ""
echo "   🔍 Check Status:"
echo "      ps -p $PID1,$PID2,$PID3,$PID4,$PID5"
echo ""
echo "   📝 View Individual Logs:"
echo "      tail -f $RESULTS_DIR/micro_aggressive.txt"
echo "      tail -f $RESULTS_DIR/ultra_aggressive.txt"
echo "      tail -f $RESULTS_DIR/super_aggressive.txt"
echo "      tail -f $RESULTS_DIR/balanced_optimized.txt"
echo "      tail -f $RESULTS_DIR/conservative.txt"
echo ""
echo "   🛑 Stop All Bots:"
echo "      kill $PID1 $PID2 $PID3 $PID4 $PID5"
echo "      # OR use: ./scripts/launch/v3.5/stop_suite.sh $RESULTS_DIR"
echo ""
echo "   📈 Analyze Results (after test completes):"
echo "      ./scripts/monitoring/analyze_results.sh $RESULTS_DIR"
echo ""

# ═══════════════════════════════════════════════════════════════════════════
# 🎯 HEALTH CHECK
# ═══════════════════════════════════════════════════════════════════════════

echo ""
print_info "Performing health check in 5 seconds..."
sleep 5

echo ""
print_header "🏥 Health Check Status"
echo ""

ALL_RUNNING=true

for pid in $PID1 $PID2 $PID3 $PID4 $PID5; do
    if ps -p $pid > /dev/null 2>&1; then
        print_success "PID $pid is running"
    else
        print_error "PID $pid has stopped unexpectedly!"
        ALL_RUNNING=false
    fi
done

echo ""

if [ "$ALL_RUNNING" = true ]; then
    print_success "All bots launched successfully! LFG!!! 💎🔥"
    echo ""
    print_info "Test will run for $DURATION_DISPLAY and complete at:"
    date -d "+${DURATION_HOURS} hours" 2>/dev/null || date -v +${DURATION_HOURS}H
else
    print_error "Some bots failed to start. Check logs in: $RESULTS_DIR"
    echo ""
    print_info "Quick debug: tail -n 50 $RESULTS_DIR/micro_aggressive.txt"
    exit 1
fi

echo ""
print_header "🚀 5-Bot Pre-Production Suite Launched Successfully!"
echo ""
print_success "LFG BRO!!! 💎🔥 Happy trading!"
echo ""
