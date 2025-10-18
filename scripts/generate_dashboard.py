#!/usr/bin/env python3
"""🔥💎 Project Flash V2.5 - Enhanced HTML Dashboard Generator 🚀💎🔥"""

import json
import glob
import os
from datetime import datetime

def load_results():
    """Load all test results from JSON files"""
    results = []
    for filepath in glob.glob("results/giga_v*_test_*.json"):
        try:
            with open(filepath, 'r') as f:
                data = json.load(f)
                if isinstance(data, list):
                    results.extend(data)
                else:
                    results.append(data)
        except Exception as e:
            print(f"⚠️  Failed to load {filepath}: {e}")
    return results

def calculate_stats(results):
    """Calculate dashboard statistics"""
    if not results:
        return None
    
    total = len(results)
    avg_roi = sum(r.get('final_roi', 0) for r in results) / total
    positive = sum(1 for r in results if r.get('final_roi', 0) > 0)
    win_rate = (positive / total) * 100
    total_filtered = sum(r.get('filtered_trades', 0) for r in results)
    best_roi = max((r.get('final_roi', 0) for r in results), default=0)
    worst_roi = min((r.get('final_roi', 0) for r in results), default=0)
    
    return {
        'total': total,
        'avg_roi': avg_roi,
        'positive': positive,
        'win_rate': win_rate,
        'total_filtered': total_filtered,
        'best_roi': best_roi,
        'worst_roi': worst_roi
    }

def generate_table_rows(results, limit=20):
    """Generate HTML table rows"""
    rows = []
    for i, r in enumerate(results[:limit], 1):
        roi = r.get('final_roi', 0)
        color = "text-green-400" if roi > 0 else "text-red-400"
        name = r.get('name', 'Unknown')[:35]
        
        row = f'''
        <tr class="border-b border-gray-700 hover:bg-gray-700/50 transition-colors">
          <td class="p-3 font-bold text-gray-400">#{i}</td>
          <td class="p-3 font-medium">{name}</td>
          <td class="p-3 text-right {color} font-bold">{roi:.2f}%</td>
          <td class="p-3 text-right text-yellow-400">{r.get('sharpe_ratio', 0):.2f}</td>
          <td class="p-3 text-right text-purple-400">{r.get('grid_spacing', 0):.2f}%</td>
          <td class="p-3 text-right text-cyan-400">{r.get('grid_levels', 0)}</td>
          <td class="p-3 text-right text-blue-400">{r.get('total_fills', 0)}</td>
          <td class="p-3 text-right text-red-400">{r.get('filtered_trades', 0)}</td>
        </tr>'''
        rows.append(row)
    
    return '\n'.join(rows)

def generate_chart_data(results):
    """Generate data for Chart.js"""
    # ROI Bar Chart (top 10)
    top_10 = results[:10]
    roi_labels = [r.get('name', '')[:15] for r in top_10]
    roi_data = [r.get('final_roi', 0) for r in top_10]
    roi_colors = ['rgba(34, 197, 94, 0.7)' if roi > 0 else 'rgba(239, 68, 68, 0.7)' for roi in roi_data]
    
    # Scatter Plot (ROI vs Sharpe)
    scatter_data = [
        {'x': r.get('sharpe_ratio', 0), 'y': r.get('final_roi', 0), 'label': r.get('name', '')[:20]}
        for r in results[:20]
    ]
    
    return {
        'roi_labels': json.dumps(roi_labels),
        'roi_data': json.dumps(roi_data),
        'roi_colors': json.dumps(roi_colors),
        'scatter_data': json.dumps(scatter_data)
    }

