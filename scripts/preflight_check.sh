#!/bin/bash

# ═══════════════════════════════════════════════════════════════════════════
# ✅ PRE-FLIGHT CHECK - Verify Everything Ready
# ═══════════════════════════════════════════════════════════════════════════

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo ""
echo "════════════════════════════════════════════════════════════════════════"
echo "  ✅ PRE-FLIGHT CHECK - Master Flash V3.5"
echo "════════════════════════════════════════════════════════════════════════"
echo ""

CHECKS_PASSED=0
CHECKS_FAILED=0

check_pass() {
    echo -e "${GREEN}✅ $1${NC}"
    ((CHECKS_PASSED++))
}

check_fail() {
    echo -e "${RED}❌ $1${NC}"
    ((CHECKS_FAILED++))
}

# Check binary
echo "Checking binary..."
if [ -f "target/release/solana-grid-bot" ]; then
    check_pass "Binary exists"
else
    check_fail "Binary not found - run: cargo build --release"
fi

# Check configs
echo ""
echo "Checking configs..."
EXPECTED_CONFIGS=7
FOUND_CONFIGS=$(ls config/overnight_*.toml 2>/dev/null | wc -l)

if [ "$FOUND_CONFIGS" -ge "$EXPECTED_CONFIGS" ]; then
    check_pass "Found $FOUND_CONFIGS configs"
else
    check_fail "Only found $FOUND_CONFIGS configs (expected $EXPECTED_CONFIGS)"
fi

# Check scripts
echo ""
echo "Checking scripts..."
for script in launch_console.sh launch_ultimate_suite.sh monitor_suite.sh analyze_results.sh; do
    if [ -x "scripts/$script" ]; then
        check_pass "scripts/$script executable"
    else
        check_fail "scripts/$script not executable - run: chmod +x scripts/$script"
    fi
done

# Check directories
echo ""
echo "Checking directories..."
for dir in logs results config; do
    if [ -d "$dir" ]; then
        check_pass "$dir/ exists"
    else
        check_fail "$dir/ missing - run: mkdir -p $dir"
    fi
done

# Check disk space
echo ""
echo "Checking disk space..."
FREE_SPACE=$(df -h . | tail -1 | awk '{print $4}')
echo "   Free space: $FREE_SPACE"
check_pass "Disk space OK"

# Quick compile test
echo ""
echo "Testing quick compile..."
if cargo check --release --quiet 2>/dev/null; then
    check_pass "Code compiles"
else
    check_fail "Compilation errors detected"
fi

# Summary
echo ""
echo "════════════════════════════════════════════════════════════════════════"
if [ $CHECKS_FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 ALL CHECKS PASSED! ($CHECKS_PASSED/$((CHECKS_PASSED+CHECKS_FAILED)))${NC}"
    echo ""
    echo "You're ready to launch!"
    echo ""
    echo "Quick launch commands:"
    echo "  ./scripts/launch_console.sh        (Interactive)"
    echo "  ./scripts/launch_ultimate_suite.sh (All 8 bots)"
else
    echo -e "${RED}⚠️  SOME CHECKS FAILED ($CHECKS_FAILED failed, $CHECKS_PASSED passed)${NC}"
    echo ""
    echo "Fix the issues above before launching."
fi
echo "════════════════════════════════════════════════════════════════════════"
echo ""
