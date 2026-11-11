//! Two-Level Hierarchical LRU Cache for HyAV
//!
//! Provides ultra-low latency (<250ns) hit paths for trusted/clean verdicts.
//! Level 1 (L1): Hot memory cache for active execution paths.
//! Level 2 (L2): Warm secondary cache for recently evaluated files.
//!
//! Indexing is composed of:
//! - `FileId` (128-bit: volume_guid + inode)
//! - `mtime_ns`: 64-bit nanosecond timestamp of last file modification
//! - `size`: 64-bit file size in bytes

use std::collections::{HashMap, HashSet};
use std::fmt;
use crate::file_id::FileId;

/// Default capacity for Level 1 (hot) cache.
pub const L1_DEFAULT_CAPACITY: usize = 2048;

/// Default capacity for Level 2 (warm) cache.
pub const L2_DEFAULT_CAPACITY: usize = 32768;

/// Primary cache key composite: (FileId, mtime_ns, size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CacheKey {
    /// 128-bit filesystem object identifier.
    pub file_id: FileId,
    /// Modification timestamp in nanoseconds.
    pub mtime_ns: u64,
    /// File size in bytes.
    pub size: u64,
}

impl CacheKey {
    /// Creates a new `CacheKey`.
    #[inline]
    pub const fn new(file_id: FileId, mtime_ns: u64, size: u64) -> Self {
        Self {
            file_id,
            mtime_ns,
            size,
        }
    }

    /// Convenience constructor from raw parts.
    #[inline]
    pub const fn from_parts(volume_guid: u64, inode: u64, mtime_ns: u64, size: u64) -> Self {
        Self {
            file_id: FileId::new(volume_guid, inode),
            mtime_ns,
            size,
        }
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Key({}; mtime={}ns; size={}B)",
            self.file_id, self.mtime_ns, self.size
        )
    }
}

/// Standard HyAV verdict classifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanVerdict {
    /// Clean file, no malware patterns detected.
    Clean,
    /// Malicious pattern or signature matched.
    Malicious,
    /// Suspicious behavior or heuristics detected.
    Suspicious,
    /// Explicitly whitelisted or signed by trusted root vendor.
    Whitelisted,
}

/// Cache entry wrapping a scan verdict with metadata and TTL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheEntry<V = ScanVerdict> {
    /// The verdict payload.
    pub verdict: V,
    /// Monotonic timestamp in nanoseconds when this entry was evaluated.
    pub cached_at_ns: u64,
    /// Time-To-Live in seconds. 0 indicates non-expiring until evicted/invalidated.
    pub ttl_secs: u32,
    /// Cryptographic SHA-256 digest of inspected file content.
    pub sha256: [u8; 32],
}

impl<V> CacheEntry<V> {
    /// Constructs a new cache entry.
    pub fn new(verdict: V, cached_at_ns: u64, ttl_secs: u32, sha256: [u8; 32]) -> Self {
        Self {
            verdict,
            cached_at_ns,
            ttl_secs,
            sha256,
        }
    }

    /// Checks if this entry has expired given current time in nanoseconds.
    #[inline]
    pub fn is_expired(&self, current_time_ns: u64) -> bool {
        if self.ttl_secs == 0 {
            return false; // TTL of 0 means permanent until evicted or invalidated
        }
        let ttl_ns = (self.ttl_secs as u64).saturating_mul(1_000_000_000);
        current_time_ns > self.cached_at_ns.saturating_add(ttl_ns)
    }
}

/// Node inside the 100% safe arena-backed doubly linked list.
struct LruNode<K, V> {
    key: K,
    value: V,
    prev: Option<usize>,
    next: Option<usize>,
}

/// Fast single-level LRU cache in 100% safe Rust.
pub struct LruLevel<K, V> {
    capacity: usize,
    map: HashMap<K, usize>,
    nodes: Vec<Option<LruNode<K, V>>>,
    free_list: Vec<usize>,
    head: Option<usize>, // MRU
    tail: Option<usize>, // LRU
}

