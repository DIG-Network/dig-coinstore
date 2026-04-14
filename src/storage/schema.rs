//! Column family names and key encoding/decoding helpers.
//!
//! Defines the storage schema: column family name constants, composite key
//! construction functions (puzzle_hash_key, height_coin_key, hint_key, etc.),
//! and key parsing utilities.
//!
//! All key formats use big-endian encoding for natural sort order in
//! lexicographic key-value stores.
//!
//! # Requirements: STO-002, STO-008
//! # Spec: docs/requirements/domains/storage/specs/STO-008.md
//! # SPEC.md: Section 7.2 (column family key/value table)
