#!/bin/bash
# Save as: scripts/launch_NOW.sh

echo "🏆 GRIDZBOTZ EMERGENCY LAUNCHER"
echo "================================"
echo ""

# Find binary
if [ -f "./target/release/solana-grid-bot" ]; then
    BINARY="./target/release/solana-grid-bot"
elif [ -f "./target/debug/solana-grid-bot" ]; then
    BINARY="./target/debug/solana-grid-bot"
else
    echo "❌ No binary found!"
    echo "Build with: cargo build --release"
    exit 1
fi

echo "✅ Binary: $BINARY"
echo ""

# Battle directory
BATTLE_DIR="logs/battle_$(date +%Y%m%d_%H%M)"
mkdir -p "$BATTLE_DIR"

echo "📂 Directory: $BATTLE_DIR"
echo ""

# List of configs to try (with fallbacks)
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

echo "🔍 Checking configs..."
echo ""

launched=0
missing=0

for entry in "${CONFIGS[@]}"; do
    IFS=':' read -r config name <<< "$entry"

    if [ -f "$config" ]; then
        echo "✓ Found: $name ($config)"

        # Launch it!
        RUST_LOG=info "$BINARY" \
            --config "$config" \
            --duration-minutes 5 \
            > "$BATTLE_DIR/${name}.log" 2>&1 &

        pid=$!
        echo $pid > "$BATTLE_DIR/${name}.pid"
        echo "  PID: $pid"

        ((launched++))
        sleep 1
    else
        echo "✗ Missing: $name ($config)"
        ((missing++))
    fi
    echo ""
done

echo "════════════════════════════════════════"
echo "✅ LAUNCHED: $launched bots"
echo "⚠️  MISSING: $missing configs"
echo "════════════════════════════════════════"
echo ""

if [ $launched -gt 0 ]; then
    echo "📊 Monitor: tail -f $BATTLE_DIR/*.log"
    echo "🛑 Stop: kill \$(cat $BATTLE_DIR/*.pid)"
else
    echo "❌ NO BOTS LAUNCHED!"
    echo ""
    echo "Available configs:"
    ls -1 config/*.toml config/production/*.toml 2>/dev/null || echo "No configs found!"
fi
echo ""