impl<K: Eq + std::hash::Hash + Clone, V> LruLevel<K, V> {
    /// Creates a new LRU level with given capacity.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Capacity must be greater than zero");
        Self {
            capacity,
            map: HashMap::with_capacity(capacity),
            nodes: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            head: None,
            tail: None,
        }
    }

    /// Number of items stored in this LRU level.
    #[inline]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Returns true if this LRU level is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Maximum capacity of this LRU level.
    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Returns true if the key is present in this level.
    #[inline]
    pub fn contains_key(&self, key: &K) -> bool {
        self.map.contains_key(key)
    }

    /// Peeks at a key without modifying LRU access order.
    pub fn peek(&self, key: &K) -> Option<&V> {
        let &idx = self.map.get(key)?;
        self.nodes[idx].as_ref().map(|n| &n.value)
    }

    /// Retrieves reference to value and promotes key to MRU (head).
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let &idx = self.map.get(key)?;
        self.move_to_head(idx);
        self.nodes[idx].as_mut().map(|n| &mut n.value)
    }

    /// Inserts key and value. If full, evicts and returns the LRU item `(K, V)`.
    pub fn insert(&mut self, key: K, value: V) -> (Option<V>, Option<(K, V)>) {
        if let Some(&idx) = self.map.get(&key) {
            // Already present: replace value and move to MRU
            self.move_to_head(idx);
            let old_val = core::mem::replace(&mut self.nodes[idx].as_mut().unwrap().value, value);
            return (Some(old_val), None);
        }

        // Check if eviction is needed before inserting new key
        let evicted = if self.map.len() >= self.capacity {
            self.pop_lru()
        } else {
            None
        };

        // Allocate slot
        let node_idx = match self.free_list.pop() {
            Some(i) => {
                self.nodes[i] = Some(LruNode {
                    key: key.clone(),
                    value,
                    prev: None,
                    next: None,
                });
                i
            }
            None => {
                let i = self.nodes.len();
                self.nodes.push(Some(LruNode {
                    key: key.clone(),
                    value,
                    prev: None,
                    next: None,
                }));
                i
            }
        };

        self.attach_head(node_idx);
        self.map.insert(key, node_idx);
        (None, evicted)
    }

    /// Removes and returns the LRU item (tail of the list).
    pub fn pop_lru(&mut self) -> Option<(K, V)> {
        let tail_idx = self.tail?;
        self.detach(tail_idx);
        let node = self.nodes[tail_idx].take().expect("Valid node");
        self.free_list.push(tail_idx);
        self.map.remove(&node.key);
        Some((node.key, node.value))
    }

    /// Explicitly removes an item by key.
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let idx = self.map.remove(key)?;
        self.detach(idx);
        let node = self.nodes[idx].take().expect("Valid node");
        self.free_list.push(idx);
        Some(node.value)
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.map.clear();
        self.nodes.clear();
        self.free_list.clear();
        self.head = None;
        self.tail = None;
    }

    fn detach(&mut self, idx: usize) {
        let (prev_opt, next_opt) = {
            let n = self.nodes[idx].as_ref().expect("Valid node");
            (n.prev, n.next)
        };

        match prev_opt {
            Some(p) => self.nodes[p].as_mut().expect("Valid prev").next = next_opt,
            None => self.head = next_opt,
        }

        match next_opt {
            Some(n) => self.nodes[n].as_mut().expect("Valid next").prev = prev_opt,
            None => self.tail = prev_opt,
        }

        let n = self.nodes[idx].as_mut().expect("Valid node");
        n.prev = None;
        n.next = None;
    }

    fn attach_head(&mut self, idx: usize) {
        {
            let n = self.nodes[idx].as_mut().expect("Valid node");
            n.prev = None;
            n.next = self.head;
        }

        match self.head {
            Some(old_head) => self.nodes[old_head].as_mut().expect("Valid head").prev = Some(idx),
            None => self.tail = Some(idx),
        }
        self.head = Some(idx);
    }

    fn move_to_head(&mut self, idx: usize) {
        if self.head == Some(idx) {
            return;
        }
        self.detach(idx);
        self.attach_head(idx);
    }
}

