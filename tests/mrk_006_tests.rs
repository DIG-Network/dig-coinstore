//! # MRK-006 — `coin_record_hash`: deterministic SMT leaf digest for [`dig_coinstore::types::CoinRecord`]
//!
//! **Normative:** [`MRK-006`](../../docs/requirements/domains/merkle/NORMATIVE.md#MRK-006)  
//! **Spec + test plan:** [`MRK-006.md`](../../docs/requirements/domains/merkle/specs/MRK-006.md)  
//! **Serialization contract:** [`STO-008`](../../docs/requirements/domains/storage/specs/STO-008.md) (fixed-width integers, big-endian)  
//! **Consumer:** [`MRK-001`](../../docs/requirements/domains/merkle/specs/MRK-001.md) — leaf **values** in [`dig_coinstore::merkle::SparseMerkleTree`] are these 32-byte digests keyed by `coin_id`.
//!
//! ## What MRK-006 mandates (and how we prove it)
//!
//! | MRK-006 rule | Evidence in this file |
//! |--------------|------------------------|
//! | `coin_record_hash` returns [`Bytes32`] | every test calls it and compares to literals or second call |
//! | `SHA256(bincode_STO008(record))` | [`vv_req_mrk_006_matches_sto008_preimage`] decodes the preimage with [`dig_coinstore::storage::kv_bincode::decode_coin_record`] |
//! | Same record → same hash (determinism) | [`vv_req_mrk_006_deterministic`] |
//! | Any field change → different hash | [`vv_req_mrk_006_mutation_sensitivity`], timestamp branch in same test |
//! | Unspent vs spent differs | [`vv_req_mrk_006_spent_vs_unspent`], [`vv_req_mrk_006_spent_height_zero_vs_unspent`] |
//! | `coinbase` flips digest | [`vv_req_mrk_006_coinbase_sensitivity`] |
//! | Golden vector (portability lock) | [`vv_req_mrk_006_cross_platform_vector`] — hex frozen from STO-008 + `chia_sha2` on 2026-04-14 |
//! | Leaf value used in `batch_insert` | [`vv_req_mrk_006_used_in_tree`] — `tree.get(&id) == Some(&coin_record_hash(&rec))` |
//! | MRK-006 ≠ generic `merkle_leaf_hash` | [`vv_req_mrk_006_not_domain_wrapped_leaf_hash`] — MRK-006 is raw SHA256 of bytes; MRK-001 internal nodes still use domain bytes |
//!
//! **SocratiCode:** MCP not wired for this workspace; discovery used spec + `src/merkle/leaf_hash.rs` + Repomix packs per `docs/prompt/start.md`.  
//! **GitNexus:** `npx gitnexus status` before edits; `impact coin_record_hash` before publishing the symbol; `analyze` after commit.

use chia_protocol::Bytes32;
use dig_coinstore::merkle::{coin_record_hash, merkle_leaf_hash, SparseMerkleTree};
use dig_coinstore::storage::kv_bincode;
use dig_coinstore::types::CoinRecord;
use dig_coinstore::Coin;

/// Canonical fixture for the **golden vector** test ([`vv_req_mrk_006_cross_platform_vector`]).
///
/// Changing any field without updating `MRK006_GOLDEN_BYTES` must fail CI — that is the regression
/// signal MRK-006 §Test Plan calls `test_coin_record_hash_cross_platform_vector`.
fn mrk006_golden_fixture() -> CoinRecord {
    let parent = Bytes32::from([0x01u8; 32]);
    let puzzle = Bytes32::from([0x02u8; 32]);
    let coin = Coin::new(parent, puzzle, 1_234_567);
    CoinRecord::new(coin, 99, 1_600_000_000, false)
}

/// SHA256(STO-008(`mrk006_golden_fixture()`))) — frozen when MRK-006 landed (`cargo test` one-shot print).
const MRK006_GOLDEN_BYTES: [u8; 32] = [
    0x46, 0x28, 0xe1, 0xfe, 0x7d, 0x46, 0x4c, 0x70, 0x3a, 0x0c, 0xdd, 0xf8, 0x43, 0x2d, 0xfa, 0x5f,
    0xf3, 0xe5, 0x1a, 0xc7, 0xe3, 0x4e, 0xc9, 0xa7, 0xac, 0x62, 0x2a, 0x89, 0xfc, 0xed, 0x3c, 0xed,
];

/// MRK-006 / test plan `test_coin_record_hash_deterministic`.
#[test]
fn vv_req_mrk_006_deterministic() {
    let r = mrk006_golden_fixture();
    assert_eq!(coin_record_hash(&r), coin_record_hash(&r));
}

