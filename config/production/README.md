# Production Configs - Project Flash V3.5

## Overview
Based on overnight test results (Oct 17-18, 2025), these are production-ready configurations.

## Configs

### 1. Ultra Aggressive (`ultra_aggressive.toml`)
- **Spacing:** 0.03%
- **Test Results:** 80 trades @ 8.6/hour
- **Use Case:** High-frequency trading, volatile markets
- **Capital Allocation:** 10-15% (risk capital)
- **Fees:** ~$3.20 per 9.3 hours

### 2. Balanced (`balanced.toml`)
- **Spacing:** 0.15%
- **Test Results:** 14 trades @ 1.5/hour
- **Use Case:** Primary strategy, all market conditions
- **Capital Allocation:** 50% (core holdings)
- **Fees:** ~$0.56 per 9.3 hours

### 3. Conservative (`conservative.toml`)
- **Spacing:** 0.30%
- **Test Results:** 7 trades @ 0.8/hour
- **Use Case:** Capital preservation, safe base
- **Capital Allocation:** 30-40% (safety net)
- **Fees:** ~$0.28 per 9.3 hours

## Validation Status
- [ ] 24-hour test (In Progress)
- [ ] 3-day continuous test
- [ ] Real market (devnet) test
- [ ] Production deployment

## Last Updated
October 18, 2025