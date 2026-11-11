//! Fast Bloom filter for quick negative lookups in HyAV signature files.
//! Uses Kirsch-Mitzenmacher optimization with two 64-bit hash values to generate k hashes.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BloomFilter {
    pub m_bits: u32,
    pub k_hashes: u8,
    pub bitvec: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BloomError {
    BufferTooSmall,
    InvalidBitLength,
    ZeroHashes,
}

impl BloomFilter {
    /// Creates a new Bloom filter optimized for an expected number of entries
    /// with a specified false positive rate (e.g. 0.01 = 1%).
    pub fn new(expected_elements: usize, false_positive_rate: f64) -> Self {
        let n = expected_elements.max(1) as f64;
        let p = false_positive_rate.clamp(0.0001, 0.5);

        // Optimal bit length: m = - (n * ln(p)) / (ln(2)^2)
        let ln2 = core::f64::consts::LN_2;
        let m = (-(n * p.ln()) / (ln2 * ln2)).ceil() as u32;
        let m_bits = m.max(64); // minimum 64 bits

        // Optimal hash count: k = (m / n) * ln(2)
        let k = ((m_bits as f64 / n) * ln2).round() as u8;
        let k_hashes = k.clamp(1, 16);

        let byte_len = ((m_bits + 7) / 8) as usize;
        let bitvec = vec![0u8; byte_len];

        Self {
            m_bits,
            k_hashes,
            bitvec,
        }
    }

    /// Creates a fixed-size Bloom filter (e.g. 1024 bytes = 8192 bits)
    pub fn with_size(byte_len: usize, k_hashes: u8) -> Self {
        let byte_len = byte_len.max(8);
        let m_bits = (byte_len * 8) as u32;
        Self {
            m_bits,
            k_hashes: k_hashes.clamp(1, 16),
            bitvec: vec![0u8; byte_len],
        }
    }

    /// Insert arbitrary bytes into the filter
    pub fn insert_bytes(&mut self, data: &[u8]) {
        let (h1, h2) = hash_pair(data);
        for i in 0..self.k_hashes {
            let bit_idx = (h1.wrapping_add((i as u64).wrapping_mul(h2))) % (self.m_bits as u64);
            let byte_pos = (bit_idx / 8) as usize;
            let bit_pos = (bit_idx % 8) as u8;
            if byte_pos < self.bitvec.len() {
                self.bitvec[byte_pos] |= 1 << bit_pos;
            }
        }
    }

    /// Check if bytes might be in the set (true = maybe, false = definitely not)
    pub fn contains_bytes(&self, data: &[u8]) -> bool {
        let (h1, h2) = hash_pair(data);
        for i in 0..self.k_hashes {
            let bit_idx = (h1.wrapping_add((i as u64).wrapping_mul(h2))) % (self.m_bits as u64);
            let byte_pos = (bit_idx / 8) as usize;
            let bit_pos = (bit_idx % 8) as u8;
            if byte_pos >= self.bitvec.len() || (self.bitvec[byte_pos] & (1 << bit_pos)) == 0 {
                return false;
            }
        }
        true
    }

    pub fn insert_sha256(&mut self, hash: &[u8; 32]) {
        self.insert_bytes(hash);
    }

    pub fn contains_sha256(&self, hash: &[u8; 32]) -> bool {
        self.contains_bytes(hash)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + self.bitvec.len());
        out.extend_from_slice(&self.m_bits.to_le_bytes());
        out.push(self.k_hashes);
        out.extend_from_slice(&[0u8; 3]); // padding/alignment
        out.extend_from_slice(&self.bitvec);
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BloomError> {
        if bytes.len() < 8 {
            return Err(BloomError::BufferTooSmall);
        }

        let m_bits = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let k_hashes = bytes[4];
        if k_hashes == 0 {
            return Err(BloomError::ZeroHashes);
        }

        let expected_bytes = ((m_bits + 7) / 8) as usize;
        let payload = &bytes[8..];
        if payload.len() < expected_bytes {
            return Err(BloomError::InvalidBitLength);
        }

        let bitvec = payload[..expected_bytes].to_vec();
        Ok(Self {
            m_bits,
            k_hashes,
            bitvec,
        })
    }
}

/// Computes two independent 64-bit hashes using FNV-1a variations
fn hash_pair(data: &[u8]) -> (u64, u64) {
    const FNV_PRIME_1: u64 = 0x100000001b3;
    const FNV_OFFSET_1: u64 = 0xcbf29ce484222325;
    const FNV_PRIME_2: u64 = 0x10000000233;
    const FNV_OFFSET_2: u64 = 0x84222325cbf29ce4;

    let mut h1 = FNV_OFFSET_1;
    let mut h2 = FNV_OFFSET_2;

    for &byte in data {
        h1 ^= byte as u64;
        h1 = h1.wrapping_mul(FNV_PRIME_1);

        h2 = h2.wrapping_mul(FNV_PRIME_2);
        h2 ^= byte as u64;
    }

    // Ensure h2 is non-zero so all k hashes are distinct
    if h2 == 0 {
        h2 = 1;
    }

    (h1, h2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_basic() {
        let mut filter = BloomFilter::new(100, 0.01);
        let key1 = b"Trojan.Win32.Malware";
        let key2 = b"Clean.Executable";

        filter.insert_bytes(key1);
        assert!(filter.contains_bytes(key1));
        assert!(!filter.contains_bytes(key2));

        let bytes = filter.to_bytes();
        let loaded = BloomFilter::from_bytes(&bytes).expect("deserialization");
        assert!(loaded.contains_bytes(key1));
        assert!(!loaded.contains_bytes(key2));
    }
}
