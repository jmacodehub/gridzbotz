pub mod bot_trait;
pub mod grid_bot;
pub mod arbitrage_bot;
pub mod momentum_bot;

pub use bot_trait::{Bot, BotStats, TickResult};
pub use grid_bot::GridBot;
//pub use arbitrage_bot::ArbitrageBot;
//pub use momentum_bot::MomentumBot;
