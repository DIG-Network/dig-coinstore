# Storage — Normative Requirements

> **Master spec:** [SPEC.md](../../../resources/SPEC.md)

---

## &sect;1 Trait Abstraction

<a id="STO-001"></a>**STO-001** A `StorageBackend` trait MUST define `get`, `put`, `delete`, `batch_write`, `prefix_scan`, `flush`, and `compact` operations. All storage backends MUST implement this trait.
> **Spec:** [`STO-001.md`](specs/STO-001.md)

---

## &sect;2 Backend Implementations

<a id="STO-002"></a>**STO-002** The RocksDB backend MUST use 12 column families: `coin_records`, `coin_by_puzzle_hash`, `unspent_by_puzzle_hash`, `coin_by_parent`, `coin_by_confirmed_height`, `coin_by_spent_height`, `hints`, `hints_by_value`, `merkle_nodes`, `archive_coin_records`, `state_snapshots`, and `metadata`.
> **Spec:** [`STO-002.md`](specs/STO-002.md)

<a id="STO-003"></a>**STO-003** The LMDB backend MUST use 6 named databases: `coins`, `coins_by_ph`, `hints`, `hints_by_value`, `snapshots`, and `metadata`.
> **Spec:** [`STO-003.md`](specs/STO-003.md)

---

## &sect;3 Bloom Filter Configuration

<a id="STO-004"></a>**STO-004** Full bloom filters (10 bits/key) MUST be configured for point-lookup column families. Prefix bloom filters (32 bytes) MUST be configured for puzzle-hash column families. Sequential-access column families MUST NOT use bloom filters.
> **Spec:** [`STO-004.md`](specs/STO-004.md)

---

## &sect;4 Atomic Writes

<a id="STO-005"></a>**STO-005** All writes for a single block MUST be committed atomically in one `WriteBatch` with a single WAL fsync. Partial block writes MUST NOT be visible to readers.
> **Spec:** [`STO-005.md`](specs/STO-005.md)

---

## &sect;5 Compaction Strategy

<a id="STO-006"></a>**STO-006** Level compaction SHOULD be used for read-heavy column families. FIFO compaction SHOULD be used for append-only column families.
> **Spec:** [`STO-006.md`](specs/STO-006.md)

---

## &sect;6 Feature Gates

<a id="STO-007"></a>**STO-007** Storage backends MUST be gated behind feature flags: `lmdb-storage`, `rocksdb-storage`, and `full-storage`. The default feature MUST be `rocksdb-storage`.
> **Spec:** [`STO-007.md`](specs/STO-007.md)

---

## &sect;7 Serialization

<a id="STO-008"></a>**STO-008** `CoinRecord` and snapshot data MUST be serialized using bincode. Key encoding helpers MUST be provided in `schema.rs`.
> **Spec:** [`STO-008.md`](specs/STO-008.md)