/// MRK-006 / `test_coin_record_hash_changes_on_mutation` — field-level preimage sensitivity.
#[test]
fn vv_req_mrk_006_mutation_sensitivity() {
    let base = mrk006_golden_fixture();
    let h0 = coin_record_hash(&base);

    let mut alt = base.clone();
    alt.confirmed_height = 100;
    assert_ne!(
        coin_record_hash(&alt),
        h0,
        "confirmed_height is part of the preimage"
    );

    let mut alt = base.clone();
    alt.timestamp = 1_600_000_001;
    assert_ne!(
        coin_record_hash(&alt),
        h0,
        "MRK-006 implementation notes: timestamp must affect digest"
    );

    let mut alt = base.clone();
    alt.ff_eligible = true;
    assert_ne!(
        coin_record_hash(&alt),
        h0,
        "ff_eligible is serialized (STO-008 row layout)"
    );

    let mut alt = base;
    alt.coin = Coin::new(
        Bytes32::from([0x01u8; 32]),
        Bytes32::from([0x02u8; 32]),
        1_234_568,
    );
    assert_ne!(
        coin_record_hash(&alt),
        h0,
        "coin amount is in the embedded `Coin` struct"
    );
}

/// MRK-006 / `test_coin_record_hash_spent_vs_unspent`.
#[test]
fn vv_req_mrk_006_spent_vs_unspent() {
    let mut spent = mrk006_golden_fixture();
    spent.spend(100);
    let unspent = {
        let mut u = spent.clone();
        u.spent_height = None;
        u
    };
    assert_ne!(
        coin_record_hash(&spent),
        coin_record_hash(&unspent),
        "spend transition must change the leaf digest for MRK-001 batch_update"
    );
}

/// MRK-006 edge note: `spent_height = Some(0)` is not the same row as unspent (`None`) for our model.
#[test]
fn vv_req_mrk_006_spent_height_zero_vs_unspent() {
    let a = mrk006_golden_fixture();
    let mut b = a.clone();
    b.spent_height = Some(0);
    assert_ne!(
        coin_record_hash(&a),
        coin_record_hash(&b),
        "Option discriminant + u64 payload must distinguish unspent from height-0 spent encoding"
    );
}

/// MRK-006 / `test_coin_record_hash_coinbase_sensitivity`.
#[test]
fn vv_req_mrk_006_coinbase_sensitivity() {
    let mut cb = mrk006_golden_fixture();
    cb.coinbase = true;
    assert_ne!(
        coin_record_hash(&cb),
        coin_record_hash(&mrk006_golden_fixture()),
        "coinbase flag is serialized as its own field"
    );
}

/// MRK-006 / `test_coin_record_hash_cross_platform_vector`.
#[test]
fn vv_req_mrk_006_cross_platform_vector() {
    assert_eq!(
        coin_record_hash(&mrk006_golden_fixture()),
        Bytes32::from(MRK006_GOLDEN_BYTES)
    );
}

/// MRK-006 preimage is exactly **STO-008** bytes: re-decoding the serialized blob round-trips the struct.
#[test]
fn vv_req_mrk_006_matches_sto008_preimage() {
    let r = mrk006_golden_fixture();
    let bytes = kv_bincode::encode_coin_record(&r).expect("fixture must encode");
    let round = kv_bincode::decode_coin_record(&bytes).expect("STO-008 strict decode");
    assert_eq!(round, r);
    // Independent SHA256 over those bytes must match `coin_record_hash` (same contract as production).
    let mut hasher = chia_sha2::Sha256::new();
    hasher.update(&bytes);
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    assert_eq!(coin_record_hash(&r), Bytes32::from(out));
}

/// MRK-006 leaf digest is **not** `merkle_leaf_hash(STO-008 bytes)` — the latter adds MRK-001/MRK-002’s `0x00` domain prefix.
#[test]
fn vv_req_mrk_006_not_domain_wrapped_leaf_hash() {
    let r = mrk006_golden_fixture();
    let raw = kv_bincode::encode_coin_record(&r).unwrap();
    assert_ne!(
        coin_record_hash(&r),
        merkle_leaf_hash(&raw),
        "MRK-006: raw SHA256 preimage; MRK-001 stores that digest as the SMT leaf value, not H(0x00||bytes)"
    );
}

/// MRK-006 / `test_coin_record_hash_used_in_tree` — the value slot MRK-001 exposes must be the MRK-006 digest.
#[test]
fn vv_req_mrk_006_used_in_tree() {
    let rec = mrk006_golden_fixture();
    let key = rec.coin_id();
    let leaf = coin_record_hash(&rec);
    let mut tree = SparseMerkleTree::new();
    tree.batch_insert(&[(key, leaf)]).unwrap();
    assert_eq!(tree.get(&key), Some(&leaf));
    let _ = tree.root();
}

/// MRK-006 usage in [`SparseMerkleTree::batch_update`] (MRK-006 §Usage / acceptance “batch_update”).
#[test]
fn vv_req_mrk_006_batch_update_rehashed_leaf() {
    let mut rec = mrk006_golden_fixture();
    let key = rec.coin_id();
    let mut tree = SparseMerkleTree::new();
    tree.batch_insert(&[(key, coin_record_hash(&rec))]).unwrap();
    rec.spend(7);
    let new_leaf = coin_record_hash(&rec);
    tree.batch_update(&[(key, new_leaf)]).unwrap();
    assert_eq!(tree.get(&key), Some(&new_leaf));
}