/// Statistics and telemetry for the Two-Level Cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheStats {
    /// Number of hits resolved directly in Level 1 hot cache.
    pub l1_hits: u64,
    /// Number of hits resolved in Level 2 warm cache.
    pub l2_hits: u64,
    /// Number of lookups that missed both L1 and L2.
    pub misses: u64,
    /// Number of items demoted from L1 to L2.
    pub l1_demotions: u64,
    /// Number of items evicted from L2 (dropped).
    pub l2_evictions: u64,
    /// Number of entries cleared via file invalidation.
    pub invalidations: u64,
    /// Current count of entries in L1.
    pub l1_len: usize,
    /// Current count of entries in L2.
    pub l2_len: usize,
    /// Maximum capacity of L1.
    pub l1_capacity: usize,
    /// Maximum capacity of L2.
    pub l2_capacity: usize,
}

impl CacheStats {
    /// Total successful hits across L1 and L2.
    #[inline]
    pub fn total_hits(&self) -> u64 {
        self.l1_hits + self.l2_hits
    }

    /// Total lookup attempts (hits + misses).
    #[inline]
    pub fn total_lookups(&self) -> u64 {
        self.total_hits() + self.misses
    }

    /// Overall hit ratio between 0.0 and 1.0.
    #[inline]
    pub fn hit_ratio(&self) -> f64 {
        let total = self.total_lookups();
        if total == 0 {
            0.0
        } else {
            (self.total_hits() as f64) / (total as f64)
        }
    }

    /// Level 1 fast hit ratio between 0.0 and 1.0.
    #[inline]
    pub fn l1_hit_ratio(&self) -> f64 {
        let total = self.total_lookups();
        if total == 0 {
            0.0
        } else {
            (self.l1_hits as f64) / (total as f64)
        }
    }
}

/// Two-Level Hierarchical LRU Cache (L1 Hot + L2 Warm).
///
/// Features:
/// - L1 Hot cache: Ultra-fast hit path.
/// - L2 Warm cache: On L1 overflow, items demote to L2.
/// - Promotion: Hit in L2 automatically promotes the item back into L1.
/// - Secondary Index: Fast `O(1)` invalidation of all cached keys for any `FileId` upon file write.
pub struct TwoLevelLruCache<V = CacheEntry<ScanVerdict>> {
    l1: LruLevel<CacheKey, V>,
    l2: LruLevel<CacheKey, V>,
    file_index: HashMap<FileId, HashSet<CacheKey>>,
    l1_hits: u64,
    l2_hits: u64,
    misses: u64,
    l1_demotions: u64,
    l2_evictions: u64,
    invalidations: u64,
}

impl<V: Clone> TwoLevelLruCache<V> {
    /// Creates a cache with default capacities (L1 = 2048, L2 = 32768).
    pub fn new_default() -> Self {
        Self::new(L1_DEFAULT_CAPACITY, L2_DEFAULT_CAPACITY)
    }

    /// Creates a cache with custom capacities.
    pub fn new(l1_capacity: usize, l2_capacity: usize) -> Self {
        Self {
            l1: LruLevel::new(l1_capacity),
            l2: LruLevel::new(l2_capacity),
            file_index: HashMap::new(),
            l1_hits: 0,
            l2_hits: 0,
            misses: 0,
            l1_demotions: 0,
            l2_evictions: 0,
            invalidations: 0,
        }
    }

    /// Queries the cache for a given key.
    ///
    /// 1. If present in L1: increments `l1_hits`, promotes within L1, returns clone.
    /// 2. If present in L2: increments `l2_hits`, removes from L2 and promotes to L1.
    ///    Any item evicted from L1 is demoted down to L2. Returns clone.
    /// 3. If missing: increments `misses`, returns `None`.
    pub fn get(&mut self, key: &CacheKey) -> Option<V> {
        // Check L1
        if let Some(val) = self.l1.get_mut(key) {
            self.l1_hits += 1;
            return Some(val.clone());
        }

        // Check L2
        if let Some(val) = self.l2.remove(key) {
            self.l2_hits += 1;
            // Promote from L2 to L1
            let (_, demoted) = self.l1.insert(*key, val.clone());
            if let Some((demoted_key, demoted_val)) = demoted {
                self.l1_demotions += 1;
                let (_, l2_evicted) = self.l2.insert(demoted_key, demoted_val);
                if let Some((dropped_key, _)) = l2_evicted {
                    self.l2_evictions += 1;
                    self.untrack_file_key(&dropped_key);
                }
            }
            return Some(val);
        }

        self.misses += 1;
        None
    }

