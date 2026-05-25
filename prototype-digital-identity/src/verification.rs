// Verification utilities for HSK proofs

use ed25519_dalek::{PublicKey, Signature, Verifier};
use sha2::{Sha256, Digest};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Verify an Ed25519 signature
pub fn verify_signature(
    message: &[u8],
    signature_bytes: &[u8],
    public_key_bytes: &[u8],
) -> Result<bool, Box<dyn std::error::Error>> {
    let public_key = PublicKey::from_bytes(public_key_bytes)
        .map_err(|e| format!("Invalid public key: {}", e))?;
    
    let signature = Signature::from_bytes(signature_bytes)
        .map_err(|e| format!("Invalid signature: {}", e))?;
    
    match public_key.verify(message, &signature) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// Compute SHA256 hash
pub fn compute_hash(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Verify hash chain integrity
pub fn verify_hash_chain(
    entries: &[crate::models::ConsentEntry],
) -> Result<bool, Vec<String>> {
    let mut previous_hash = vec![0u8; 32];
    let mut invalid_entries = Vec::new();
    let mut all_valid = true;

    for entry in entries {
        let computed_hash = compute_entry_hash_from_entry(entry, &previous_hash);
        
        if hex::encode(&computed_hash) != entry.entry_id {
            all_valid = false;
            invalid_entries.push(entry.entry_id.clone());
        }
        
        previous_hash = computed_hash.to_vec();
    }

    if all_valid {
        Ok(true)
    } else {
        Err(invalid_entries)
    }
}

fn compute_entry_hash_from_entry(
    entry: &crate::models::ConsentEntry,
    previous_hash: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    
    hasher.update(previous_hash);
    hasher.update(entry.granted_at.to_rfc3339().as_bytes());
    hasher.update(entry.action.as_bytes());
    hasher.update(entry.scope.to_string().as_bytes());
    hasher.update(entry.purpose.as_bytes());
    hasher.update(&entry.duration_seconds.to_be_bytes());
    
    if let Some(ref constraints) = entry.constraints {
        hasher.update(constraints.to_string().as_bytes());
    }
    
    hasher.update(&entry.public_key);
    
    hasher.finalize().into()
}

/// Format for HSK proof submission
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsentLedgerProof {
    pub proof_type: String,
    pub citizen_did: String,
    pub public_key: String, // Base64
    pub entry_count: usize,
    pub entries: Vec<ProofEntry>,
    pub merkle_root: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ProofEntry {
    pub entry_id: String,
    pub action: String,
    pub scope: serde_json::Value,
    pub purpose: String,
    pub granted_at: String,
    pub expires_at: String,
    pub previous_entry_id: String,
    pub signature: String, // Base64
}

impl From<&crate::models::ConsentEntry> for ProofEntry {
    fn from(e: &crate::models::ConsentEntry) -> Self {
        Self {
            entry_id: e.entry_id.clone(),
            action: e.action.clone(),
            scope: e.scope.clone(),
            purpose: e.purpose.clone(),
            granted_at: e.granted_at.to_rfc3339(),
            expires_at: e.expires_at.to_rfc3339(),
            previous_entry_id: e.previous_entry_id.clone(),
            signature: base64::encode(&e.signature),
        }
    }
}

/// Generate HSK proof package for a citizen
pub fn generate_hsk_proof(
    citizen_did: &str,
    public_key: &[u8],
    entries: &[crate::models::ConsentEntry],
) -> ConsentLedgerProof {
    let proof_entries: Vec<ProofEntry> = entries.iter()
        .map(|e| e.into())
        .collect();

    ConsentLedgerProof {
        proof_type: "ConsentLedger".to_string(),
        citizen_did: citizen_did.to_string(),
        public_key: base64::encode(public_key),
        entry_count: proof_entries.len(),
        entries: proof_entries,
        merkle_root: None, // Would compute if we had a Merkle tree
    }
}

/// Serialize proof for HSK verifier
pub fn serialize_proof(proof: &ConsentLedgerProof) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(proof)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_hash() {
        let data = b"test data";
        let hash1 = compute_hash(data);
        let hash2 = compute_hash(data);
        
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32);
    }
}
