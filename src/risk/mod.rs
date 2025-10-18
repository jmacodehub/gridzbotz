//! 🛡️ Risk Management System
//! 
//! Features:
//! - Position sizing (Kelly Criterion)
//! - Stop-loss & take-profit
//! - Circuit breakers
//! - Drawdown protection

pub mod position_sizer;
pub mod stop_loss;
pub mod circuit_breaker;

pub use position_sizer::PositionSizer;
pub use stop_loss::StopLossManager;
pub use circuit_breaker::CircuitBreaker;
