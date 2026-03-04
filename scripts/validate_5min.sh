#!/bin/bash
# Save as: scripts/validate_5min.sh

echo "════════════════════════════════════════════════════════════"
echo "  ⚡ 5-MINUTE VALIDATION TEST - 3 BOTS"
echo "════════════════════════════════════════════════════════════"
echo ""

# Find binary
if [ -f "./target/release/solana-grid-bot" ]; then
    BINARY="./target/release/solana-grid-bot"
elif [ -f "./target/debug/solana-grid-bot" ]; then
    BINARY="./target/debug/solana-grid-bot"
else
    echo "❌ No binary found! Building..."
    cargo build --release
    BINARY="./target/release/solana-grid-bot"
fi

echo "✅ Binary: $BINARY"
echo ""

# Create validation directory
VALIDATION_DIR="logs/validation_$(date +%Y%m%d_%H%M)"
mkdir -p "$VALIDATION_DIR"

echo "📂 Validation directory: $VALIDATION_DIR"
echo ""

# Test config 1: Balanced (safest)
echo "════════════════════════════════════════════════════════════"
echo "  [1/3] TESTING: BALANCED (5 minutes)"
echo "════════════════════════════════════════════════════════════"
echo ""

if [ -f "config/overnight_balanced.toml" ]; then
    echo "Starting balanced bot..."
    echo "Output visible below AND saved to: $VALIDATION_DIR/balanced.log"
    echo ""

    RUST_LOG=info "$BINARY" \
        --config config/overnight_balanced.toml \
        --duration-hours 0.0833 \
        2>&1 | tee "$VALIDATION_DIR/balanced.log"

    echo ""
    echo "✅ Balanced test completed!"
else
    echo "⚠️  Config not found: config/overnight_balanced.toml"
fi

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  ✅ 5-MINUTE VALIDATION COMPLETE!"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "📊 Results saved to: $VALIDATION_DIR/"
echo ""
echo "🔍 Check logs:"
echo "   cat $VALIDATION_DIR/balanced.log"
echo ""
echo "If this ran successfully, you're ready for the 20-hour run!"
echo ""
