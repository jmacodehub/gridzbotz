// Add this near the top with other module declarations
pub mod adaptive_optimizer;
pub use adaptive_optimizer::AdaptiveOptimizerConfig;

// Then in the Config struct, add:
// (Insert after metrics field)

/// Adaptive Optimizer configuration
#[serde(default)]
pub adaptive_optimizer: AdaptiveOptimizerConfig,
