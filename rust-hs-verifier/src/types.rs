use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub type Hash = [u8; 32];
pub type SignatureBytes = [u8; 64];
pub type PublicKeyBytes = [u8; 32];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProofType {
    ConsentLedger,
    MemoryPassport,
    DeletionProof,
    PredictionScope,
}

impl std::fmt::Display for ProofType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofType::ConsentLedger => write!(f, "ConsentLedger"),
            ProofType::MemoryPassport => write!(f, "MemoryPassport"),
            ProofType::DeletionProof => write!(f, "DeletionProof"),
            ProofType::PredictionScope => write!(f, "PredictionScope"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProofRequest {
    pub request_id: String,
    pub system_id: String,
    pub requested_proofs: Vec<ProofType>,
    pub issued_at: DateTime<Utc>,
    pub deadline: DateTime<Utc>,
    pub nonce: [u8; 32],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemResponse {
    pub request_id: String,
    pub system_id: String,
    pub provided_proofs: Vec<(ProofType, Vec<u8>)>,
    pub submitted_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum VerificationResult {
    Compliant,
    Violation {
        missing_proofs: Vec<ProofType>,
        invalid_proofs: Vec<ProofType>,
        reason: String,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum VerifyError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Hash chain broken")]
    HashChainBroken,
    #[error("Expired")]
    Expired,
    #[error("Missing required field")]
    MissingField,
    #[error("Ambiguous or malformed data")]
    Ambiguous,
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConsentEntry {
    pub entry_id: String,
    pub timestamp: String,
    pub action: String,
    pub scope: Vec<String>,
    pub purpose: String,
    pub duration_seconds: u64,
    pub constraints: String,
    pub public_key: String,
    pub signature: String,
    pub previous_entry_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryPassport {
    pub issuer: String,
    pub subject: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub scope: String,
    pub constraints: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletionProof {
    pub storage_root_pre: Hash,
    pub storage_root_post: Hash,
    pub merkle_proofs: Vec<MerkleProof>,
    pub timestamp: u64,
    pub signature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MerkleProof {
    pub leaf_hash: Hash,
    pub path: Vec<(Hash, bool)>,
    pub root_hash: Hash,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PredictionAttestation {
    pub model_hash: Hash,
    pub training_data_hashes: Vec<Hash>,
    pub allowed_inferences: Vec<String>,
    pub inference_logs: Vec<InferenceLogEntry>,
    pub attestation_signature: String,
    pub timestamp: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InferenceLogEntry {
    pub input_hash: Hash,
    pub output_hash: Hash,
    pub inference_type: String,
    pub timestamp: u64,
    pub scope_match: bool,
}
