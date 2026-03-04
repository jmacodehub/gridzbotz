#!/bin/bash
# ═══════════════════════════════════════════════════════════════════════
# 💎 PROJECT FLASH - Professional Environment Setup
# ═══════════════════════════════════════════════════════════════════════
# 
# This script:
# 1. Renames v3.5_preproduction → v3.5_testing
# 2. Creates v3.5_development with proper settings
# 3. Sets up professional folder structure
#
# Usage: ./scripts/config/setup_dev_environment.sh
# ═══════════════════════════════════════════════════════════════════════

set -euo pipefail

# Colors for output
RED='\\033[0;31m'
GREEN='\\033[0;32m'
YELLOW='\\033[1;33m'
CYAN='\\033[0;36m'
PURPLE='\\033[0;35m'
NC='\\033[0m' # No Color
BOLD='\\033[1m'

echo -e "${PURPLE}${BOLD}"
echo "═══════════════════════════════════════════════════════════════════"
echo "  💎 PROJECT FLASH - Professional Environment Setup"
echo "═══════════════════════════════════════════════════════════════════"
echo -e "${NC}"

# Paths
BASE_DIR="config/production"
OLD_DIR="${BASE_DIR}/v3.5_preproduction"
TESTING_DIR="${BASE_DIR}/v3.5_testing"
DEV_DIR="${BASE_DIR}/v3.5_development"

# ═══════════════════════════════════════════════════════════════════════
# STEP 1: Rename preproduction → testing
# ═══════════════════════════════════════════════════════════════════════

echo -e "${CYAN}${BOLD}Step 1: Rename v3.5_preproduction → v3.5_testing${NC}"
echo "───────────────────────────────────────────────────────────────────"

if [ -d "$OLD_DIR" ]; then
    if [ -d "$TESTING_DIR" ]; then
        echo -e "${YELLOW}⚠️  Warning: $TESTING_DIR already exists!${NC}"
        read -p "Overwrite? (y/n): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            echo -e "${RED}❌ Aborted.${NC}"
            exit 1
        fi
        rm -rf "$TESTING_DIR"
    fi
    
    mv "$OLD_DIR" "$TESTING_DIR"
    echo -e "${GREEN}✅ Renamed: v3.5_preproduction → v3.5_testing${NC}"
    echo ""
elif [ -d "$TESTING_DIR" ]; then
    echo -e "${GREEN}✅ v3.5_testing already exists (skipping rename)${NC}"
    echo ""
else
    echo -e "${RED}❌ Error: Neither v3.5_preproduction nor v3.5_testing found!${NC}"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════════════
# STEP 2: Create v3.5_development from v3.5_testing
# ═══════════════════════════════════════════════════════════════════════

echo -e "${CYAN}${BOLD}Step 2: Create v3.5_development configs${NC}"
echo "───────────────────────────────────────────────────────────────────"

if [ -d "$DEV_DIR" ]; then
    echo -e "${YELLOW}⚠️  Warning: $DEV_DIR already exists!${NC}"
    read -p "Overwrite? (y/n): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${RED}❌ Aborted.${NC}"
        exit 1
    fi
    rm -rf "$DEV_DIR"
fi

mkdir -p "$DEV_DIR"
echo -e "${GREEN}✅ Created directory: $DEV_DIR${NC}"
echo ""

# Process each config file
config_count=0
for config_file in "$TESTING_DIR"/*.toml; do
    if [ -f "$config_file" ]; then
        filename=$(basename "$config_file")
        target_file="$DEV_DIR/$filename"
        
        echo -e "${CYAN}📝 Processing: $filename${NC}"
        
        # Copy and modify config
        sed -e 's/environment = "testing"/environment = "development"/' \\
            -e 's/enable_regime_gate = false/enable_regime_gate = true/' \\
            -e 's/min_volatility_to_trade = 0\\.0/min_volatility_to_trade = 0.02/' \\
            -e 's/pause_in_very_low_vol = false/pause_in_very_low_vol = true/' \\
            "$config_file" > "$target_file"
        
        echo -e "${GREEN}   ✅ Created: $filename${NC}"
        echo "   Changes applied:"
        echo "      • environment: testing → development"
        echo "      • enable_regime_gate: false → true"
        echo "      • min_volatility_to_trade: 0.0 → 0.02 (2%)"
        echo "      • pause_in_very_low_vol: false → true"
        echo ""
        
        config_count=$((config_count + 1))
    fi
done

# ═══════════════════════════════════════════════════════════════════════
# SUMMARY
# ═══════════════════════════════════════════════════════════════════════

echo -e "${PURPLE}${BOLD}"
echo "═══════════════════════════════════════════════════════════════════"
echo "  ✅ Setup Complete!"
echo "═══════════════════════════════════════════════════════════════════"
echo -e "${NC}"

echo -e "${GREEN}📁 Folder Structure:${NC}"
echo "   config/production/"
echo "   ├── v3.5_testing/      ✅ (Testing configs - regime OFF)"
echo "   └── v3.5_development/  ✅ (Development configs - regime ON)"
echo ""

echo -e "${GREEN}📊 Configs Created:${NC}"
echo "   • $config_count configuration files in v3.5_development/"
echo ""

echo -e "${CYAN}🚀 Next Steps:${NC}"
echo ""
echo "1. Launch 1h development test:"
echo -e "   ${GREEN}./scripts/launch/v3.5/launch_suite.sh 1h $DEV_DIR${NC}"
echo ""
echo "2. Monitor the test:"
echo -e "   ${GREEN}./scripts/monitoring/monitor.sh${NC}"
echo ""
echo "3. After test completes, compare results:"
echo -e "   ${GREEN}grep -c 'orders filled' results/project_flash_5bot_1h_*/ultra_aggressive.txt${NC}"
echo ""

echo -e "${PURPLE}${BOLD}"
echo "═══════════════════════════════════════════════════════════════════"
echo "  💎 Professional Setup Complete! Ready to test!"
echo "═══════════════════════════════════════════════════════════════════"
echo -e "${NC}"
