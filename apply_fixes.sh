#!/bin/bash

echo "🔧 Applying Option B fixes..."

# Fix 1: Disable real_trader
echo "1️⃣ Disabling real_trader module..."
sed -i.bak 's/^pub mod real_trader;$/\/\/ pub mod real_trader;  \/\/ DISABLED - Phase 7/' src/trading/mod.rs
sed -i.bak 's/^#\[cfg(all(feature = "live-trading", feature = "security"))\]$/\/\/ &/' src/trading/mod.rs

# Fix 2: Remove RegimeGateConfig import
echo "2️⃣ Fixing analytics imports..."
sed -i.bak '/use crate::config::RegimeGateConfig;/d' src/strategies/shared/analytics/mod.rs

# Fix 3: Add Clone to PriceFeed
echo "3️⃣ Adding Clone derive to PriceFeed..."
sed -i.bak '/^pub struct PriceFeed {/i\
#[derive(Clone)]
' src/trading/price_feed.rs

# Fix 4: Fix WebSocket await
echo "4️⃣ Fixing WebSocket initialization..."
sed -i.bak 's/PythWebSocketFeed::new(vec\[self.feed_id.clone()\]).await?/let mut ws = PythWebSocketFeed::new(vec[self.feed_id.clone()]); ws.start().await?/' src/trading/price_feed.rs

# Fix 5: Remove .ok()
echo "5️⃣ Fixing .ok() method..."
sed -i.bak 's/.get_price(id).await.ok()/.get_price(id).await/' src/trading/price_feed.rs

# Fix 6: Remove OrderType import
echo "6️⃣ Fixing unused imports..."
sed -i.bak 's/OrderSide, OrderType, PlacedOrder/OrderSide, PlacedOrder/' src/dex/jupiter_client.rs

# Fix 7: Fix deprecated rand
echo "7️⃣ Fixing deprecated rand methods..."
sed -i.bak 's/rand::thread_rng()/rand::rng()/' src/trading/pyth_websocket.rs
sed -i.bak 's/\.gen_range(/\.random_range(/' src/trading/pyth_websocket.rs

echo ""
echo "✅ All fixes applied!"
echo "🧪 Running cargo check..."
cargo check --lib

echo ""
echo "📋 Backup files created with .bak extension"
echo "   If something breaks, you can restore with:"
echo "   find src -name '*.bak' -exec sh -c 'mv \"\$1\" \"\${1%.bak}\"' _ {} \\;"
