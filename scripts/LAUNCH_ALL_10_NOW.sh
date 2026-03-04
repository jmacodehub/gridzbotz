#!/bin/bash
echo "🏆 GRIDZBOTZ 20-HOUR BATTLE ROYALE - ALL 10 GLADIATORS!"
echo "========================================================"
echo ""
BINARY="./target/release/solana-grid-bot"
BATTLE_DIR="logs/battle_20h_$(date +%Y%m%d_%H%M)"
mkdir -p "$BATTLE_DIR"
echo "📂 Directory: $BATTLE_DIR"
echo "⏰ Duration: 20 hours"
echo "🎯 Expected end: $(date -v+20H '+%b %d %H:%M' 2>/dev/null || date -d '+20 hours' '+%b %d %H:%M' 2>/dev/null)"
echo ""
declare -a CONFIGS=(
    "config/production/ultra_aggressive.toml:maxlevels"
    "config/overnight_aggressive.toml:aggressive"
    "config/overnight_balanced.toml:balanced"
    "config/master.toml:master"
    "config/overnight_conservative.toml:conservative"
    "config/overnight_super_aggressive.toml:superagg"
    "config/overnight_multi_strategy.toml:multistrat"
    "config/overnight_ultra_aggressive.toml:ultraagg"
    "config/production/balanced.toml:prodbal"
    "config/production/conservative.toml:prodcons"
)
launched=0
for entry in "${CONFIGS[@]}"; do
    IFS=':' read -r config name <<< "$entry"
    echo "[$name] Launching for 20 hours..."
    RUST_LOG=info "$BINARY" --config "$config" --duration-hours 20 > "$BATTLE_DIR/${name}.log" 2>&1 &
    pid=$!
    echo $pid > "$BATTLE_DIR/${name}.pid"
    echo "   ✓ PID: $pid"
    ((launched++))
    sleep 2
done
echo ""
echo "════════════════════════════════════════════════════════"
echo "✅ LAUNCHED: $launched/10 GLADIATORS FOR 20 HOURS!"
echo "════════════════════════════════════════════════════════"
echo ""
echo "📂 Battle directory: $BATTLE_DIR"
echo ""
echo "📊 Tomorrow morning commands:"
echo "  Quick check: ps aux | grep solana-grid-bot | wc -l"
echo "  View status: tail -1 $BATTLE_DIR/*.log"
echo "  Stop all:    kill \$(cat $BATTLE_DIR/*.pid)"
echo ""
echo "💤 GO TO BED NOW! SEE YOU IN 20 HOURS!"
echo "🏆 LFG!!!"
