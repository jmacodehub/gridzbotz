cat > scripts/quick_summary.sh << 'BASHEOF'
#!/bin/bash

echo "================================================================"
echo "  🔥💎 PROJECT FLASH V2.5 - QUICK SUMMARY 🚀💎🔥"
echo "================================================================"
echo ""

echo "📊 LATEST TEST RESULTS:"
ls -lht results/*.json 2>/dev/null | head -5 || echo "  No results yet..."
echo ""

echo "📈 RECENT LOG ACTIVITY:"
tail -20 logs/*.log 2>/dev/null | tail -10 || echo "  No logs yet..."
echo ""

echo "💾 STORAGE USAGE:"
du -sh results/ logs/ 2>/dev/null || echo "  Directories not found"
echo ""

echo "🎯 ACTIVE PROCESSES:"
ps aux | grep -E "giga_test|live_monitor" | grep -v grep || echo "  No active tests"
echo ""

echo "⏰ Current Time: $(date '+%Y-%m-%d %H:%M:%S')"
echo "================================================================"
BASHEOF

chmod +x scripts/quick_summary.sh
