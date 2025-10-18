//! ═══════════════════════════════════════════════════════════════════════════
//! UNIFIED CONFIGURATION SYSTEM - PROJECT FLASH
//! Single Source of Truth for All Bot Settings
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Serialize, Deserialize};
use std::path::Path;
use anyhow::Result;

/// Master bot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotConfig {
    pub test_name: String,
    pub trading: TradingConfig,
    pub grid: GridConfig,
    pub risk: RiskConfig,
    pub feed: FeedConfig,
    pub monitoring: MonitoringConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradingConfig {
    pub initial_usdc: f64,
    pub initial_sol: f64,
    pub mode: TradingMode,
    pub auto_trade: bool,
    pub test_duration_hours: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TradingMode {
    Paper,
    Simulation,
    LiveDevnet,
    LiveMainnet,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridConfig {
    pub spacing_pct: f64,
    pub order_size: f64,
    pub num_buy_orders: usize,
    pub num_sell_orders: usize,
    pub auto_rebalance: bool,
    pub min_usdc_reserve: f64,
    pub min_sol_reserve: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskConfig {
    pub enable_stop_loss: bool,
    pub stop_loss_pct: f64,
    pub enable_take_profit: bool,
    pub take_profit_pct: f64,
    pub max_position_pct: f64,
    pub max_daily_loss_pct: f64,
    pub enable_circuit_breaker: bool,
    pub circuit_breaker_loss_pct: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub feed_type: FeedType,
    pub update_interval_ms: u64,
    pub enable_fallback: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FeedType {
    HTTP,
    WebSocket,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub log_level: String,
    pub status_interval_sec: u64,
    pub save_results: bool,
    pub results_file: String,
}

impl Default for BotConfig {
    fn default() -> Self {
        Self::overnight_test()
    }
}

impl BotConfig {
    /// Load config from TOML file
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config: BotConfig = toml::from_str(&contents)?;
        Ok(config)
    }
    
    /// Save config to TOML file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let toml = toml::to_string_pretty(self)?;
        std::fs::write(path, toml)?;
        Ok(())
    }
    
    /// 🌙 OVERNIGHT TEST - Balanced, long duration, auto-rebalancing
    pub fn overnight_test() -> Self {
        Self {
            test_name: "Overnight Auto-Rebalancing Grid Test".to_string(),
            trading: TradingConfig {
                initial_usdc: 5000.0,
                initial_sol: 10.0,
                mode: TradingMode::Paper,
                auto_trade: true,
                test_duration_hours: 8,
            },
            grid: GridConfig {
                spacing_pct: 0.002,       // 0.2% spacing
                order_size: 0.5,
                num_buy_orders: 15,
                num_sell_orders: 15,
                auto_rebalance: true,
                min_usdc_reserve: 200.0,
                min_sol_reserve: 1.0,
            },
            risk: RiskConfig {
                enable_stop_loss: true,
                stop_loss_pct: 0.05,
                enable_take_profit: true,
                take_profit_pct: 0.10,
                max_position_pct: 0.5,
                max_daily_loss_pct: 0.10,
                enable_circuit_breaker: true,
                circuit_breaker_loss_pct: 0.15,
            },
            feed: FeedConfig {
                feed_type: FeedType::HTTP,
                update_interval_ms: 1000,
                enable_fallback: false,
            },
            monitoring: MonitoringConfig {
                log_level: "info".to_string(),
                status_interval_sec: 1800,  // 30 min updates
                save_results: true,
                results_file: "results/overnight_test.json".to_string(),
            },
        }
    }
    
    /// 🎯 AGGRESSIVE - More fills, tighter grid
    pub fn aggressive() -> Self {
        Self {
            test_name: "Aggressive High-Frequency Test".to_string(),
            trading: TradingConfig {
                initial_usdc: 5000.0,
                initial_sol: 10.0,
                mode: TradingMode::Paper,
                auto_trade: true,
                test_duration_hours: 4,
            },
            grid: GridConfig {
                spacing_pct: 0.0015,      // 0.15% spacing (tighter!)
                order_size: 0.8,
                num_buy_orders: 20,
                num_sell_orders: 20,
                auto_rebalance: true,
                min_usdc_reserve: 100.0,
                min_sol_reserve: 0.5,
            },
            risk: RiskConfig {
                enable_stop_loss: true,
                stop_loss_pct: 0.08,
                enable_take_profit: true,
                take_profit_pct: 0.15,
                max_position_pct: 0.7,
                max_daily_loss_pct: 0.15,
                enable_circuit_breaker: true,
                circuit_breaker_loss_pct: 0.20,
            },
            feed: FeedConfig {
                feed_type: FeedType::HTTP,
                update_interval_ms: 500,   // Faster updates
                enable_fallback: false,
            },
            monitoring: MonitoringConfig {
                log_level: "info".to_string(),
                status_interval_sec: 900,  // 15 min updates
                save_results: true,
                results_file: "results/aggressive_test.json".to_string(),
            },
        }
    }
    
    /// 🛡️ CONSERVATIVE - Lower risk, wider grid
    pub fn conservative() -> Self {
        Self {
            test_name: "Conservative Safe Test".to_string(),
            trading: TradingConfig {
                initial_usdc: 5000.0,
                initial_sol: 10.0,
                mode: TradingMode::Paper,
                auto_trade: true,
                test_duration_hours: 8,
            },
            grid: GridConfig {
                spacing_pct: 0.003,       // 0.3% spacing (wider)
                order_size: 0.3,
                num_buy_orders: 10,
                num_sell_orders: 10,
                auto_rebalance: true,
                min_usdc_reserve: 300.0,
                min_sol_reserve: 2.0,
            },
            risk: RiskConfig {
                enable_stop_loss: true,
                stop_loss_pct: 0.03,
                enable_take_profit: true,
                take_profit_pct: 0.05,
                max_position_pct: 0.3,
                max_daily_loss_pct: 0.05,
                enable_circuit_breaker: true,
                circuit_breaker_loss_pct: 0.08,
            },
            feed: FeedConfig {
                feed_type: FeedType::HTTP,
                update_interval_ms: 2000,  // Slower updates
                enable_fallback: false,
            },
            monitoring: MonitoringConfig {
                log_level: "info".to_string(),
                status_interval_sec: 3600, // 1 hour updates
                save_results: true,
                results_file: "results/conservative_test.json".to_string(),
            },
        }
    }
    
    /// Display configuration
    pub fn display(&self) {
        println!("\n╔═══════════════════════════════════════════════════════════════╗");
        println!("║                                                               ║");
        println!("║          🤖 BOT CONFIGURATION - {}          ", self.test_name);
        println!("║                                                               ║");
        println!("╚═══════════════════════════════════════════════════════════════╝\n");
        
        println!("💰 TRADING SETUP");
        println!("   Capital:       ${:.2} USDC + {:.2} SOL", 
                self.trading.initial_usdc, self.trading.initial_sol);
        println!("   Mode:          {:?}", self.trading.mode);
        println!("   Duration:      {} hours", self.trading.test_duration_hours);
        println!("   Auto-Trade:    {}", if self.trading.auto_trade { "✅ ON" } else { "❌ OFF" });
        
        println!("\n📊 GRID STRATEGY");
        println!("   Spacing:       {}%", self.grid.spacing_pct * 100.0);
        println!("   Order Size:    {} SOL", self.grid.order_size);
        println!("   Buy Orders:    {}", self.grid.num_buy_orders);
        println!("   Sell Orders:   {}", self.grid.num_sell_orders);
        println!("   Rebalancing:   {}", if self.grid.auto_rebalance { "🤖 AUTO" } else { "❌ OFF" });
        println!("   Reserves:      ${:.0} USDC, {:.1} SOL", 
                self.grid.min_usdc_reserve, self.grid.min_sol_reserve);
        
        println!("\n🛡️ RISK MANAGEMENT");
        println!("   Stop-Loss:     {} at -{}%", 
                if self.risk.enable_stop_loss { "✅" } else { "❌" },
                self.risk.stop_loss_pct * 100.0);
        println!("   Take-Profit:   {} at +{}%", 
                if self.risk.enable_take_profit { "✅" } else { "❌" },
                self.risk.take_profit_pct * 100.0);
        println!("   Max Position:  {}% of capital", self.risk.max_position_pct * 100.0);
        println!("   Daily Limit:   -{}%", self.risk.max_daily_loss_pct * 100.0);
        println!("   Breaker:       {} at -{}%", 
                if self.risk.enable_circuit_breaker { "✅" } else { "❌" },
                self.risk.circuit_breaker_loss_pct * 100.0);
        
        println!("\n📡 PRICE FEED");
        println!("   Type:          {:?}", self.feed.feed_type);
        println!("   Update Rate:   {}ms", self.feed.update_interval_ms);
        println!("   Fallback:      {}", if self.feed.enable_fallback { "✅" } else { "❌" });
        
        println!("\n📋 MONITORING");
        println!("   Log Level:     {}", self.monitoring.log_level);
        println!("   Updates:       Every {}s", self.monitoring.status_interval_sec);
        println!("   Save Results:  {}", if self.monitoring.save_results { "✅" } else { "❌" });
        if self.monitoring.save_results {
            println!("   Results File:  {}", self.monitoring.results_file);
        }
        println!();
    }
}
