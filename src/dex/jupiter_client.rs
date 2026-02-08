//! 🪐 Jupiter Aggregator Client
//! 
//! Cross-DEX trading via Jupiter for best prices across all Solana DEXs.
//! 
//! # Features
//! - Automatic best price routing
//! - Slippage protection
//! - Cross-DEX execution
//! - Position tracking
//! - Statistics monitoring
//! 
//! # Example
//! ```
//! use solana_grid_bot::dex::{JupiterClient, Order, OrderSide, OrderType, Trader};
//! use solana_sdk::signature::Keypair;
//! use solana_sdk::pubkey::Pubkey;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! let wallet = Keypair::new();
//! let sol_mint = Pubkey::new_unique();
//! let usdc_mint = Pubkey::new_unique();
//! 
//! let mut client = JupiterClient::new(
//!     "https://api.mainnet-beta.solana.com".to_string(),
//!     wallet,
//!     sol_mint,
//!     usdc_mint,
//!     1000.0, // Starting capital
//! )?;
//! 
//! let order = Order::new(OrderSide::Bid, 180.0, 1.0, OrderType::Limit);
//! let placed = client.place_order(order).await?;
//! println!("Order placed: {}", placed.order_id);
//! # Ok(())
//! # }
//! ```

use super::{Order, OrderSide, PlacedOrder, Position, Trader};
use anyhow::{Result, bail};
use async_trait::async_trait;
use log::{info, warn, debug};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
};
use std::sync::Arc;
use std::time::SystemTime;

// ═══════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════

/// Default slippage tolerance in basis points (50 = 0.5%)
const DEFAULT_SLIPPAGE_BPS: u16 = 50;

/// Minimum order size (prevents dust orders)
const MIN_ORDER_SIZE: f64 = 0.001;

// ═══════════════════════════════════════════════════════════════════════════
// JUPITER CLIENT
// ═══════════════════════════════════════════════════════════════════════════

/// Jupiter aggregator client for cross-DEX trading
/// 
/// This client provides:
/// - Automatic best price routing across all Solana DEXs
/// - Slippage protection with configurable tolerance
/// - Position tracking and P&L calculation
/// - Order statistics and monitoring
pub struct JupiterClient {
    /// RPC client for Solana blockchain
    #[allow(dead_code)]
    rpc: Arc<RpcClient>,
    
    /// Trading wallet keypair
    wallet: Arc<Keypair>,
    
    /// Base token mint (e.g., SOL)
    base_mint: Pubkey,
    
    /// Quote token mint (e.g., USDC)
    quote_mint: Pubkey,
    
    /// Current position
    position: Position,
    
    /// Total orders placed
    orders_placed: u64,
    
    /// Total orders cancelled
    orders_cancelled: u64,
    
    /// Last order timestamp
    last_order_time: Option<SystemTime>,
    
    /// Slippage tolerance in basis points
    slippage_bps: u16,
}

impl JupiterClient {
    /// Create a new Jupiter aggregator client
    /// 
    /// # Arguments
    /// * `rpc_url` - Solana RPC endpoint URL
    /// * `wallet` - Trading wallet keypair
    /// * `base_mint` - Base token mint address (e.g., SOL)
    /// * `quote_mint` - Quote token mint address (e.g., USDC)
    /// * `initial_capital` - Starting quote currency amount
    /// 
    /// # Returns
    /// * Initialized Jupiter client
    pub fn new(
        rpc_url: String,
        wallet: Keypair,
        base_mint: Pubkey,
        quote_mint: Pubkey,
        initial_capital: f64,
    ) -> Result<Self> {
        info!("🪐 Initializing Jupiter Aggregator client");
        info!("   Base mint:  {}", base_mint);
        info!("   Quote mint: {}", quote_mint);
        info!("   Wallet:     {}", wallet.pubkey());
        info!("   Capital:    ${:.2}", initial_capital);
        
        Ok(Self {
            rpc: Arc::new(RpcClient::new(rpc_url)),
            wallet: Arc::new(wallet),
            base_mint,
            quote_mint,
            position: Position::new(initial_capital),
            orders_placed: 0,
            orders_cancelled: 0,
            last_order_time: None,
            slippage_bps: DEFAULT_SLIPPAGE_BPS,
        })
    }
    
