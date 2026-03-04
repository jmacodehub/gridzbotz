#!/bin/bash
echo "════════════════════════════════════════════════════════════"
echo "📊 DETAILED TRADE ANALYSIS - CURRENT RUN"
echo "════════════════════════════════════════════════════════════"
echo ""

for log in logs/battle_20h_20260208_2349/*.log; do
    name=$(basename $log .log)
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    echo "📈 $(echo $name | tr '[:lower:]' '[:upper:]')"
    echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
    
    # Trade counts
    buys=$(grep -c "BUY" "$log" 2>/dev/null || echo "0")
    sells=$(grep -c "SELL" "$log" 2>/dev/null || echo "0")
    repos=$(grep "Grid Repositions:" "$log" | tail -1 | awk '{print $3}')
    
    # Signals
    buy_signals=$(grep "Buy:" "$log" | tail -1 | awk '{print $2}' | tr -d ',')
    sell_signals=$(grep "Sell:" "$log" | tail -1 | awk '{print $2}' | tr -d ',')
    hold_signals=$(grep "Hold:" "$log" | tail -1 | awk '{print $2}')
    
    # ROI & Fees
    roi=$(grep "ROI:" "$log" | tail -1 | awk '{print $2}')
    fees=$(grep "Total Fees:" "$log" | tail -1 | awk '{print $3}')
    win_rate=$(grep "Win Rate:" "$log" | tail -1 | awk '{print $3}')
    
    echo "🔢 TRADES:"
    echo "   Executed Buys:    $buys"
    echo "   Executed Sells:   $sells"
    echo "   Total Trades:     $((buys + sells))"
    echo "   Grid Rebalances:  $repos"
    echo ""
    echo "📊 SIGNALS:"
    echo "   Buy Signals:      $buy_signals"
    echo "   Sell Signals:     $sell_signals"
    echo "   Hold Signals:     $hold_signals"
    echo "   Signal Fill:      $(echo "scale=2; ($buys + $sells) * 100 / ($buy_signals + $sell_signals + 0.001)" | bc)%"
    echo ""
    echo "💰 PERFORMANCE:"
    echo "   ROI:              $roi"
    echo "   Total Fees:       $fees"
    echo "   Win Rate:         $win_rate"
    if [ "$((buys + sells))" -gt 0 ]; then
        echo "   Avg Fee/Trade:    \$$(echo "scale=4; ${fees#\$} / ($buys + $sells)" | bc)"
    fi
    echo ""
done

echo "════════════════════════════════════════════════════════════"