def generate_html(stats, rows, chart_data):
    """Generate complete HTML dashboard"""
    
    avg_roi_color = "text-green-400" if stats['avg_roi'] > 0 else "text-red-400"
    
    html = f'''<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Project Flash V2.5 - Results Dashboard</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <script src="https://cdn.jsdelivr.net/npm/chart.js@4.4.0/dist/chart.umd.min.js"></script>
    <style>
        body {{
            background: linear-gradient(135deg, #0a0e27 0%, #1a1f3a 100%);
            font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
            min-height: 100vh;
        }}
        .card {{
            background: rgba(26, 31, 58, 0.85);
            backdrop-filter: blur(12px);
            border-radius: 16px;
            padding: 24px;
            border: 1px solid rgba(102, 126, 234, 0.15);
        }}
        .glow {{
            box-shadow: 0 0 40px rgba(102, 126, 234, 0.4);
        }}
        @keyframes pulse {{
            0%, 100% {{ opacity: 1; }}
            50% {{ opacity: 0.7; }}
        }}
        .pulse {{ animation: pulse 2s ease-in-out infinite; }}
    </style>
</head>
<body class="text-white p-4 md:p-8">

<div class="max-w-7xl mx-auto">
    <!-- Header -->
    <div class="card glow mb-8 text-center">
        <h1 class="text-4xl md:text-6xl font-bold bg-gradient-to-r from-purple-400 via-pink-500 to-blue-500 bg-clip-text text-transparent mb-4">
            🔥💎 PROJECT FLASH V2.5 🚀💎🔥
        </h1>
        <p class="text-xl text-gray-300 mb-2">Results Dashboard</p>
        <p class="text-sm text-gray-400">Generated: {datetime.now().strftime("%Y-%m-%d %H:%M:%S")}</p>
        <div class="mt-4 inline-block px-4 py-2 bg-green-500/20 rounded-full text-green-400 text-sm pulse">
            ● Auto-refresh: 30s
        </div>
    </div>

    <!-- Stats Grid -->
    <div class="grid grid-cols-2 md:grid-cols-4 gap-4 md:gap-6 mb-8">
        <div class="card border-l-4 border-blue-500">
            <div class="text-gray-400 text-xs md:text-sm uppercase tracking-wide">Total Tests</div>
            <div class="text-3xl md:text-4xl font-bold text-blue-400 mt-2">{stats['total']}</div>
        </div>
        <div class="card border-l-4 border-green-500">
            <div class="text-gray-400 text-xs md:text-sm uppercase tracking-wide">Average ROI</div>
            <div class="text-3xl md:text-4xl font-bold {avg_roi_color} mt-2">{stats['avg_roi']:.2f}%</div>
        </div>
        <div class="card border-l-4 border-yellow-500">
            <div class="text-gray-400 text-xs md:text-sm uppercase tracking-wide">Win Rate</div>
            <div class="text-3xl md:text-4xl font-bold text-yellow-400 mt-2">{stats['win_rate']:.1f}%</div>
        </div>
        <div class="card border-l-4 border-red-500">
            <div class="text-gray-400 text-xs md:text-sm uppercase tracking-wide">Filtered</div>
            <div class="text-3xl md:text-4xl font-bold text-red-400 mt-2">{stats['total_filtered']}</div>
        </div>
    </div>

    <!-- Charts Row -->
    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 mb-8">
        <div class="card">
            <h2 class="text-xl font-bold mb-4 flex items-center">
                <span class="mr-2">📊</span> ROI Distribution
            </h2>
            <canvas id="roiChart"></canvas>
        </div>
        <div class="card">
            <h2 class="text-xl font-bold mb-4 flex items-center">
                <span class="mr-2">📈</span> ROI vs Sharpe Ratio
            </h2>
            <canvas id="perfChart"></canvas>
        </div>
    </div>

    <!-- Results Table -->
    <div class="card overflow-x-auto">
        <h2 class="text-2xl font-bold mb-6 flex items-center">
            <span class="mr-2">📋</span> Detailed Results
        </h2>
        <table class="w-full text-sm">
            <thead>
                <tr class="border-b-2 border-gray-700 text-left">
                    <th class="p-3">Rank</th>
                    <th class="p-3">Strategy</th>
                    <th class="p-3 text-right">ROI</th>
                    <th class="p-3 text-right">Sharpe</th>
                    <th class="p-3 text-right">Spacing</th>
                    <th class="p-3 text-right">Levels</th>
                    <th class="p-3 text-right">Fills</th>
                    <th class="p-3 text-right">Filtered</th>
                </tr>
            </thead>
            <tbody>
{rows}
            </tbody>
        </table>
    </div>

    <!-- Footer -->
    <div class="text-center mt-8 text-gray-500 text-sm">
        <p>🔥💎 Project Flash V2.5 - Powered by Rust & Solana 🚀💎🔥</p>
    </div>
</div>

<script>
// Chart.js Configuration
Chart.defaults.color = '#9CA3AF';
Chart.defaults.borderColor = 'rgba(156, 163, 175, 0.1)';

// ROI Distribution Chart
const roiCtx = document.getElementById('roiChart').getContext('2d');
new Chart(roiCtx, {{
    type: 'bar',
    data: {{
        labels: {chart_data['roi_labels']},
        datasets: [{{
            label: 'ROI (%)',
            data: {chart_data['roi_data']},
            backgroundColor: {chart_data['roi_colors']},
            borderWidth: 0
        }}]
    }},
    options: {{
        responsive: true,
        maintainAspectRatio: true,
        plugins: {{
            legend: {{ display: false }},
            tooltip: {{
                backgroundColor: 'rgba(0, 0, 0, 0.8)',
                padding: 12,
                titleFont: {{ size: 14 }},
                bodyFont: {{ size: 13 }}
            }}
        }},
        scales: {{
            y: {{
                beginAtZero: true,
                grid: {{ color: 'rgba(156, 163, 175, 0.1)' }},
                ticks: {{ callback: value => value + '%' }}
            }},
            x: {{
                grid: {{ display: false }}
            }}
        }}
    }}
}});

// Performance Comparison Chart
const perfCtx = document.getElementById('perfChart').getContext('2d');
new Chart(perfCtx, {{
    type: 'scatter',
    data: {{
        datasets: [{{
            label: 'Strategies',
            data: {chart_data['scatter_data']},
            backgroundColor: 'rgba(102, 126, 234, 0.7)',
            borderColor: 'rgba(102, 126, 234, 1)',
            borderWidth: 2,
            pointRadius: 7,
            pointHoverRadius: 10
        }}]
    }},
    options: {{
        responsive: true,
        maintainAspectRatio: true,
        plugins: {{
            legend: {{ display: false }},
            tooltip: {{
                backgroundColor: 'rgba(0, 0, 0, 0.8)',
                padding: 12,
                callbacks: {{
                    label: context => {{
                        const point = context.raw;
                        return [
                            `ROI: ${{point.y.toFixed(2)}}%`,
                            `Sharpe: ${{point.x.toFixed(2)}}`,
                            `Strategy: ${{point.label}}`
                        ];
                    }}
                }}
            }}
        }},
        scales: {{
            x: {{
                title: {{ display: true, text: 'Sharpe Ratio', color: '#9CA3AF' }},
                grid: {{ color: 'rgba(156, 163, 175, 0.1)' }}
            }},
            y: {{
                title: {{ display: true, text: 'ROI (%)', color: '#9CA3AF' }},
                grid: {{ color: 'rgba(156, 163, 175, 0.1)' }}
            }}
        }}
    }}
}});

// Auto-refresh every 30 seconds
setTimeout(() => location.reload(), 30000);
</script>

</body>
</html>'''
    
    return html