    /// Set custom slippage tolerance
    /// 
    /// # Arguments
    /// * `slippage_bps` - Slippage tolerance in basis points (100 = 1%)
    pub fn with_slippage(mut self, slippage_bps: u16) -> Self {
        self.slippage_bps = slippage_bps;
        info!("   Slippage:   {}bps ({:.2}%)", slippage_bps, slippage_bps as f64 / 100.0);
        self
    }
    
    /// Execute a swap via Jupiter
    /// 
    /// # Arguments
    /// * `input_mint` - Input token mint
    /// * `output_mint` - Output token mint
    /// * `amount` - Amount to swap
    /// * `slippage_bps` - Slippage tolerance
    /// 
    /// # Returns
    /// * Transaction signature
    async fn execute_swap(
        &mut self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: f64,
        slippage_bps: u16,
    ) -> Result<Signature> {
        info!("🔄 Executing Jupiter swap");
        debug!("   Input:    {} ({})", amount, input_mint);
        debug!("   Output:   {}", output_mint);
        debug!("   Slippage: {}bps", slippage_bps);
        
        // TODO: Production implementation
        // 1. Query Jupiter API for best route
        //    GET https://quote-api.jup.ag/v6/quote
        //    ?inputMint={}&outputMint={}&amount={}
        // 2. Get swap transaction from Jupiter
        //    POST https://quote-api.jup.ag/v6/swap
        //    with route data
        // 3. Sign transaction with wallet
        // 4. Send to Solana blockchain
        // 5. Wait for confirmation
        // 6. Parse result from logs
        
        warn!("⚠️  SIMULATION MODE: Jupiter swap not executed");
        warn!("   Would swap {:.4} via Jupiter API", amount);
        warn!("   Using best route across all DEXs");
        
        // Simulate the fill
        self.simulate_fill(input_mint, output_mint, amount)?;
        
        // Update statistics
        self.orders_placed += 1;
        self.last_order_time = Some(SystemTime::now());
        
        info!("✅ Swap executed (simulation)");
        
        Ok(Signature::default())
    }
    
    /// Simulate order fill for testing
    /// 
    /// Updates position based on simulated swap execution
    fn simulate_fill(
        &mut self,
        input_mint: Pubkey,
        output_mint: Pubkey,
        amount: f64,
    ) -> Result<()> {
        // Validate mints
        if input_mint == output_mint {
            anyhow::bail!("Input and output mints cannot be the same");
        }
        
        // Determine trade direction
        let is_buy = output_mint == self.base_mint;
        
        if is_buy {
            // Buying base with quote
            let cost = amount; // Quote amount spent
            let received = amount / 180.0; // Assume SOL price ~$180
            
            debug!("   Buying {:.4} base for ${:.2}", received, cost);
            
            // Update position
            self.position.quote_amount -= cost;
            
            // Update average entry price
            let old_base = self.position.base_amount;
            let old_avg = self.position.avg_entry_price;
            let new_base = old_base + received;
            
            if new_base > 0.0 {
                self.position.avg_entry_price = 
                    ((old_avg * old_base) + (cost / received)) / new_base;
            }
            
            self.position.base_amount = new_base;
            
        } else {
            // Selling base for quote
            let sold = amount; // Base amount sold
            let received = sold * 180.0; // Assume SOL price ~$180
            
            debug!("   Selling {:.4} base for ${:.2}", sold, received);
            
            // Calculate realized P&L
            if self.position.avg_entry_price > 0.0 {
                let pnl = (180.0 - self.position.avg_entry_price) * sold;
                self.position.realized_pnl += pnl;
                debug!("   Realized P&L: ${:+.2}", pnl);
            }
            
            // Update position
            self.position.base_amount -= sold;
            self.position.quote_amount += received;
        }
        
        Ok(())
    }
    
    /// Get trading statistics
    /// 
    /// # Returns
    /// * Tuple of (orders_placed, orders_cancelled)
    pub fn stats(&self) -> (u64, u64) {
        (self.orders_placed, self.orders_cancelled)
    }
    
