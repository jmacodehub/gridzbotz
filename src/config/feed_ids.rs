//! ═══════════════════════════════════════════════════════════════════════════
//! 🎯 CENTRALIZED PYTH FEED IDS - Single source of truth (2025)
//! ═══════════════════════════════════════════════════════════════════════════

/// All Pyth feed IDs as static constant
pub struct PythFeedIds;

impl PythFeedIds {
    /// All feed IDs as a constant array (zero allocations)
    pub const FEED_IDS: &'static [&'static str] = &[
        "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d", // SOL/USD
        "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43", // BTC/USD
        "0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace", // ETH/USD
    ];

    /// SOL/USD price feed ID
    pub fn sol_usd() -> &'static str {
        Self::FEED_IDS[0]
    }

    /// BTC/USD price feed ID
    pub fn btc_usd() -> &'static str {
        Self::FEED_IDS[1]
    }

    /// ETH/USD price feed ID
    pub fn eth_usd() -> &'static str {
        Self::FEED_IDS[2]
    }

    /// Get all feed IDs as static array reference
    pub fn as_static_array() -> &'static [&'static str] {
        Self::FEED_IDS
    }

    /// Get all feed IDs as Vec<String> for queries
    pub fn as_vec() -> Vec<String> {
        Self::FEED_IDS.iter().map(|id| id.to_string()).collect()
    }

    /// Validate if feed ID exists
    pub fn is_valid(feed_id: &str) -> bool {
        Self::FEED_IDS.contains(&feed_id)
    }

    /// Get human-readable name
    pub fn name_for_id(feed_id: &str) -> Option<&'static str> {
        match feed_id {
            id if id == Self::sol_usd() => Some("SOL/USD"),
            id if id == Self::btc_usd() => Some("BTC/USD"),
            id if id == Self::eth_usd() => Some("ETH/USD"),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feed_ids_valid() {
        for id in PythFeedIds::FEED_IDS {
            assert_eq!(id.len(), 66);
            assert!(id.starts_with("0x"));
        }
    }

    #[test]
    fn test_as_vec() {
        let vec = PythFeedIds::as_vec();
        assert_eq!(vec.len(), 3);
    }
}