    /// Peeks at the value without altering LRU order.
    pub fn peek(&self, key: &CacheKey) -> Option<&V> {
        if let Some(val) = self.l1.peek(key) {
            return Some(val);
        }
        self.l2.peek(key)
    }

    /// Checks if a key exists in either L1 or L2.
    #[inline]
    pub fn contains_key(&self, key: &CacheKey) -> bool {
        self.l1.contains_key(key) || self.l2.contains_key(key)
    }

    /// Inserts or updates an entry in the cache.
    /// Always lands in L1 (MRU). If L1 overflows, the evicted item is demoted to L2.
    /// If L2 overflows, its LRU item is dropped.
    pub fn insert(&mut self, key: CacheKey, value: V) {
        // Track key in secondary file index
        self.track_file_key(&key);

        // If previously in L2, remove it first
        if self.l2.contains_key(&key) {
            self.l2.remove(&key);
        }

        // Insert into L1
        let (_, demoted) = self.l1.insert(key, value);
        if let Some((demoted_key, demoted_val)) = demoted {
            self.l1_demotions += 1;
            let (_, l2_evicted) = self.l2.insert(demoted_key, demoted_val);
            if let Some((dropped_key, _)) = l2_evicted {
                self.l2_evictions += 1;
                self.untrack_file_key(&dropped_key);
            }
        }
    }

    /// Removes an exact `CacheKey` from both levels if present.
    pub fn remove(&mut self, key: &CacheKey) -> Option<V> {
        self.untrack_file_key(key);
        if let Some(val) = self.l1.remove(key) {
            return Some(val);
        }
        self.l2.remove(key)
    }

    /// Invalidates all cached entries associated with the given `FileId`.
    ///
    /// Invoked immediately whenever a write handle is opened or atomic replacement occurs.
    /// Returns the number of invalidated cache entries.
    pub fn invalidate_file(&mut self, file_id: &FileId) -> usize {
        let Some(keys) = self.file_index.remove(file_id) else {
            return 0;
        };

        let mut count = 0;
        for key in keys {
            if self.l1.remove(&key).is_some() {
                count += 1;
            }
            if self.l2.remove(&key).is_some() {
                count += 1;
            }
        }

        self.invalidations += count as u64;
        count
    }

    /// Clears both cache levels and indices.
    pub fn clear(&mut self) {
        self.l1.clear();
        self.l2.clear();
        self.file_index.clear();
    }

    /// Returns current statistics.
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            l1_hits: self.l1_hits,
            l2_hits: self.l2_hits,
            misses: self.misses,
            l1_demotions: self.l1_demotions,
            l2_evictions: self.l2_evictions,
            invalidations: self.invalidations,
            l1_len: self.l1.len(),
            l2_len: self.l2.len(),
            l1_capacity: self.l1.capacity(),
            l2_capacity: self.l2.capacity(),
        }
    }

    /// Total entries currently stored across both L1 and L2.
    #[inline]
    pub fn len(&self) -> usize {
        self.l1.len() + self.l2.len()
    }

    /// Returns true if both cache levels are empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn track_file_key(&mut self, key: &CacheKey) {
        self.file_index
            .entry(key.file_id)
            .or_default()
            .insert(*key);
    }

    fn untrack_file_key(&mut self, key: &CacheKey) {
        if let Some(keys) = self.file_index.get_mut(&key.file_id) {
            keys.remove(key);
            if keys.is_empty() {
                self.file_index.remove(&key.file_id);
            }
        }
    }
}

impl<V: Clone> Default for TwoLevelLruCache<V> {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_level_lru_eviction() {
        let mut lru = LruLevel::<u32, &'static str>::new(2);
        lru.insert(1, "one");
        lru.insert(2, "two");

