//! Binary Codec for HyAV signature databases (.sig/.db/.hsv)
//! Serializes and deserializes Header, Bloom filter section, Records, and Trailer.

use crate::bloom::{BloomError, BloomFilter};
use crate::header::{DbType, Header, HeaderError, HEADER_SIZE};
use crate::record::{
    ElfHeaderRecord, HashExactRecord, PathRuleRecord, PatternRecord, PeHeaderRecord, RecordCodecError,
    RecordPayload, SignatureRecord, SyntheticElfInfo, SyntheticPeInfo,
};
use crate::trailer::{Trailer, TrailerError, TRAILER_SIZE};

#[derive(Debug, Clone, PartialEq)]
pub struct SignatureDatabase {
    pub header: Header,
    pub bloom: BloomFilter,
    pub records: Vec<SignatureRecord>,
    pub trailer: Option<Trailer>,
}

#[derive(Debug)]
pub enum CodecError {
    Header(HeaderError),
    Bloom(BloomError),
    Record(RecordCodecError),
    Trailer(TrailerError),
    BufferTooShort,
    InvalidOffsets,
    MissingTrailer,
    VerificationFailed(TrailerError),
}

impl From<HeaderError> for CodecError {
    fn from(e: HeaderError) -> Self {
        CodecError::Header(e)
    }
}

impl From<BloomError> for CodecError {
    fn from(e: BloomError) -> Self {
        CodecError::Bloom(e)
    }
}

impl From<RecordCodecError> for CodecError {
    fn from(e: RecordCodecError) -> Self {
        CodecError::Record(e)
    }
}

impl From<TrailerError> for CodecError {
    fn from(e: TrailerError) -> Self {
        CodecError::Trailer(e)
    }
}

impl SignatureDatabase {
    /// Encodes the entire signature database into a binary vector.
    /// If `signing_seed` is provided, generates an authentic Trailer with SHA-256 and Ed25519 signature.
    pub fn encode(&self, signing_seed: Option<&[u8; 32]>) -> Result<Vec<u8>, CodecError> {
        let bloom_bytes = self.bloom.to_bytes();
        let mut records_bytes = Vec::new();
        for r in &self.records {
            records_bytes.extend_from_slice(&r.serialize());
        }

        let bloom_offset = HEADER_SIZE as u32;
        let bloom_size = bloom_bytes.len() as u32;
        let records_offset = bloom_offset + bloom_size;
        let records_size = records_bytes.len() as u32;
        let trailer_offset = records_offset + records_size;

        let mut header = self.header.clone();
        header.record_count = self.records.len() as u32;
        header.bloom_offset = bloom_offset;
        header.bloom_size = bloom_size;
        header.records_offset = records_offset;
        header.records_size = records_size;
        header.trailer_offset = trailer_offset;

        let header_bytes = header.to_bytes();

        let mut content = Vec::with_capacity((trailer_offset as usize) + TRAILER_SIZE);
        content.extend_from_slice(&header_bytes);
        content.extend_from_slice(&bloom_bytes);
        content.extend_from_slice(&records_bytes);

        if let Some(seed) = signing_seed {
            let trailer = Trailer::sign(&content, seed);
            content.extend_from_slice(&trailer.to_bytes());
        } else if let Some(trailer) = &self.trailer {
            content.extend_from_slice(&trailer.to_bytes());
        }

        Ok(content)
    }

    /// Decodes a signature database from binary bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, CodecError> {
        if bytes.len() < HEADER_SIZE {
            return Err(CodecError::BufferTooShort);
        }

        let header = Header::from_bytes(&bytes[0..HEADER_SIZE])?;

        let b_start = header.bloom_offset as usize;
        let b_end = b_start + (header.bloom_size as usize);
        if bytes.len() < b_end {
            return Err(CodecError::BufferTooShort);
        }
        let bloom = BloomFilter::from_bytes(&bytes[b_start..b_end])?;

        let r_start = header.records_offset as usize;
        let r_end = r_start + (header.records_size as usize);
        if bytes.len() < r_end {
            return Err(CodecError::BufferTooShort);
        }

        let records_data = &bytes[r_start..r_end];
        let mut records = Vec::with_capacity(header.record_count as usize);
        let mut offset = 0;
        for _ in 0..header.record_count {
            if offset >= records_data.len() {
                break;
            }
            let (rec, consumed) = SignatureRecord::deserialize(&records_data[offset..])?;
            records.push(rec);
            offset += consumed;
        }

        let t_start = header.trailer_offset as usize;
        let trailer = if bytes.len() >= t_start + TRAILER_SIZE {
            Some(Trailer::from_bytes(&bytes[t_start..t_start + TRAILER_SIZE])?)
        } else {
            None
        };

