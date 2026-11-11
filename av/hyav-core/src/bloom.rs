//! High-Throughput Bloom Filter Implementation
//!
//! Designed for HyAV fast rejection of known malicious SHA-256 prefixes and signatures.
//! Default configuration: 64M bits (8 MiB) and k = 7 hash functions.
//! Uses Kirsch-Mitzenmacher double-hashing optimization for maximum throughput.

use std::fmt;

/// Default filter size: 64 Mbits = 67,108,864 bits = 8 MiB.
pub const DEFAULT_BLOOM_BITS: usize = 64 * 1024 * 1024;

/// Default number of hash functions (k = 7).
pub const DEFAULT_BLOOM_K: usize = 7;

/// Error types for Bloom filter operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BloomError {
    /// Specified filter size in bits was zero.
    InvalidSize,
    /// Number of hash functions k was zero.
    InvalidK,
    /// Buffer passed to deserialize was too small or unaligned.
    BufferTooSmall,
}

impl fmt::Display for BloomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BloomError::InvalidSize => write!(f, "Bloom filter size must be greater than 0"),
            BloomError::InvalidK => write!(f, "Number of hash functions k must be greater than 0"),
            BloomError::BufferTooSmall => write!(f, "Buffer is too small to restore Bloom filter"),
        }
    }
}