def main():
    print("🔥💎 Generating Project Flash V2.5 Dashboard... 🚀💎🔥\n")
    
    # Load results
    results = load_results()
    if not results:
        print("❌ No results found! Run some tests first:")
        print("   cargo run --example giga_test --release")
        return 1
    
    # Sort by ROI
    results.sort(key=lambda x: x.get('final_roi', 0), reverse=True)
    
    # Calculate stats
    stats = calculate_stats(results)
    print(f"✅ Loaded {stats['total']} test results")
    print(f"📊 Average ROI: {stats['avg_roi']:.2f}%")
    print(f"🎯 Win Rate: {stats['win_rate']:.1f}%")
    print(f"🚫 Filtered: {stats['total_filtered']} trades\n")
    
    # Generate components
    rows = generate_table_rows(results)
    chart_data = generate_chart_data(results)
    html = generate_html(stats, rows, chart_data)
    
    # Save file
    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
    filename = f"results/dashboard_{timestamp}.html"
    
    with open(filename, 'w', encoding='utf-8') as f:
        f.write(html)
    
    filepath = os.path.abspath(filename)
    print(f"✅ Dashboard generated: {filename}")
    print(f"🌐 Open in browser: file://{filepath}")
    print("\n💎 Dashboard features:")
    print("   • Auto-refreshes every 30 seconds")
    print("   • Interactive charts with Chart.js")
    print("   • Mobile responsive")
    print("   • Shows grid spacing & levels")
    
    return 0

if __name__ == '__main__':
    exit(main())
