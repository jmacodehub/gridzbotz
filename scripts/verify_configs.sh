#!/bin/bash

echo "═══════════════════════════════════════════════════════════"
echo "  🎯 PROJECT FLASH V3.5+ - PRODUCTION CONFIG VERIFICATION"
echo "═══════════════════════════════════════════════════════════"
echo ""

cd config/production

configs=(
    "conservative.toml:0.30%:Safe"
    "balanced.toml:0.15%:Optimal"
    "aggressive.toml:0.10%:Active"
    "ultra_aggressive.toml:0.03%:Maximum"
)

echo "📋 Configuration Summary:"
echo "────────────────────────────────────────────────────────────"
printf "%-25s %-12s %-15s %s\n" "Config" "Spacing" "Profile" "Status"
echo "────────────────────────────────────────────────────────────"

for config_info in "${configs[@]}"; do
    IFS=':' read -r file spacing profile <<< "$config_info"
    
    if [ -f "$file" ]; then
        size=$(ls -lh "$file" | awk '{print $5}')
        printf "%-25s %-12s %-15s %s\n" "$file" "$spacing" "$profile" "✅ ($size)"
    else
        printf "%-25s %-12s %-15s %s\n" "$file" "$spacing" "$profile" "❌ Missing"
    fi
done

echo "────────────────────────────────────────────────────────────"
echo ""

# Count valid configs
valid_count=$(ls -1 *.toml 2>/dev/null | wc -l)
echo "✅ Valid Configs: $valid_count/4"
echo ""

if [ $valid_count -eq 4 ]; then
    echo "🎉 ALL PRODUCTION CONFIGS READY!"
    echo "═══════════════════════════════════════════════════════════"
else
    echo "⚠️  Some configs are missing. Run creation commands."
    echo "═══════════════════════════════════════════════════════════"
fi