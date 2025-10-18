//! Auto-Optimizer - Find best configs from test results

use serde::Deserialize;
use std::fs;
use glob::glob;
use colored::*;

#[derive(Debug, Deserialize, Clone)]
struct TestResult {
    name: String,
    final_roi: f64,
    sharpe_ratio: f64,
    total_fills: usize,
    filtered_trades: usize,
    grid_spacing: f64,
    grid_levels: u32,
    max_drawdown: f64,
}

#[tokio::main]
async fn main() {
    println!("\n{}\n", "=== AUTO-OPTIMIZER V2.5 - PROJECT FLASH ===".bright_green().bold());
    
    println!("Scanning for test results...\n");
    
    // Load results
    let mut results = vec![];
    let mut file_count = 0;
    
    for entry in glob("results/giga_v*_test_*.json").expect("Failed to read glob pattern") {
        if let Ok(path) = entry {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<Vec<TestResult>>(&content) {
                        Ok(data) => {
                            file_count += 1;
                            results.extend(data);
                        }
                        Err(e) => println!("Skipping invalid file: {:?} ({})", path, e),
                    }
                }
                Err(e) => println!("Failed to read {:?}: {}", path, e),
            }
        }
    }
    
    if results.is_empty() {
        println!("{}", "No results found! Run tests first.".bright_red());
        println!("Try: cargo run --example giga_test --release\n");
        return;
    }
    
    println!("Loaded {} results from {} files\n", 
        results.len().to_string().bright_green(),
        file_count.to_string().bright_yellow()
    );
    
    // Sort by ROI
    results.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap());
    
    // Calculate stats
    let avg_roi = results.iter().map(|r| r.final_roi).sum::<f64>() / results.len() as f64;
    let best = &results[0];
    let total_filtered = results.iter().map(|r| r.filtered_trades).sum::<usize>();
    let positive_count = results.iter().filter(|r| r.final_roi > 0.0).count();
    let win_rate = (positive_count as f64 / results.len() as f64) * 100.0;
    
    // Best by Sharpe
    let mut by_sharpe = results.clone();
    by_sharpe.sort_by(|a, b| b.sharpe_ratio.partial_cmp(&a.sharpe_ratio).unwrap());
    let best_sharpe = &by_sharpe[0];
    
    println!("{}", "=== BEST PERFORMER (ROI) ===".bright_yellow());
    println!("  Strategy: {}", best.name.bright_green());
    println!("  ROI: {:.2}%", best.final_roi);
    println!("  Sharpe: {:.2}", best.sharpe_ratio);
    println!("  Spacing: {:.2}%", best.grid_spacing);
    println!("  Levels: {}", best.grid_levels);
    println!("  Fills: {}", best.total_fills);
    println!();
    
    println!("{}", "=== BEST RISK-ADJUSTED (SHARPE) ===".bright_yellow());
    println!("  Strategy: {}", best_sharpe.name.bright_cyan());
    println!("  Sharpe: {:.2}", best_sharpe.sharpe_ratio);
    println!("  ROI: {:.2}%", best_sharpe.final_roi);
    println!("  Spacing: {:.2}%", best_sharpe.grid_spacing);
    println!("  Levels: {}", best_sharpe.grid_levels);
    println!();
    
    println!("{}", "=== OVERALL STATISTICS ===".bright_magenta());
    println!("  Total Tests: {}", results.len());
    println!("  Average ROI: {:.2}%", avg_roi);
    println!("  Win Rate: {:.1}% ({}/{})", win_rate, positive_count, results.len());
    println!("  Total Filtered: {} trades", total_filtered);
    println!();
    
    println!("{}", "=== RECOMMENDED CONFIG FOR LIVE ===".bright_cyan().bold());
    println!("  (Based on best risk-adjusted returns)\n");
    println!("{}", "  [trading]".bright_white());
    println!("  grid_spacing_percent = {:.2}", best_sharpe.grid_spacing);
    println!("  grid_levels = {}", best_sharpe.grid_levels);
    println!("  # Expected ROI: {:.2}%", best_sharpe.final_roi);
    println!("  # Sharpe Ratio: {:.2}", best_sharpe.sharpe_ratio);
    println!();
    
    // Save to file
    save_analysis(&results, best, best_sharpe, avg_roi, win_rate, total_filtered);
    
    println!("{}", "=== ANALYSIS COMPLETE ===".bright_green().bold());
    println!();
}

fn save_analysis(
    results: &[TestResult],
    best_roi: &TestResult,
    best_sharpe: &TestResult,
    avg_roi: f64,
    win_rate: f64,
    total_filtered: usize,
) {
    use chrono::Local;
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    
    let filename = format!("results/optimizer_analysis_{}.txt", timestamp);
    
    let mut content = String::from("AUTO-OPTIMIZER V2.5 ANALYSIS REPORT\n");
    content.push_str("===============================================\n\n");
    content.push_str(&format!("Generated: {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    content.push_str(&format!("Total Tests: {}\n\n", results.len()));
    
    content.push_str("BEST ROI:\n");
    content.push_str(&format!("  Strategy: {}\n", best_roi.name));
    content.push_str(&format!("  ROI: {:.2}%\n", best_roi.final_roi));
    content.push_str(&format!("  Config: {:.2}% spacing, {} levels\n\n", 
        best_roi.grid_spacing, best_roi.grid_levels));
    
    content.push_str("BEST RISK-ADJUSTED (SHARPE):\n");
    content.push_str(&format!("  Strategy: {}\n", best_sharpe.name));
    content.push_str(&format!("  Sharpe: {:.2}\n", best_sharpe.sharpe_ratio));
    content.push_str(&format!("  ROI: {:.2}%\n", best_sharpe.final_roi));
    content.push_str(&format!("  Config: {:.2}% spacing, {} levels\n\n", 
        best_sharpe.grid_spacing, best_sharpe.grid_levels));
    
    content.push_str("OVERALL STATS:\n");
    content.push_str(&format!("  Avg ROI: {:.2}%\n", avg_roi));
    content.push_str(&format!("  Win Rate: {:.1}%\n", win_rate));
    content.push_str(&format!("  Total Filtered: {}\n\n", total_filtered));
    
    content.push_str("RECOMMENDED LIVE CONFIG:\n");
    content.push_str("[trading]\n");
    content.push_str(&format!("grid_spacing_percent = {:.2}\n", best_sharpe.grid_spacing));
    content.push_str(&format!("grid_levels = {}\n", best_sharpe.grid_levels));
    
    if let Ok(_) = fs::write(&filename, content) {
        println!("Analysis saved to: {}", filename.bright_green());
    }
}

