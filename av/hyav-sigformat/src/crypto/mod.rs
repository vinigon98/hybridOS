pub mod ed25519;
pub mod sha256;
pub mod sha512;

pub use ed25519::{sign, verify, Ed25519Error, Keypair};
pub use sha256::{sha256, Sha256};
pub use sha512::{sha512, Sha512};
