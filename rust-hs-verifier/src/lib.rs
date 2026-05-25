pub mod challenge;
pub mod evaluate;
pub mod certificate;
pub mod issuer;
pub mod adapter;
pub mod verifiers;
pub mod transparency;
pub mod server;
pub mod types;

pub use types::*;

use sha2::{Sha256, Digest};

pub fn sha256(data: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

pub fn hash_to_hex(hash: &Hash) -> String {
    hex::encode(hash)
}

pub fn hex_to_hash(hex: &str) -> Result<Hash, hex::FromHexError> {
    let bytes = hex::decode(hex)?;
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&bytes);
    Ok(hash)
}
