use solana_grid_bot::{Config, risk::*};

#[test]
fn test_position_sizer() {
    let config = Config::load().expect("Config should load");
    let sizer = PositionSizer::new(&config);

    let size = sizer.calculate_size(100.0, 0.02, 0.6);
    assert!(size > 0.0);
    assert!(size < 100.0); // Should be reasonable

    // Test validation
    assert!(sizer.validate_size(size, 100.0).is_ok());
    assert!(sizer.validate_size(0.0001, 100.0).is_err()); // Too small
}

#[test]
fn test_stop_loss_trigger() {
    let config = Config::load().expect("Config should load");
    let mut sl_manager = StopLossManager::new(&config);

    let entry = 100.0;
    let loss_price = 95.0; // -5% loss

    // Should trigger at 5% loss (default)
    assert!(sl_manager.should_stop_loss(entry, loss_price));

    // Should not trigger at small loss
    assert!(!sl_manager.should_stop_loss(entry, 99.5));
}

#[test]
fn test_circuit_breaker() {
    let config = Config::load().expect("Config should load");
    let mut breaker = CircuitBreaker::new(&config);

    // Should allow trading initially
    assert!(breaker.is_trading_allowed());

    // Record 5 consecutive losses (as percentages, not dollar amounts)
    // PnL must be passed as percentage to match the unit test convention
    let mut balance = 10000.0;
    for _ in 0..5 {
        balance -= 10.0;
        breaker.record_trade(-0.1, balance); // -0.1% loss
    }

    // Should trip after max consecutive losses
    assert!(!breaker.is_trading_allowed());
}
