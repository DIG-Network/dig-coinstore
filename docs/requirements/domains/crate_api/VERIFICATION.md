# Crate API — Verification

| ID | Status | Summary | Verification Approach |
|----|--------|---------|----------------------|
| [API-001](NORMATIVE.md#API-001) | -- | CoinStore constructors | Tests: new(path) compiles and returns Result, with_config() respects custom config, init_genesis() bootstraps chain, empty on construction, height()==0 after genesis, duplicate genesis returns GenesisAlreadyInitialized. |
| [API-002](NORMATIVE.md#API-002) | -- | CoinRecord struct | Tests: all 6 fields accessible, new() creates unspent record, is_spent() false for new/true after spend(), spend() sets spent_height, coin_id() matches Coin::coin_id(), to_coin_state() produces correct CoinState, Debug/Clone/Serialize/Deserialize derives. |
| [API-003](NORMATIVE.md#API-003) | -- | CoinStoreConfig builder | Tests: all defaults match spec table, builder chaining, preserves unset fields, 8 with_* methods, StorageBackend enum variants, Default trait, CoinStore integration. |
| [API-004](NORMATIVE.md#API-004) | -- | CoinStoreError enum | Tests: all 15 variants constructible, Clone round-trip, PartialEq equality/inequality, Display formatting via thiserror, Error trait, structured data types correct. |
| [API-005](NORMATIVE.md#API-005) | -- | BlockData and CoinAddition | Tests: all BlockData fields accessible, all CoinAddition fields accessible, additions Vec<CoinAddition>, removals Vec<CoinId>, hints Vec<(CoinId, Bytes32)>, expected_state_root Option. |
| [API-006](NORMATIVE.md#API-006) | -- | ApplyBlockResult and RollbackResult | Tests: ApplyBlockResult all 4 fields, RollbackResult all 4 fields, modified_coins HashMap type, apply_block returns ApplyBlockResult, rollback_to_block returns RollbackResult. |
| [API-007](NORMATIVE.md#API-007) | -- | CoinStoreStats struct | Tests: all 9 fields accessible, stats() on fresh store returns zeros/defaults, stats() after apply_block reflects new state, Clone+Debug. |
| [API-008](NORMATIVE.md#API-008) | -- | CoinStoreSnapshot struct | Tests: all 8 fields accessible, Serialize round-trip, Deserialize round-trip, snapshot() produces valid CoinStoreSnapshot, restore() from snapshot, coins HashMap type, hints Vec type. |
| [API-009](NORMATIVE.md#API-009) | -- | Type aliases and lineage info | Tests: CoinId alias equals Bytes32, PuzzleHash alias equals Bytes32, aliases used consistently in API, UnspentLineageInfo struct fields accessible (coin_id, parent_id, parent_parent_id). |
| [API-010](NORMATIVE.md#API-010) | -- | RollbackAboveTip and is_unspent | Tests: RollbackAboveTip variant constructible with target/current, is_unspent() returns true for unspent coin, is_unspent() returns false for spent/missing coin, is_unspent() is O(1) lock-free. |

**Status legend:** -- draft · ⚠️ partial · ❌ gap