    /// Display statistics to console
    pub fn display_stats(&self) {
        println!("\n📊 Jupiter Client Statistics:");
        println!("   Orders Placed:    {}", self.orders_placed);
        println!("   Orders Cancelled: {}", self.orders_cancelled);
        
        if let Some(last_time) = self.last_order_time {
            if let Ok(elapsed) = last_time.elapsed() {
                println!("   Last Order:       {}s ago", elapsed.as_secs());
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADER TRAIT IMPLEMENTATION
// ═══════════════════════════════════════════════════════════════════════════

#[async_trait]
impl Trader for JupiterClient {
    async fn place_order(&mut self, order: Order) -> Result<PlacedOrder> {
        info!("📝 Placing {} order via Jupiter", order.side.as_str());
        info!("   Price: ${:.4}", order.price);
        info!("   Size:  {:.4}", order.size);
        info!("   Value: ${:.2}", order.value());
        
        // Validate order
        if order.size < MIN_ORDER_SIZE {
            bail!("Order size too small: {:.6} (min: {})", order.size, MIN_ORDER_SIZE);
        }
        
        order.validate()?;
        
        // Determine swap direction and amount
        let (input_mint, output_mint, amount) = match order.side {
            OrderSide::Bid => {
                // Buying base: Quote → Base
                let quote_amount = order.price * order.size;
                debug!("   Swap: ${:.2} USDC → {:.4} SOL", quote_amount, order.size);
                (self.quote_mint, self.base_mint, quote_amount)
            }
            OrderSide::Ask => {
                // Selling base: Base → Quote
                debug!("   Swap: {:.4} SOL → ${:.2} USDC", order.size, order.value());
                (self.base_mint, self.quote_mint, order.size)
            }
        };
        
        // Execute swap via Jupiter
        let _sig = self.execute_swap(
            input_mint,
            output_mint,
            amount,
            self.slippage_bps,
        ).await?;
        
        // Build placed order
        let placed = PlacedOrder {
            order: order.clone(),
            order_id: self.orders_placed as u128,
            market: self.base_mint, // Use base mint as market ID
            owner: self.wallet.pubkey(),
            timestamp: chrono::Utc::now().timestamp(),
        };
        
        info!("✅ Order placed via Jupiter");
        info!("   Order ID: {}", placed.order_id);
        info!("   Trader:   {}", self.trader_type());
        
        Ok(placed)
    }
    
    async fn cancel_order(&mut self, order_id: u128) -> Result<()> {
        warn!("⚠️  Jupiter orders execute immediately");
        warn!("   Cannot cancel order ID: {}", order_id);
        warn!("   Orders are atomic swaps (no order book)");
        
        // Technically not an error, just not applicable
        self.orders_cancelled += 1;
        
        Ok(())
    }
    
    async fn get_balance(&self) -> Result<(f64, f64)> {
        // TODO: Query actual on-chain token accounts
        // For now, return simulated position balances
        Ok((self.position.base_amount, self.position.quote_amount))
    }
    
    async fn get_position(&self) -> Result<Position> {
        Ok(self.position.clone())
    }
    
    fn trader_type(&self) -> &'static str {
        "Jupiter Aggregator"
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_client() -> JupiterClient {
        let wallet = Keypair::new();
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        
        JupiterClient::new(
            "https://api.devnet.solana.com".to_string(),
            wallet,
            base_mint,
            quote_mint,
            1000.0,
        ).unwrap()
    }
    
    #[test]
    fn test_client_creation() {
        let client = create_test_client();
        assert_eq!(client.orders_placed, 0);
        assert_eq!(client.position.quote_amount, 1000.0);
        assert_eq!(client.slippage_bps, DEFAULT_SLIPPAGE_BPS);
    }
    
    #[test]
    fn test_custom_slippage() {
        let client = create_test_client().with_slippage(100); // 1%
        assert_eq!(client.slippage_bps, 100);
    }
    
    #[test]
    fn test_simulate_buy() {
        let mut client = create_test_client();
        
        // Simulate buying 1 SOL at $180
        let result = client.simulate_fill(
            client.quote_mint,
            client.base_mint,
            180.0, // $180 USDC
        );
        
        assert!(result.is_ok());
        assert!((client.position.base_amount - 1.0).abs() < 0.01);
        assert!((client.position.quote_amount - 820.0).abs() < 0.01);
    }
    
    #[test]
    fn test_simulate_sell() {
        let mut client = create_test_client();
        
        // Setup: Buy first
        client.simulate_fill(client.quote_mint, client.base_mint, 180.0).unwrap();
        
        // Now sell
        let result = client.simulate_fill(
            client.base_mint,
            client.quote_mint,
            1.0, // 1 SOL
        );
        
        assert!(result.is_ok());
        assert!(client.position.base_amount.abs() < 0.01);
        assert!((client.position.quote_amount - 1000.0).abs() < 0.01);
    }
    
    #[test]
    fn test_stats() {
        let client = create_test_client();
        let (placed, cancelled) = client.stats();
        assert_eq!(placed, 0);
        assert_eq!(cancelled, 0);
    }
}
