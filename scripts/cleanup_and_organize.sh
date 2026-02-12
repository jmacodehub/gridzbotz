#!/bin/bash

echo "🧹 Starting cleanup and organization..."

# 1. Archive last night's results
echo "📦 Archiving test results..."
mkdir -p results/archive
if [ -d "results/ultimate_20251018_004203" ]; then
    mv results/ultimate_20251018_004203 results/archive/
    echo "   ✅ Archived: ultimate_20251018_004203"
fi

# 2. Clean build artifacts
echo "🔨 Cleaning build artifacts..."
cargo clean
echo "   ✅ Build artifacts cleaned"

# 3. Organize configs
echo "📁 Organizing configs..."
mkdir -p config/{production,testing,archive,templates}

# Move old configs to archive if needed
if [ -f "config/master.toml" ]; then
    cp config/master.toml config/archive/master_backup_$(date +%Y%m%d).toml
    echo "   ✅ Backed up master.toml"
fi

# 4. Create production config directory structure
echo "📂 Setting up production structure..."
mkdir -p config/production/{conservative,balanced,aggressive}

# 5. Organize logs
echo "📝 Organizing logs..."
mkdir -p logs/archive
if ls logs/*.log 1> /dev/null 2>&1; then
    mv logs/*.log logs/archive/ 2>/dev/null || true
    echo "   ✅ Archived old logs"
fi

# 6. Git status
echo "📊 Git status:"
git status --short

# 7. Summary
echo ""
echo "════════════════════════════════════════════════════════════"
echo "  ✅ CLEANUP COMPLETE!"
echo "════════════════════════════════════════════════════════════"
echo ""
echo "📁 Directory Structure:"
tree -L 2 -d config/ results/ 2>/dev/null || ls -R config/ results/
echo ""
EOF

chmod +x scripts/cleanup_and_organize.sh
./scripts/cleanup_and_organize.sh

echo ""
echo "✅ MISSION 1B COMPLETE: Workspace organized!"