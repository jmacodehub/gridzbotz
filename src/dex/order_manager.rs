//! 📋 Order Management System
//! 
//! Tracks active orders, manages lifecycle, handles fills

use super::{PlacedOrder, OrderSide};
use std::collections::HashMap;
use anyhow::Result;
use log::{info, debug};

pub struct OrderManager {
    active_orders: HashMap<u128, PlacedOrder>,
    filled_orders: Vec<PlacedOrder>,
    cancelled_orders: Vec<PlacedOrder>,
}

impl OrderManager {
    pub fn new() -> Self {
        info!("📋 Initializing Order Manager");
        Self {
            active_orders: HashMap::new(),
            filled_orders: Vec::new(),
            cancelled_orders: Vec::new(),
        }
    }
    
    /// Add a new order to tracking
    pub fn add_order(&mut self, order: PlacedOrder) {
        let order_id = order.order_id;
        self.active_orders.insert(order_id, order);
        info!("➕ Tracking new order: {}", order_id);
        debug!("   Active orders: {}", self.active_orders.len());
    }
    
    /// Mark an order as filled
    pub fn mark_filled(&mut self, order_id: u128) -> Result<()> {
        if let Some(order) = self.active_orders.remove(&order_id) {
            info!("✅ Order filled: {}", order_id);
            self.filled_orders.push(order);
            Ok(())
        } else {
            anyhow::bail!("Order not found: {}", order_id)
        }
    }
    
    /// Cancel an active order
    pub fn cancel_order(&mut self, order_id: u128) -> Result<()> {
        if let Some(order) = self.active_orders.remove(&order_id) {
            info!("❌ Order cancelled: {}", order_id);
            self.cancelled_orders.push(order);
            Ok(())
        } else {
            anyhow::bail!("Order not found: {}", order_id)
        }
    }
    
    /// Get all active buy orders
    pub fn get_active_bids(&self) -> Vec<&PlacedOrder> {
        self.active_orders
            .values()
            .filter(|o| matches!(o.order.side, OrderSide::Bid))
            .collect()
    }
    
    /// Get all active sell orders
    pub fn get_active_asks(&self) -> Vec<&PlacedOrder> {
        self.active_orders
            .values()
            .filter(|o| matches!(o.order.side, OrderSide::Ask))
            .collect()
    }
    
    /// Get statistics
    pub fn stats(&self) -> OrderStats {
        OrderStats {
            active: self.active_orders.len(),
            filled: self.filled_orders.len(),
            cancelled: self.cancelled_orders.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct OrderStats {
    pub active: usize,
    pub filled: usize,
    pub cancelled: usize,
}