/// Ultra-fast 128-bit mixing function in 100% safe Rust.
/// Produces two independent, uniformly distributed 64-bit hashes (h1, h2).
#[inline]
pub fn hash128(data: &[u8]) -> (u64, u64) {
    let mut h1: u64 = 0x517cc1b727220a95;
    let mut h2: u64 = 0x9e3779b97f4a7c15;

    let len = data.len();
    let mut chunks = data.chunks_exact(16);
    for chunk in chunks.by_ref() {
        let k1 = u64::from_le_bytes(chunk[0..8].try_into().unwrap());
        let k2 = u64::from_le_bytes(chunk[8..16].try_into().unwrap());

        let mut k1_m = k1.wrapping_mul(0xbf58476d1ce4e5b9);
        k1_m = (k1_m ^ (k1_m >> 30)).wrapping_mul(0x94d049bb133111eb);
        h1 = (h1 ^ k1_m)
            .rotate_left(27)
            .wrapping_add(h2)
            .wrapping_mul(5)
            .wrapping_add(0x52dce729);

        let mut k2_m = k2.wrapping_mul(0x94d049bb133111eb);
        k2_m = (k2_m ^ (k2_m >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        h2 = (h2 ^ k2_m)
            .rotate_left(31)
            .wrapping_add(h1)
            .wrapping_mul(5)
            .wrapping_add(0x38495ab5);
    }

    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let mut rem_buf = [0u8; 16];
        rem_buf[..remainder.len()].copy_from_slice(remainder);
        let k1 = u64::from_le_bytes(rem_buf[0..8].try_into().unwrap());
        let k2 = u64::from_le_bytes(rem_buf[8..16].try_into().unwrap());
        h1 ^= k1.wrapping_mul(0xbf58476d1ce4e5b9);
        h2 ^= k2.wrapping_mul(0x94d049bb133111eb);
    }

    h1 ^= len as u64;
    h2 ^= len as u64;

    h1 = (h1 ^ (h1 >> 33)).wrapping_mul(0xff51afd7ed558ccd);
    h1 = (h1 ^ (h1 >> 33)).wrapping_mul(0xc4ceb9fe1a85ec53);
    h1 ^= h1 >> 33;

    h2 = (h2 ^ (h2 >> 33)).wrapping_mul(0xff51afd7ed558ccd);
    h2 = (h2 ^ (h2 >> 33)).wrapping_mul(0xc4ceb9fe1a85ec53);
    h2 ^= h2 >> 33;

    (h1, h2)
}

/// High-throughput Bloom filter backed by a 64-bit aligned bitset.
#[derive(Clone)]
pub struct BloomFilter {
    words: Vec<u64>,
    num_bits: usize,
    k: usize,
    is_power_of_two: bool,
    mask: usize,
    items_count: usize,
}

impl BloomFilter {
    /// Constructs a Bloom filter with the standard HyAV parameters:
    /// 64M bits (8 MiB) and k = 7.
    pub fn new_default() -> Self {
        Self::new(DEFAULT_BLOOM_BITS, DEFAULT_BLOOM_K).expect("Default configuration is valid")
    }

    /// Constructs a Bloom filter with custom bit size and hash function count k.
    pub fn new(num_bits: usize, k: usize) -> Result<Self, BloomError> {
        if num_bits == 0 {
            return Err(BloomError::InvalidSize);
        }
        if k == 0 {
            return Err(BloomError::InvalidK);
        }

        // Round up to nearest multiple of 64
        let word_count = (num_bits + 63) / 64;
        let effective_bits = word_count * 64;

        let is_power_of_two = effective_bits.is_power_of_two();
        let mask = if is_power_of_two {
            effective_bits - 1
        } else {
            0
        };

        Ok(Self {
            words: vec![0u64; word_count],
            num_bits: effective_bits,
            k,
            is_power_of_two,
            mask,
            items_count: 0,
        })
    }

    /// Total number of addressable bits in the filter.
    #[inline]
    pub const fn num_bits(&self) -> usize {
        self.num_bits
    }

    /// Number of hash functions (k).
    #[inline]
    pub const fn k(&self) -> usize {
        self.k
    }

    /// Approximate number of items inserted so far.
    #[inline]
    pub const fn len(&self) -> usize {
        self.items_count
    }

    /// Returns true if no items have been inserted.
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.items_count == 0
    }

    /// Memory footprint of the bit array in bytes.
    #[inline]
    pub fn memory_usage(&self) -> usize {
        self.words.len() * core::mem::size_of::<u64>()
    }

    /// Clears all bits in the filter.
    pub fn clear(&mut self) {
        self.words.fill(0);
        self.items_count = 0;
    }

    /// Calculates the bit index using Kirsch-Mitzenmacher double-hashing.
    #[inline(always)]
    fn get_bit_index(&self, h1: u64, h2: u64, i: usize) -> usize {
        // Kirsch-Mitzenmacher: gi(x) = h1 + i * h2 (mod m)
        // Ensure h2 is odd so gcd(h2, m) = 1 for power-of-two modulus
        let step = h2 | 1;
        let combined = h1.wrapping_add((i as u64).wrapping_mul(step));
        if self.is_power_of_two {
            (combined as usize) & self.mask
        } else {
            (combined as usize) % self.num_bits
        }
    }

    /// Inserts an item (byte slice) into the Bloom filter.
    #[inline]
    pub fn insert(&mut self, item: impl AsRef<[u8]>) {
        let (h1, h2) = hash128(item.as_ref());
        self.insert_hash(h1, h2);
    }

    /// Inserts an item given its pre-computed 128-bit hash components (h1, h2).
    #[inline]
    pub fn insert_hash(&mut self, h1: u64, h2: u64) {
        for i in 0..self.k {
            let bit_idx = self.get_bit_index(h1, h2, i);
            let word_idx = bit_idx >> 6;
            let bit_offset = bit_idx & 63;
            self.words[word_idx] |= 1u64 << bit_offset;
        }
        self.items_count = self.items_count.saturating_add(1);
    }

    /// Queries whether an item is possibly in the set.
    ///
    /// Returns:
    /// - `false`: The item is definitely NOT in the set (0% false negatives).
    /// - `true`: The item is POSSIBLY in the set (with small false positive probability).
    #[inline]
    pub fn contains(&self, item: impl AsRef<[u8]>) -> bool {
        let (h1, h2) = hash128(item.as_ref());
        self.contains_hash(h1, h2)
    }

    /// Queries whether precomputed hash components are possibly in the set.
    #[inline]
    pub fn contains_hash(&self, h1: u64, h2: u64) -> bool {
        for i in 0..self.k {
            let bit_idx = self.get_bit_index(h1, h2, i);
            let word_idx = bit_idx >> 6;
            let bit_offset = bit_idx & 63;
            if (self.words[word_idx] & (1u64 << bit_offset)) == 0 {
                return false;
            }
        }
        true
    }

    /// Counts the total number of bits currently set to 1.
    pub fn count_ones(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Estimates the current number of unique elements inserted using the Swamidass & Baldi formula:
    /// n* = -(m / k) * ln(1 - X / m)
    /// where X is the number of bits set to 1.
    pub fn estimated_element_count(&self) -> usize {
        let m = self.num_bits as f64;
        let k = self.k as f64;
        let x = self.count_ones() as f64;

        if x >= m {
            return usize::MAX;
        }

        let ratio = 1.0 - (x / m);
        let n_star = -(m / k) * ratio.ln();
        if n_star < 0.0 {
            0
        } else {
            n_star.round() as usize
        }
    }

    /// Calculates theoretical false positive probability for a given element count `n`.
    /// p = (1 - e^(-k * n / m))^k
    pub fn theoretical_fp_rate(&self, n: usize) -> f64 {
        let m = self.num_bits as f64;
        let k = self.k as f64;
        let n = n as f64;
        let exponent = -k * n / m;
        (1.0 - exponent.exp()).powf(k)
    }

    /// Serializes raw bit array to little-endian bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.words.len() * 8);
        for w in &self.words {
            bytes.extend_from_slice(&w.to_le_bytes());
        }
        bytes
    }

    /// Reconstructs Bloom filter from serialized bit array bytes.
    pub fn from_bytes(bytes: &[u8], k: usize) -> Result<Self, BloomError> {
        if bytes.is_empty() || bytes.len() % 8 != 0 {
            return Err(BloomError::BufferTooSmall);
        }
        if k == 0 {
            return Err(BloomError::InvalidK);
        }

        let word_count = bytes.len() / 8;
        let mut words = Vec::with_capacity(word_count);
        for chunk in bytes.chunks_exact(8) {
            words.push(u64::from_le_bytes(chunk.try_into().unwrap()));
        }

        let num_bits = word_count * 64;
        let is_power_of_two = num_bits.is_power_of_two();
        let mask = if is_power_of_two { num_bits - 1 } else { 0 };

        Ok(Self {
            words,
            num_bits,
            k,
            is_power_of_two,
            mask,
            items_count: 0,
        })
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::new_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_default_dimensions() {
        let bf = BloomFilter::new_default();
        assert_eq!(bf.num_bits(), 64 * 1024 * 1024); // 64M bits
        assert_eq!(bf.k(), 7);
        assert_eq!(bf.memory_usage(), 8 * 1024 * 1024); // 8 MB
        assert!(bf.is_empty());
    }

    #[test]
    fn test_zero_false_negatives() {
        let mut bf = BloomFilter::new(1024 * 1024, 7).unwrap();
        let items: Vec<String> = (0..5000).map(|i| format!("malware_signature_{:08x}", i)).collect();

        for item in &items {
            bf.insert(item.as_bytes());
        }

        // Every inserted item must be present (0% false negatives)
        for item in &items {
            assert!(
                bf.contains(item.as_bytes()),
                "Bloom filter must have zero false negatives for inserted item: {}",
                item
            );
        }
    }

    #[test]
    fn test_false_positive_rate_verification() {
        // Test with 1M bits and k=7
        let bits = 1024 * 1024; // 1,048,576 bits
        let k = 7;
        let mut bf = BloomFilter::new(bits, k).unwrap();

        let inserted_count = 10_000;
        for i in 0..inserted_count {
            let key = format!("inserted_sha256_{:08x}", i);
            bf.insert(key.as_bytes());
        }

        // Verify theoretical FP rate
        let theoretical_fp = bf.theoretical_fp_rate(inserted_count);
        assert!(theoretical_fp < 0.001);

        // Test empirical false positive rate against 50,000 non-members
        let test_count = 50_000;
        let mut false_positives = 0;
        for i in inserted_count..(inserted_count + test_count) {
            let key = format!("non_member_sha256_{:08x}", i);
            if bf.contains(key.as_bytes()) {
                false_positives += 1;
            }
        }

        let empirical_fp = (false_positives as f64) / (test_count as f64);
        assert!(
            empirical_fp <= 0.01,
            "Empirical FP rate ({:.6}) exceeded acceptable limit (0.01)",
            empirical_fp
        );
    }

    #[test]
    fn test_clear_and_reinsertion() {
        let mut bf = BloomFilter::new(65536, 7).unwrap();
        bf.insert(b"test_payload_123");
        assert!(bf.contains(b"test_payload_123"));

        bf.clear();
        assert_eq!(bf.count_ones(), 0);
        assert!(!bf.contains(b"test_payload_123"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut bf = BloomFilter::new(1024, 7).unwrap();
        bf.insert(b"signature_alpha");
        bf.insert(b"signature_beta");

        let bytes = bf.to_bytes();
        let restored = BloomFilter::from_bytes(&bytes, 7).unwrap();

        assert!(restored.contains(b"signature_alpha"));
        assert!(restored.contains(b"signature_beta"));
        assert_eq!(restored.count_ones(), bf.count_ones());
    }
}
