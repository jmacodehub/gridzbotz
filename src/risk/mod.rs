//! 🛡️ Risk Management System
//! 
//! Features:
//! - Position sizing (Kelly Criterion)
//! - Stop-loss & take-profit
//! - Circuit breakers
//! - Drawdown protection
//! - Win rate guard (PR #131 C4)

pub mod position_sizer;
pub mod stop_loss;
pub mod circuit_breaker;
pub mod win_rate_guard;

pub use position_sizer::PositionSizer;
pub use stop_loss::StopLossManager;
pub use circuit_breaker::CircuitBreaker;
pub use win_rate_guard::WinRateGuard;
