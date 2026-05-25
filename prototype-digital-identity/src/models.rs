use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct Citizen {
    pub id: Uuid,
    pub did: String,
    pub public_key: Vec<u8>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CitizenResponse {
    pub id: String,
    pub did: String,
    pub public_key: String, // Base64 encoded
    pub created_at: String,
}

impl From<Citizen> for CitizenResponse {
    fn from(c: Citizen) -> Self {
        Self {
            id: c.id.to_string(),
            did: c.did,
            public_key: base64::encode(&c.public_key),
            created_at: c.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ConsentEntry {
    pub id: Uuid,
    pub entry_id: String,
    pub citizen_id: Uuid,
    pub action: String,
    pub scope: serde_json::Value,
    pub purpose: String,
    pub duration_seconds: i64,
    pub granted_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub constraints: Option<serde_json::Value>,
    pub previous_entry_id: String,
    pub public_key: Vec<u8>,
    pub signature: Vec<u8>,
    pub system_signature: Option<Vec<u8>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsentEntryResponse {
    pub entry_id: String,
    pub action: String,
    pub scope: serde_json::Value,
    pub purpose: String,
    pub duration_seconds: i64,
    pub granted_at: String,
    pub expires_at: String,
    pub constraints: Option<serde_json::Value>,
    pub previous_entry_id: String,
    pub public_key: String, // Base64 encoded
    pub signature: String,  // Base64 encoded
    pub system_signature: Option<String>, // Base64 encoded
}

impl From<ConsentEntry> for ConsentEntryResponse {
    fn from(e: ConsentEntry) -> Self {
        Self {
            entry_id: e.entry_id,
            action: e.action,
            scope: e.scope,
            purpose: e.purpose,
            duration_seconds: e.duration_seconds,
            granted_at: e.granted_at.to_rfc3339(),
            expires_at: e.expires_at.to_rfc3339(),
            constraints: e.constraints,
            previous_entry_id: e.previous_entry_id,
            public_key: base64::encode(&e.public_key),
            signature: base64::encode(&e.signature),
            system_signature: e.system_signature.map(|s| base64::encode(&s)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct VerificationResult {
    pub valid: bool,
    pub entry_id: String,
    pub error: Option<String>,
    pub chain_valid: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChainVerificationResult {
    pub citizen_did: String,
    pub valid: bool,
    pub entry_count: usize,
    pub invalid_entries: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccessCheckResult {
    pub citizen_did: String,
    pub resource: String,
    pub purpose: String,
    pub consented: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct HSKProofs {
    pub citizen_did: String,
    pub public_key: String,
    pub entry_count: usize,
    pub latest_entry_id: Option<String>,
    pub entries: Vec<ConsentEntryResponse>,
    pub proof_type: String,
}
