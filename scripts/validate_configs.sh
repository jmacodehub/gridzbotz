#!/bin/bash

echo "🔥 V4.1 CONFIG VALIDATION - ALL 4 BOTS"
echo "=======================================

"

CONFIGS=(
    "config/production/v4.0_development/micro_aggressive_v4.toml"
    "config/production/v4.0_development/balanced_opportunist_v4.toml"
    "config/production/v4.0_development/volatility_hunter_v4.toml"
    "config/production/v4.0_development/atr_dynamic_v4.toml"
)

PASSED=0
FAILED=0

for config in "${CONFIGS[@]}"; do
    name=$(basename "$config" .toml)
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "🤖 Testing: $name"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    if [ ! -f "$config" ]; then
        echo "❌ Config file not found!"
        ((FAILED++))
        continue
    fi
    
    # Run for 60 seconds (enough time for feeds to connect and start trading)
    timeout 60s cargo run --release -- --config "$config" --duration-minutes 1 > /tmp/test_$name.log 2>&1
    
    EXIT_CODE=$?
    
    if [ $EXIT_CODE -eq 124 ]; then
        # Timeout = success (bot ran for 60s)
        echo "✅ PASSED - Bot ran for 60 seconds successfully"
        
        # Check for key success markers
        if grep -q "HEALTHY" /tmp/test_$name.log; then
            echo "   ✅ Feed health checks passing"
        fi
        if grep -q "Grid stable\|Rebalanced" /tmp/test_$name.log; then
            echo "   ✅ Trading loop active"
        fi
        
        ((PASSED++))
    else
        echo "❌ FAILED - Exit code: $EXIT_CODE"
        ((FAILED++))
        echo ""
        echo "Last 30 lines of output:"
        tail -30 /tmp/test_$name.log
    fi
    echo ""
done

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "📊 VALIDATION RESULTS"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "✅ Passed: $PASSED/4"
echo "❌ Failed: $FAILED/4"
echo ""

if [ $PASSED -eq 4 ]; then
    echo "🎉 ALL CONFIGS VALIDATED! READY FOR SPRINT TESTS!"
    exit 0
else
    echo "⚠️  Some configs need fixes"
    exit 1
fi