        // Access 1 to make it MRU, 2 becomes LRU
        assert_eq!(lru.get_mut(&1), Some(&mut "one"));

        // Inserting 3 should evict 2
        let (_, evicted) = lru.insert(3, "three");
        assert_eq!(evicted, Some((2, "two")));

        assert!(lru.contains_key(&1));
        assert!(lru.contains_key(&3));
        assert!(!lru.contains_key(&2));
    }

    #[test]
    fn test_two_level_lru_promotion_and_demotion() {
        // L1 capacity = 2, L2 capacity = 2
        let mut cache = TwoLevelLruCache::<String>::new(2, 2);

        let k1 = CacheKey::from_parts(1, 100, 1000, 500);
        let k2 = CacheKey::from_parts(1, 101, 1000, 600);
        let k3 = CacheKey::from_parts(1, 102, 1000, 700);
        let k4 = CacheKey::from_parts(1, 103, 1000, 800);
        let k5 = CacheKey::from_parts(1, 104, 1000, 900);

        // Insert k1, k2 -> both in L1
        cache.insert(k1, "file1".into());
        cache.insert(k2, "file2".into());
        assert_eq!(cache.l1.len(), 2);
        assert_eq!(cache.l2.len(), 0);

        // Insert k3 -> L1 full, k1 demoted to L2
        cache.insert(k3, "file3".into());
        assert_eq!(cache.l1.len(), 2); // contains k2, k3
        assert_eq!(cache.l2.len(), 1); // contains k1
        assert!(cache.l2.contains_key(&k1));

        // Insert k4 -> L1 full, k2 demoted to L2
        cache.insert(k4, "file4".into());
        assert_eq!(cache.l1.len(), 2); // contains k3, k4
        assert_eq!(cache.l2.len(), 2); // contains k1, k2

        // Access k1 (currently in L2): promotes to L1!
        // k3 was LRU in L1, so k3 gets demoted to L2
        let val = cache.get(&k1);
        assert_eq!(val, Some("file1".into()));
        assert!(cache.l1.contains_key(&k1));
        assert!(cache.l2.contains_key(&k3));

        // Insert k5 -> L1 overflows (demoting k4 to L2), L2 overflows (evicting its LRU)
        cache.insert(k5, "file5".into());
        let stats = cache.stats();
        assert!(stats.l1_demotions >= 3);
        assert!(stats.l2_evictions >= 1);
    }

    #[test]
    fn test_file_invalidation_multi_versions() {
        let mut cache = TwoLevelLruCache::<ScanVerdict>::new(10, 10);
        let target_file = FileId::new(0xABCD, 0x1234);
        let other_file = FileId::new(0xABCD, 0x5678);

        let k1 = CacheKey::new(target_file, 1000, 500);
        let k2 = CacheKey::new(target_file, 2000, 550); // Modified version
        let k_other = CacheKey::new(other_file, 1000, 500);

        cache.insert(k1, ScanVerdict::Clean);
        cache.insert(k2, ScanVerdict::Clean);
        cache.insert(k_other, ScanVerdict::Whitelisted);

        assert_eq!(cache.len(), 3);

        // File modified / write handle opened -> invalidate target_file
        let count = cache.invalidate_file(&target_file);
        assert_eq!(count, 2);

        // target_file entries must be gone
        assert!(!cache.contains_key(&k1));
        assert!(!cache.contains_key(&k2));

        // other_file must still be present
        assert!(cache.contains_key(&k_other));
        assert_eq!(cache.get(&k_other), Some(ScanVerdict::Whitelisted));
    }

    #[test]
    fn test_cache_entry_ttl_expiration() {
        let entry = CacheEntry::new(
            ScanVerdict::Clean,
            1_000_000_000, // 1 sec in ns
            60,            // 60 secs TTL
            [0u8; 32],
        );

        // At 30 secs: not expired
        assert!(!entry.is_expired(30_000_000_000));
        // At 60 secs: not expired
        assert!(!entry.is_expired(61_000_000_000));
        // At 62 secs: expired!
        assert!(entry.is_expired(62_000_000_001));
    }
}
