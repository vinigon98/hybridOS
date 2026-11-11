//! # hyav-core
//!
//! Core primitives, high-throughput Bloom filter, and two-level hierarchical LRU cache
//! for the HybridOS Antivirus Subsystem (HyAV).
//!
//! Designed for low-latency in-memory trust verification and malware rejection.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bloom;
pub mod cache;
pub mod file_id;

// Re-export primary types for ergonomic usage across hyav-* crates
pub use bloom::{
    hash128, BloomError, BloomFilter, DEFAULT_BLOOM_BITS, DEFAULT_BLOOM_K,
};
pub use cache::{
    CacheEntry, CacheKey, CacheStats, LruLevel, ScanVerdict, TwoLevelLruCache,
    L1_DEFAULT_CAPACITY, L2_DEFAULT_CAPACITY,
};
pub use file_id::FileId;
