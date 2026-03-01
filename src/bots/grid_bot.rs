// ... (entire file identical until the #[cfg(test)] mod at the end) ...

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;

    /// Smoke test: GridBot::new() must succeed with a valid default config.
    /// Exercises the full construction path: config validation → strategy wiring → bot init.
    #[test]
    fn test_bot_creation_smoke() {
        let config = ConfigBuilder::new()
            .build()
            .expect("ConfigBuilder::new().build() must produce a valid config");
        let result = GridBot::new(config);
        assert!(
            result.is_ok(),
            "GridBot::new() failed with valid config: {:?}",
            result.err()
        );
    }
}
