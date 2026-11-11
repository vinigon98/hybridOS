//! Signature database Trailer with SHA-256 content verification and Ed25519 digital signature.
//! Magic: HYAVEOF\0 (8 bytes) preceded by 32 bytes SHA-256 and 64 bytes Ed25519 signature = 104 bytes.

use crate::crypto::{ed25519, sha256};

pub const TRAILER_MAGIC: [u8; 8] = *b"HYAVEOF\0";
pub const TRAILER_SIZE: usize = 104;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trailer {
    pub content_sha256: [u8; 32],
    pub ed25519_signature: [u8; 64],
    pub magic: [u8; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrailerError {
    BufferTooSmall,
    InvalidMagic,
    Sha256Mismatch,
    Ed25519Invalid(ed25519::Ed25519Error),
}

impl Trailer {
    pub fn new(content_sha256: [u8; 32], ed25519_signature: [u8; 64]) -> Self {
        Self {
            content_sha256,
            ed25519_signature,
            magic: TRAILER_MAGIC,
        }
    }

    /// Computes SHA-256 of content and signs it with Ed25519 secret seed
    pub fn sign(content: &[u8], secret_seed: &[u8; 32]) -> Self {
        let content_hash = sha256::sha256(content);
        let signature = ed25519::sign(secret_seed, &content_hash);
        Self::new(content_hash, signature)
    }

    /// Verifies both the SHA-256 hash against content and the Ed25519 digital signature
    pub fn verify(&self, content: &[u8], public_key: &[u8; 32]) -> Result<(), TrailerError> {
        // 1. Verify content SHA-256
        let actual_hash = sha256::sha256(content);
        if actual_hash != self.content_sha256 {
            return Err(TrailerError::Sha256Mismatch);
        }

        // 2. Verify Ed25519 signature of the content hash
        ed25519::verify(public_key, &self.content_sha256, &self.ed25519_signature)
            .map_err(TrailerError::Ed25519Invalid)?;

        Ok(())
    }

    pub fn to_bytes(&self) -> [u8; TRAILER_SIZE] {
        let mut buf = [0u8; TRAILER_SIZE];
        buf[0..32].copy_from_slice(&self.content_sha256);
        buf[32..96].copy_from_slice(&self.ed25519_signature);
        buf[96..104].copy_from_slice(&self.magic);
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TrailerError> {
        if bytes.len() < TRAILER_SIZE {
            return Err(TrailerError::BufferTooSmall);
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[96..104]);
        if magic != TRAILER_MAGIC {
            return Err(TrailerError::InvalidMagic);
        }

        let mut content_sha256 = [0u8; 32];
        content_sha256.copy_from_slice(&bytes[0..32]);

        let mut ed25519_signature = [0u8; 64];
        ed25519_signature.copy_from_slice(&bytes[32..96]);

        Ok(Self {
            content_sha256,
            ed25519_signature,
            magic,
        })
    }
}
