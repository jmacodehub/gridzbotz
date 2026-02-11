#!/bin/bash
#═══════════════════════════════════════════════════════════════════════════
# 🤖 CONFIG BATTLE ROYALE V5 - AI OPTIMIZATION SYSTEM
#═══════════════════════════════════════════════════════════════════════════
#
# Tests multiple grid bot configs in parallel and picks the winner!
# NOW WITH V5 AI ULTIMATE EDITION! 🧠🔥
#
# USAGE:
#   ./scripts/config_battle.sh
#   ./scripts/config_battle.sh --duration 24h
#   ./scripts/config_battle.sh --custom config1.toml,config2.toml,config3.toml
#
#═══════════════════════════════════════════════════════════════════════════

set -e

# ─────────────────────────────────────────────────────────────────────────
# 🎨 COLORS
# ─────────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
MAGENTA='\033[0;35m'
CYAN='\033[0;36m'
BOLD='\033[1m'
RESET='\033[0m'

# ─────────────────────────────────────────────────────────────────────────
# ⚙️  CONFIGURATION
# ─────────────────────────────────────────────────────────────────────────
DURATION="24h"  # Battle duration
REPORT_INTERVAL="1h"  # Status update frequency

# 🔥 V5 DEFAULT CONFIGS - AI vs Static Showdown!
DEFAULT_CONFIGS=(
    "config/optimized/conservative_v4.toml"                    # 🛡️ Baseline: Safe & steady
    "config/optimized/balanced_v4.toml"                        # ⚖️ Comparison: Moderate risk
    "config/optimized/multi_strategy_v5_ai_ultimate.toml"      # 🧠🔥 THE CHAMPION: Full AI!
)

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --duration)
            DURATION="$2"
            shift 2
            ;;
        --custom)
            IFS=',' read -ra CUSTOM_CONFIGS <<< "$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--duration 24h] [--custom config1.toml,config2.toml]"
            exit 1
            ;;
    esac
done

# Use custom or default configs
if [ -n "${CUSTOM_CONFIGS}" ]; then
    CONFIGS=("${CUSTOM_CONFIGS[@]}")
else
    CONFIGS=("${DEFAULT_CONFIGS[@]}")
fi

# ─────────────────────────────────────────────────────────────────────────
# 🎯 BANNER
# ─────────────────────────────────────────────────────────────────────────
echo -e "${BOLD}${MAGENTA}"
echo "═══════════════════════════════════════════════════════════════════════"
echo "  🤖 AI CONFIG BATTLE ROYALE V5.0 🤖"
echo "  🧠 Now with FULL ADAPTIVE OPTIMIZATION! 🔥"
echo "═══════════════════════════════════════════════════════════════════════"
echo -e "${RESET}"
echo -e "${CYAN}Duration:${RESET} $DURATION"
echo -e "${CYAN}Contenders:${RESET} ${#CONFIGS[@]}"
echo ""

for i in "${!CONFIGS[@]}"; do
    CONFIG="${CONFIGS[$i]}"
    NICKNAME=$(basename "$CONFIG" .toml)
    
    # Add special marker for V5 AI
    if [[ "$CONFIG" == *"v5"* ]]; then
        AI_BADGE=" 🧠🔥"
    else
        AI_BADGE=""
    fi
    
    echo -e "${BOLD}${YELLOW}[$((i+1))]${RESET} ${GREEN}$NICKNAME${AI_BADGE}${RESET}"
    echo "    ${CYAN}→${RESET} $CONFIG"
done

echo ""
echo -e "${BOLD}${YELLOW}⏳ STARTING BATTLE IN 5 SECONDS...${RESET}"
echo ""
sleep 5

# ─────────────────────────────────────────────────────────────────────────
# 🚀 LAUNCH BATTLES
# ─────────────────────────────────────────────────────────────────────────
echo -e "${BOLD}${GREEN}🚀 Launching ${#CONFIGS[@]} battle instances...${RESET}"
echo ""

PIDS=()
LOG_DIR="logs/battles/$(date +%Y%m%d_%H%M%S)"
mkdir -p "$LOG_DIR"

for i in "${!CONFIGS[@]}"; do
    CONFIG="${CONFIGS[$i]}"
    NICKNAME=$(basename "$CONFIG" .toml)
    LOG_FILE="$LOG_DIR/${NICKNAME}.log"
    METRICS_FILE="$LOG_DIR/${NICKNAME}_metrics.json"
    
    if [[ "$CONFIG" == *"v5"* ]]; then
        AI_BADGE="🧠"
    else
        AI_BADGE="  "
    fi
    
    echo -e "${CYAN}[Battle $((i+1))]${RESET} Starting ${GREEN}$NICKNAME${RESET} $AI_BADGE"
    
    # Launch bot in background with metrics export
    cargo run --release -- \
        --config "$CONFIG" \
        --paper-trading \
        --duration "$DURATION" \
        --metrics-export "$METRICS_FILE" \
        > "$LOG_FILE" 2>&1 &
    
    PIDS+=($!)
    echo -e "           ${YELLOW}PID: ${PIDS[$i]}${RESET}"
    echo -e "           ${YELLOW}Log: $LOG_FILE${RESET}"
    echo ""
    
    sleep 2  # Stagger starts
done

echo -e "${BOLD}${GREEN}✅ All battles launched!${RESET}"
echo ""
echo -e "${CYAN}PIDs:${RESET} ${PIDS[*]}"
echo -e "${CYAN}Logs:${RESET} $LOG_DIR"
echo ""

# ─────────────────────────────────────────────────────────────────────────
# 📊 MONITORING LOOP
# ─────────────────────────────────────────────────────────────────────────
echo -e "${BOLD}${BLUE}📊 Monitoring battles (Ctrl+C to stop early)...${RESET}"
echo ""

