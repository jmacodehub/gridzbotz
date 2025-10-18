#!/bin/bash
# Live GIGA V2.5 Monitor

while true; do
    clear
    echo "═══════════════════════════════════════════════════════════════"
    echo "  🔥💎 PROJECT FLASH V2.5 - LIVE MONITOR 🚀💎🔥"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    echo "📊 ACTIVE TESTS:"
    ps aux | grep giga_test | grep -v grep | wc -l
    echo ""
    echo "💾 LATEST RESULTS:"
    ls -lht results/ | head -6
    echo ""
    echo "📈 RECENT ACTIVITY (last 15 lines):"
    tail -15 logs/overnight_v25.log 2>/dev/null || echo "No logs yet..."
    echo ""
    echo "💰 DISK USAGE:"
    du -sh results/ logs/
    echo ""
    echo "🕐 $(date '+%Y-%m-%d %H:%M:%S')"
    echo "═══════════════════════════════════════════════════════════════"
    sleep 10
done
