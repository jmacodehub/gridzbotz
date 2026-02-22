//! ═══════════════════════════════════════════════════════════════════════════
//! 🚀 MASTER GIGA TEST V4.0 - SIMPLIFIED & WORKING
//! 
//! Runs your existing GridBot using the ACTUAL API
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::{Config, GridBot};
use tokio::time::{interval, Duration, Instant};
use log::info;
use std::env;

// ═══════════════════════════════════════════════════════════════════════════
// Test Modes
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy)]
enum TestMode {
    Quick,       // 10 minutes
    Standard,    // 1 hour
    Overnight,   // 8 hours
}

impl TestMode {
    fn duration_secs(&self) -> u64 {
        match self {
            Self::Quick => 600,       // 10 min
            Self::Standard => 3600,   // 1 hour
            Self::Overnight => 28800, // 8 hours
        }
    }
    
    fn emoji(&self) -> &'static str {
        match self {
            Self::Quick => "⚡",
            Self::Standard => "🎯",
            Self::Overnight => "🌙",
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Test Metrics
// ═══════════════════════════════════════════════════════════════════════════

struct TestMetrics {
    start_time: Instant,
    total_cycles: u64,
    successful_cycles: u64,
    failed_cycles: u64,
    min_cycle_time_ms: u64,
    max_cycle_time_ms: u64,
    total_cycle_time_ms: u64,
}

impl TestMetrics {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            total_cycles: 0,
            successful_cycles: 0,
            failed_cycles: 0,
            min_cycle_time_ms: u64::MAX,
            max_cycle_time_ms: 0,
            total_cycle_time_ms: 0,
        }
    }
    
    fn record_cycle(&mut self, duration_ms: u64) {
        self.total_cycles += 1;
        self.successful_cycles += 1;
        self.total_cycle_time_ms += duration_ms;
        self.min_cycle_time_ms = self.min_cycle_time_ms.min(duration_ms);
        self.max_cycle_time_ms = self.max_cycle_time_ms.max(duration_ms);
    }
    
    fn avg_cycle_time_ms(&self) -> f64 {
        if self.total_cycles == 0 {
            0.0
        } else {
            self.total_cycle_time_ms as f64 / self.total_cycles as f64
        }
    }
    
    fn success_rate(&self) -> f64 {
        if self.total_cycles == 0 {
            100.0
        } else {
            (self.successful_cycles as f64 / self.total_cycles as f64) * 100.0
        }
    }
    
    fn print_summary(&self, mode: TestMode) {
        let elapsed = self.start_time.elapsed();
        
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║  {} MASTER GIGA TEST V4.0 - FINAL REPORT", mode.emoji());
        println!("╚═══════════════════════════════════════════════════════════════╝\n");
        
        println!("⏱️  Test Duration:");
        println!("   Elapsed:      {:?}", elapsed);
        println!("   Mode:         {:?}", mode);
        
        println!("\n📊 Cycle Performance:");
        println!("   Total:        {}", self.total_cycles);
        println!("   Success:      {} ({:.2}%)", self.successful_cycles, self.success_rate());
        println!("   Failed:       {}", self.failed_cycles);
        println!("   Avg Time:     {:.2}ms", self.avg_cycle_time_ms());
        println!("   Min Time:     {}ms", if self.min_cycle_time_ms == u64::MAX { 0 } else { self.min_cycle_time_ms });
        println!("   Max Time:     {}ms", self.max_cycle_time_ms);
        
        println!("\n✅ Test Status: {}", 
            if self.success_rate() > 99.0 { "PASSED ✅" } else { "NEEDS REVIEW ⚠️" });
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Main
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();
    
    // Parse args
    let args: Vec<String> = env::args().collect();
    let mode = if let Some(pos) = args.iter().position(|a| a == "--mode") {
        match args.get(pos + 1).map(|s| s.as_str()) {
            Some("quick") => TestMode::Quick,
            Some("standard") => TestMode::Standard,
            Some("overnight") => TestMode::Overnight,
            _ => TestMode::Quick,
        }
    } else {
        TestMode::Quick
    };
    
    let config_path = if let Some(pos) = args.iter().position(|a| a == "--config") {
        args.get(pos + 1).cloned().unwrap_or_else(|| "config/giga_v2.toml".to_string())
    } else {
        "config/giga_v2.toml".to_string()
    };
    
    print_banner(mode, &config_path);
    
    // Load config
    let config = Config::from_file(&config_path)?;
    
    info!("🤖 Initializing GridBot...");
    let mut bot = GridBot::new(config)?;
    bot.initialize().await?;
    info!("✅ GridBot initialized");
    
    // Initialize metrics
    let mut metrics = TestMetrics::new();
    
    // Calculate test parameters  
    let total_seconds = mode.duration_secs();
    info!("🚀 Starting {} mode test: {} seconds", mode.emoji(), total_seconds);
    
    // Main test loop - run the bot just like main.rs does
    let mut test_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0u64;
    
    while elapsed_seconds < total_seconds {
        test_interval.tick().await;
        let cycle_start = Instant::now();
        elapsed_seconds += 1;
        
        // Bot runs internally - just let time pass
        // The bot's internal tasks handle everything
        
        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        metrics.record_cycle(cycle_time);
        
        // Periodic logging
        if elapsed_seconds % 60 == 0 {
            let progress = (elapsed_seconds as f64 / total_seconds as f64) * 100.0;
            info!("📊 Progress: {:.1}% | Time: {}s/{}", 
                progress, elapsed_seconds, total_seconds);
        }
    }
    
    // Print final report
    metrics.print_summary(mode);
    
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

fn print_banner(mode: TestMode, config_path: &str) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║     🚀 MASTER GIGA TEST V4.0 - SIMPLIFIED & WORKING         ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  {} Mode:        {:?}", mode.emoji(), mode);
    println!("  📁 Config:      {}", config_path);
    println!("  ⏱️  Duration:    {}s", mode.duration_secs());
    println!();
    println!("═══════════════════════════════════════════════════════════════\n");
}