START_TIME=$(date +%s)

while true; do
    sleep 3600  # Check every hour
    
    ELAPSED=$(($(date +%s) - START_TIME))
    HOURS=$((ELAPSED / 3600))
    
    echo ""
    echo -e "${BOLD}${MAGENTA}═══════════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}${CYAN}  BATTLE STATUS - ${HOURS}h Elapsed${RESET}"
    echo -e "${BOLD}${MAGENTA}═══════════════════════════════════════════════════════════════${RESET}"
    echo ""
    
    # Check each battle
    for i in "${!CONFIGS[@]}"; do
        CONFIG="${CONFIGS[$i]}"
        NICKNAME=$(basename "$CONFIG" .toml)
        PID=${PIDS[$i]}
        METRICS_FILE="$LOG_DIR/${NICKNAME}_metrics.json"
        
        if ps -p "$PID" > /dev/null 2>&1; then
            STATUS="${GREEN}RUNNING${RESET}"
        else
            STATUS="${RED}STOPPED${RESET}"
        fi
        
        if [[ "$CONFIG" == *"v5"* ]]; then
            AI_BADGE=" 🧠"
        else
            AI_BADGE=""
        fi
        
        echo -e "${BOLD}[$((i+1))] $NICKNAME${AI_BADGE}${RESET}"
        echo -e "   Status: $STATUS"
        
        # Try to extract quick metrics from log
        if [ -f "$METRICS_FILE" ]; then
            PNL=$(jq -r '.total_pnl // "N/A"' "$METRICS_FILE" 2>/dev/null || echo "N/A")
            TRADES=$(jq -r '.total_trades // "N/A"' "$METRICS_FILE" 2>/dev/null || echo "N/A")
            WIN_RATE=$(jq -r '.win_rate // "N/A"' "$METRICS_FILE" 2>/dev/null || echo "N/A")
            
            echo -e "   PnL: ${YELLOW}\$$PNL${RESET}"
            echo -e "   Trades: $TRADES"
            echo -e "   Win Rate: $WIN_RATE%"
        else
            echo -e "   ${YELLOW}Metrics pending...${RESET}"
        fi
        
        echo ""
    done
    
    # Check if all battles finished
    ALL_DONE=true
    for PID in "${PIDS[@]}"; do
        if ps -p "$PID" > /dev/null 2>&1; then
            ALL_DONE=false
            break
        fi
    done
    
    if [ "$ALL_DONE" = true ]; then
        echo -e "${BOLD}${GREEN}🏁 All battles complete!${RESET}"
        break
    fi
done

# ─────────────────────────────────────────────────────────────────────────
# 🏆 ANALYZE RESULTS
# ─────────────────────────────────────────────────────────────────────────
echo ""
echo -e "${BOLD}${MAGENTA}═══════════════════════════════════════════════════════════════${RESET}"
echo -e "${BOLD}${CYAN}  🏆 FINAL RESULTS & WINNER SELECTION 🏆${RESET}"
echo -e "${BOLD}${MAGENTA}═══════════════════════════════════════════════════════════════${RESET}"
echo ""

# Run analysis script (Python or Rust analyzer)
if command -v python3 &> /dev/null; then
    python3 scripts/analyze_battle.py "$LOG_DIR"
else
    # Fallback: Simple bash analysis
    echo -e "${YELLOW}⚠️  Python not found, using basic analysis${RESET}"
    echo ""
    
    BEST_SCORE=0
    WINNER=""
    
    for i in "${!CONFIGS[@]}"; do
        NICKNAME=$(basename "${CONFIGS[$i]}" .toml)
        METRICS_FILE="$LOG_DIR/${NICKNAME}_metrics.json"
        
        if [ -f "$METRICS_FILE" ]; then
            PNL=$(jq -r '.total_pnl // 0' "$METRICS_FILE" 2>/dev/null || echo "0")
            WIN_RATE=$(jq -r '.win_rate // 0' "$METRICS_FILE" 2>/dev/null || echo "0")
            
            # Simple score: PnL * 0.7 + WinRate * 0.3
            SCORE=$(echo "$PNL * 0.7 + $WIN_RATE * 0.3" | bc -l)
            
            if [[ "${CONFIGS[$i]}" == *"v5"* ]]; then
                AI_BADGE=" 🧠"
            else
                AI_BADGE=""
            fi
            
            echo -e "${BOLD}[$((i+1))] $NICKNAME${AI_BADGE}${RESET}"
            echo -e "   PnL: \$$PNL"
            echo -e "   Win Rate: $WIN_RATE%"
            echo -e "   ${CYAN}Score: $SCORE${RESET}"
            echo ""
            
            if (( $(echo "$SCORE > $BEST_SCORE" | bc -l) )); then
                BEST_SCORE=$SCORE
                WINNER=$NICKNAME
            fi
        fi
    done
    
    echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
    echo -e "${BOLD}${YELLOW}  🥇 WINNER: $WINNER 🥇${RESET}"
    echo -e "${BOLD}${YELLOW}  Score: $BEST_SCORE${RESET}"
    echo -e "${BOLD}${GREEN}═══════════════════════════════════════════════════════════════${RESET}"
    echo ""
fi

echo -e "${CYAN}Full results saved to:${RESET} $LOG_DIR"
echo -e "${CYAN}Next steps:${RESET}"
echo "  1. Review logs in $LOG_DIR"
echo "  2. Promote winner to production: cp config/optimized/${WINNER}.toml config/production/mainnet.toml"
echo "  3. Test winner on devnet before mainnet deployment"
echo ""

echo -e "${BOLD}${GREEN}🎉 Battle complete! May the best AI win! 🧠🔥${RESET}"