        Ok(Self {
            header,
            bloom,
            records,
            trailer,
        })
    }

    /// Verifies the database against the raw binary file and trusted Ed25519 public key.
    pub fn verify_raw(bytes: &[u8], public_key: &[u8; 32]) -> Result<Self, CodecError> {
        let db = Self::decode(bytes)?;
        let trailer = db.trailer.as_ref().ok_or(CodecError::MissingTrailer)?;
        let content_len = db.header.trailer_offset as usize;
        if bytes.len() < content_len {
            return Err(CodecError::BufferTooShort);
        }

        trailer
            .verify(&bytes[..content_len], public_key)
            .map_err(CodecError::VerificationFailed)?;

        Ok(db)
    }

    // -------------------------------------------------------------------------
    // Scanning and Matching Engine APIs
    // -------------------------------------------------------------------------

    /// Matches a 32-byte cryptographic hash against the database.
    /// First checks the fast Bloom filter; if negative, immediately returns None.
    pub fn find_hash_exact(&self, hash: &[u8; 32], size: u64) -> Option<&SignatureRecord> {
        // Fast negative Bloom filter rejection
        if !self.bloom.contains_sha256(hash) {
            return None;
        }

        for r in &self.records {
            if let RecordPayload::HashExact(h) = &r.payload {
                if h.matches(hash, size) {
                    return Some(r);
                }
            }
        }
        None
    }

    /// Scans a byte buffer against all PATTERN signatures.
    pub fn scan_patterns(&self, data: &[u8]) -> Vec<&SignatureRecord> {
        let mut hits = Vec::new();
        for r in &self.records {
            if let RecordPayload::Pattern(p) = &r.payload {
                if p.matches(data) {
                    hits.push(r);
                }
            }
        }
        hits
    }

    /// Evaluates path against PATH_RULE signatures.
    pub fn scan_path(&self, path: &str) -> Vec<&SignatureRecord> {
        let mut hits = Vec::new();
        for r in &self.records {
            if let RecordPayload::PathRule(p) = &r.payload {
                if p.matches(path) {
                    hits.push(r);
                }
            }
        }
        hits
    }

    /// Evaluates PE heuristics against PE_HEADER signatures.
    pub fn scan_pe(&self, pe: &SyntheticPeInfo) -> Vec<&SignatureRecord> {
        let mut hits = Vec::new();
        for r in &self.records {
            if let RecordPayload::PeHeader(p) = &r.payload {
                if p.matches(pe) {
                    hits.push(r);
                }
            }
        }
        hits
    }

    /// Evaluates ELF heuristics against ELF_HEADER signatures.
    pub fn scan_elf(&self, elf: &SyntheticElfInfo) -> Vec<&SignatureRecord> {
        let mut hits = Vec::new();
        for r in &self.records {
            if let RecordPayload::ElfHeader(p) = &r.payload {
                if p.matches(elf) {
                    hits.push(r);
                }
            }
        }
        hits
    }
}

// -----------------------------------------------------------------------------
// Database Builder
// -----------------------------------------------------------------------------

pub struct DatabaseBuilder {
    db_type: DbType,
    records: Vec<SignatureRecord>,
    bloom_elements: usize,
    bloom_fp_rate: f64,
}

impl DatabaseBuilder {
    pub fn new(db_type: DbType) -> Self {
        Self {
            db_type,
            records: Vec::new(),
            bloom_elements: 1024,
            bloom_fp_rate: 0.01,
        }
    }

    pub fn with_capacity(mut self, capacity: usize) -> Self {
        self.bloom_elements = capacity.max(64);
        self
    }

    pub fn add_record(mut self, record: SignatureRecord) -> Self {
        self.records.push(record);
        self
    }

    pub fn build(self) -> SignatureDatabase {
        let count = self.records.len();
        let mut bloom = BloomFilter::new(self.bloom_elements.max(count), self.bloom_fp_rate);

        // Populate Bloom filter with all exact hashes and known pattern tokens
        for r in &self.records {
            match &r.payload {
                RecordPayload::HashExact(h) => {
                    bloom.insert_sha256(&h.hash);
                }
                RecordPayload::Pattern(p) => {
                    if p.pattern.len() >= 4 {
                        // Take first 8 bytes of pattern as bloom token
                        let token_len = p.pattern.len().min(8);
                        bloom.insert_bytes(&p.pattern[..token_len]);
                    }
                }
                _ => {}
            }
        }

        let header = Header::new(self.db_type, count as u32, 0, 0, 0, 0, 0);

        SignatureDatabase {
            header,
            bloom,
            records: self.records,
            trailer: None,
        }
    }
}
