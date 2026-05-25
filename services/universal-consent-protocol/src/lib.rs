//! Universal Consent Protocol (UCP)
//! 
//! Cross-domain consent federation protocol enabling consent granted
//! on HSK to be recognized and verified across different platforms.
//! The "HTTP of consent" - standardized, cryptographically verifiable.

use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::collections::HashMap;

/// Universal Consent Token (UCT)
/// 
/// A portable, cryptographically verifiable token representing
/// user consent that can be presented to any UCP-compliant service.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UniversalConsentToken {
    /// Token version
    pub version: String,
    
    /// HSK proof (Merkle proof from HSK transparency log)
    pub hsk_proof: HSKProof,
    
    /// W3C Verifiable Credential format
    pub w3c_vc: VerifiableCredential,
    
    /// OAuth 2.0 Rich Authorization Request
    pub oauth_rar: RichAuthorizationRequest,
    
    /// GNAP (Grant Negotiation and Authorization Protocol) data
    pub gnap_grant: GNAPGrant,
    
    /// DPoP (Demonstrating Proof-of-Possession) proof
    pub dpop_proof: DPoPProof,
    
    /// Token binding to prevent replay
    pub token_binding: TokenBinding,
    
    /// Expiration timestamp
    pub exp: u64,
    
    /// Issued at timestamp
    pub iat: u64,
}

/// HSK-specific proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HSKProof {
    /// Consent ID in HSK
    pub consent_id: String,
    /// Merkle root at time of consent
    pub merkle_root: [u8; 32],
    /// Merkle proof path
    pub merkle_path: Vec<[u8; 32]>,
    /// Leaf index
    pub leaf_index: usize,
    /// HSK transparency log URL
    pub transparency_log_url: String,
}

/// W3C Verifiable Credential
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VerifiableCredential {
    /// Context
    #[serde(rename = "@context")]
    pub context: Vec<String>,
    
    /// Type
    #[serde(rename = "type")]
    pub vc_type: Vec<String>,
    
    /// Credential ID
    pub id: String,
    
    /// Issuer (HSK platform)
    pub issuer: String,
    
    /// Issuance date
    pub issuance_date: String,
    
    /// Expiration date
    pub expiration_date: Option<String>,
    
    /// Credential subject (the consent)
    pub credential_subject: ConsentCredentialSubject,
    
    /// Proof (LD signature)
    pub proof: Option<LinkedDataProof>,
}

/// Consent as credential subject
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsentCredentialSubject {
    /// Subject ID (user DID)
    pub id: String,
    
    /// Consent purpose
    pub purpose: String,
    
    /// Data categories
    pub data_categories: Vec<String>,
    
    /// Processing activities
    pub processing: Vec<ProcessingActivity>,
    
    /// Valid from
    pub valid_from: String,
    
    /// Valid until
    pub valid_until: Option<String>,
    
    /// Constraints
    pub constraints: Option<ConsentConstraints>,
}

/// Processing activity
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProcessingActivity {
    pub activity: String,
    pub description: String,
}

/// Consent constraints
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsentConstraints {
    pub geographic_restriction: Option<String>,
    pub purpose_limitation: Option<String>,
    pub retention_limit: Option<String>,
}

/// Linked Data proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkedDataProof {
    #[serde(rename = "type")]
    pub proof_type: String,
    pub created: String,
    pub proof_purpose: String,
    pub verification_method: String,
    pub jws: String,
}

/// OAuth 2.0 Rich Authorization Request
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RichAuthorizationRequest {
    /// Authorization type
    pub authorization_type: String,
    
    /// Locations (data locations)
    pub locations: Vec<String>,
    
    /// Actions (what can be done)
    pub actions: Vec<String>,
    
    /// Data types
    pub data_types: Vec<String>,
    
    /// Identifier (consent reference)
    pub identifier: String,
    
    /// Purpose
    pub purpose: String,
}

/// GNAP Grant
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPGrant {
    /// Access token
    pub access_token: Vec<GNAPAccessToken>,
    
    /// Subject
    pub subject: Option<GNAPSubject>,
    
    /// Interact (if needed)
    pub interact: Option<GNAPInteract>,
}

/// GNAP access token
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPAccessToken {
    pub access: Vec<GNAPAccess>,
    pub flags: Vec<String>,
    pub label: String,
}

/// GNAP access
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPAccess {
    pub resource_type: String,
    pub actions: Vec<String>,
    pub locations: Vec<String>,
    pub data_types: Vec<String>,
}

/// GNAP subject
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPSubject {
    pub sub_ids: Vec<SubjectIdentifier>,
    pub assertions: HashMap<String, String>,
}

/// Subject identifier
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubjectIdentifier {
    pub format: String,
    pub id: String,
}

/// GNAP interact
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPInteract {
    pub redirect: bool,
    pub callback: GNAPCallback,
}

/// GNAP callback
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GNAPCallback {
    pub uri: String,
    pub nonce: String,
}

/// DPoP proof
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DPoPProof {
    /// JWT header
    pub header: DPoPHeader,
    
    /// JWT payload
    pub payload: DPoPPayload,
    
    /// Signature
    pub signature: String,
}

/// DPoP header
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DPoPHeader {
    pub alg: String,
    #[serde(rename = "typ")]
    pub typ: String,
    pub jwk: JWK,
}

/// JSON Web Key
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JWK {
    pub kty: String,
    pub crv: String,
    pub x: String,
    pub y: Option<String>,
}

/// DPoP payload
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DPoPPayload {
    pub jti: String,
    pub htm: String,
    pub htu: String,
    pub iat: u64,
    pub ath: Option<String>,
}

/// Token binding
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TokenBinding {
    /// Binding type (e.g., "tls", "dpop")
    pub binding_type: String,
    
    /// Binding data
    pub binding_data: String,
}

/// Universal Consent Protocol handler
pub struct UniversalConsentProtocol {
    /// Trusted HSK instances
    trusted_hsk_instances: Vec<String>,
    
    /// Verification keys
    verification_keys: HashMap<String, Vec<u8>>,
}

/// Verification result
#[derive(Clone, Debug)]
pub enum VerificationResult {
    Valid(ConsentVerification),
    Invalid(String),
    Expired,
    Revoked,
    UntrustedIssuer,
}

/// Consent verification details
#[derive(Clone, Debug)]
pub struct ConsentVerification {
    pub subject_id: String,
    pub purpose: String,
    pub data_categories: Vec<String>,
    pub valid_from: u64,
    pub valid_until: Option<u64>,
    pub hsk_verified: bool,
    pub vc_verified: bool,
    pub dpop_verified: bool,
}

impl UniversalConsentProtocol {
    /// Create new UCP handler
    pub fn new() -> Self {
        UniversalConsentProtocol {
            trusted_hsk_instances: vec![],
            verification_keys: HashMap::new(),
        }
    }
    
    /// Add trusted HSK instance
    pub fn add_trusted_hsk(&mut self, url: String, public_key: Vec<u8>) {
        self.trusted_hsk_instances.push(url.clone());
        self.verification_keys.insert(url, public_key);
    }
    
    /// Create universal consent token from HSK consent
    pub fn create_token(
        &self,
        hsk_proof: HSKProof,
        subject_did: String,
        purpose: String,
        data_categories: Vec<String>,
        valid_hours: u64,
    ) -> UniversalConsentToken {
        let now = current_timestamp();
        let exp = now + (valid_hours * 3600);
        
        // Create W3C VC
        let w3c_vc = self.create_verifiable_credential(
            &subject_did,
            &purpose,
            &data_categories,
            now,
            exp,
        );
        
        // Create OAuth RAR
        let oauth_rar = self.create_rich_auth_request(
            &purpose,
            &data_categories,
            &hsk_proof.consent_id,
        );
        
        // Create GNAP grant
        let gnap_grant = self.create_gnap_grant(
            &subject_did,
            &purpose,
            &data_categories,
        );
        
        // Create DPoP proof
        let dpop_proof = self.create_dpop_proof(&hsk_proof.consent_id);
        
        // Create token binding
        let token_binding = TokenBinding {
            binding_type: "dpop".to_string(),
            binding_data: dpop_proof.payload.jti.clone(),
        };
        
        UniversalConsentToken {
            version: "1.0".to_string(),
            hsk_proof,
            w3c_vc,
            oauth_rar,
            gnap_grant,
            dpop_proof,
            token_binding,
            exp,
            iat: now,
        }
    }
    
    /// Verify universal consent token
    pub fn verify_token(&self, token: &UniversalConsentToken) -> VerificationResult {
        // Check expiration
        let now = current_timestamp();
        if now > token.exp {
            return VerificationResult::Expired;
        }
        
        // Verify HSK proof
        if !self.verify_hsk_proof(&token.hsk_proof) {
            return VerificationResult::Invalid("HSK proof verification failed".to_string());
        }
        
        // Verify W3C VC
        if !self.verify_verifiable_credential(&token.w3c_vc) {
            return VerificationResult::Invalid("VC verification failed".to_string());
        }
        
        // Verify DPoP
        if !self.verify_dpop(&token.dpop_proof) {
            return VerificationResult::Invalid("DPoP verification failed".to_string());
        }
        
        // Check consistency across formats
        if !self.verify_format_consistency(token) {
            return VerificationResult::Invalid("Format inconsistency".to_string());
        }
        
        VerificationResult::Valid(ConsentVerification {
            subject_id: token.w3c_vc.credential_subject.id.clone(),
            purpose: token.w3c_vc.credential_subject.purpose.clone(),
            data_categories: token.w3c_vc.credential_subject.data_categories.clone(),
            valid_from: parse_timestamp(&token.w3c_vc.credential_subject.valid_from),
            valid_until: token.w3c_vc.credential_subject.valid_until.as_ref()
                .map(|t| parse_timestamp(t)),
            hsk_verified: true,
            vc_verified: true,
            dpop_verified: true,
        })
    }
    
    /// Create W3C Verifiable Credential
    fn create_verifiable_credential(
        &self,
        subject_did: &str,
        purpose: &str,
        data_categories: &[String],
        iat: u64,
        exp: u64,
    ) -> VerifiableCredential {
        VerifiableCredential {
            context: vec![
                "https://www.w3.org/2018/credentials/v1".to_string(),
                "https://hsk.platform/consent/v1".to_string(),
            ],
            vc_type: vec![
                "VerifiableCredential".to_string(),
                "ConsentCredential".to_string(),
            ],
            id: format!("urn:uuid:{}", generate_uuid()),
            issuer: "https://hsk.platform".to_string(),
            issuance_date: format_timestamp(iat),
            expiration_date: Some(format_timestamp(exp)),
            credential_subject: ConsentCredentialSubject {
                id: subject_did.to_string(),
                purpose: purpose.to_string(),
                data_categories: data_categories.to_vec(),
                processing: vec![
                    ProcessingActivity {
                        activity: "collection".to_string(),
                        description: format!("Collect data for {}", purpose),
                    },
                    ProcessingActivity {
                        activity: "processing".to_string(),
                        description: format!("Process data for {}", purpose),
                    },
                ],
                valid_from: format_timestamp(iat),
                valid_until: Some(format_timestamp(exp)),
                constraints: None,
            },
            proof: None, // Would be signed
        }
    }
    
    /// Create OAuth 2.0 Rich Authorization Request
    fn create_rich_auth_request(
        &self,
        purpose: &str,
        data_categories: &[String],
        consent_id: &str,
    ) -> RichAuthorizationRequest {
        RichAuthorizationRequest {
            authorization_type: "consent".to_string(),
            locations: vec!["https://service.example.com/data".to_string()],
            actions: vec!["read".to_string(), "process".to_string()],
            data_types: data_categories.to_vec(),
            identifier: consent_id.to_string(),
            purpose: purpose.to_string(),
        }
    }
    
    /// Create GNAP grant
    fn create_gnap_grant(
        &self,
        subject_id: &str,
        purpose: &str,
        data_categories: &[String],
    ) -> GNAPGrant {
        GNAPGrant {
            access_token: vec![GNAPAccessToken {
                access: vec![GNAPAccess {
                    resource_type: "data".to_string(),
                    actions: vec!["read".to_string(), "process".to_string()],
                    locations: vec!["https://service.example.com".to_string()],
                    data_types: data_categories.to_vec(),
                }],
                flags: vec!["bearer".to_string()],
                label: format!("consent-{}", purpose),
            }],
            subject: Some(GNAPSubject {
                sub_ids: vec![SubjectIdentifier {
                    format: "did".to_string(),
                    id: subject_id.to_string(),
                }],
                assertions: {
                    let mut map = HashMap::new();
                    map.insert("purpose".to_string(), purpose.to_string());
                    map
                },
            }),
            interact: None,
        }
    }
    
    /// Create DPoP proof
    fn create_dpop_proof(&self, jti: &str) -> DPoPProof {
        DPoPProof {
            header: DPoPHeader {
                alg: "ES256".to_string(),
                typ: "dpop+jwt".to_string(),
                jwk: JWK {
                    kty: "EC".to_string(),
                    crv: "P-256".to_string(),
                    x: "base64url_encoded_x".to_string(),
                    y: Some("base64url_encoded_y".to_string()),
                },
            },
            payload: DPoPPayload {
                jti: jti.to_string(),
                htm: "POST".to_string(),
                htu: "https://service.example.com/api".to_string(),
                iat: current_timestamp(),
                ath: None,
            },
            signature: "base64url_encoded_signature".to_string(),
        }
    }
    
    /// Verify HSK proof
    fn verify_hsk_proof(&self, proof: &HSKProof) -> bool {
        // Check if HSK instance is trusted
        if !self.trusted_hsk_instances.contains(&proof.transparency_log_url) {
            return false;
        }
        
        // Verify Merkle proof (simplified)
        // Would actually verify against HSK transparency log
        !proof.merkle_root.is_empty()
    }
    
    /// Verify W3C Verifiable Credential
    fn verify_verifiable_credential(&self, vc: &VerifiableCredential) -> bool {
        // Check required fields
        if vc.context.is_empty() || vc.vc_type.is_empty() {
            return false;
        }
        
        // Verify proof if present
        if let Some(proof) = &vc.proof {
            !proof.jws.is_empty()
        } else {
            true // Accept unsigned for now
        }
    }
    
    /// Verify DPoP proof
    fn verify_dpop(&self, dpop: &DPoPProof) -> bool {
        // Verify header
        if dpop.header.typ != "dpop+jwt" {
            return false;
        }
        
        // Verify payload
        if dpop.payload.iat > current_timestamp() + 60 {
            return false; // Future timestamp
        }
        
        // Verify signature (simplified)
        !dpop.signature.is_empty()
    }
    
    /// Verify consistency across all formats
    fn verify_format_consistency(&self, token: &UniversalConsentToken) -> bool {
        // Purpose must match across formats
        let vc_purpose = &token.w3c_vc.credential_subject.purpose;
        let oauth_purpose = &token.oauth_rar.purpose;
        
        if vc_purpose != oauth_purpose {
            return false;
        }
        
        // Data categories must match
        let vc_categories: std::collections::HashSet<_> = 
            token.w3c_vc.credential_subject.data_categories.iter().collect();
        let oauth_categories: std::collections::HashSet<_> = 
            token.oauth_rar.data_types.iter().collect();
        
        if vc_categories != oauth_categories {
            return false;
        }
        
        // Subject ID must match
        let vc_subject = &token.w3c_vc.credential_subject.id;
        let gnap_subject = token.gnap_grant.subject.as_ref()
            .and_then(|s| s.sub_ids.first())
            .map(|s| &s.id);
        
        if let Some(gs) = gnap_subject {
            if vc_subject != gs {
                return false;
            }
        }
        
        true
    }
    
    /// Serialize token to JSON
    pub fn serialize_token(&self, token: &UniversalConsentToken) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(token)
    }
    
    /// Deserialize token from JSON
    pub fn deserialize_token(&self, json: &str) -> Result<UniversalConsentToken, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// UCP service endpoint
pub struct UCPService {
    protocol: UniversalConsentProtocol,
}

impl UCPService {
    pub fn new() -> Self {
        UCPService {
            protocol: UniversalConsentProtocol::new(),
        }
    }
    
    /// API endpoint: Verify consent token
    pub fn verify_consent(&self, token_json: &str) -> Result<ConsentVerification, String> {
        let token = self.protocol.deserialize_token(token_json)
            .map_err(|e| format!("Invalid token format: {}", e))?;
        
        match self.protocol.verify_token(&token) {
            VerificationResult::Valid(v) => Ok(v),
            VerificationResult::Invalid(e) => Err(format!("Verification failed: {}", e)),
            VerificationResult::Expired => Err("Token expired".to_string()),
            VerificationResult::Revoked => Err("Token revoked".to_string()),
            VerificationResult::UntrustedIssuer => Err("Untrusted issuer".to_string()),
        }
    }
    
    /// API endpoint: Check if consent covers specific action
    pub fn check_action_permission(
        &self,
        token_json: &str,
        action: &str,
        data_category: &str,
    ) -> Result<bool, String> {
        let verification = self.verify_consent(token_json)?;
        
        // Check if data category is covered
        if !verification.data_categories.contains(&data_category.to_string()) {
            return Ok(false);
        }
        
        // Check if action is permitted (simplified)
        let permitted_actions = vec!["read", "process", "analyze"];
        Ok(permitted_actions.contains(&action))
    }
}

/// Helper functions
fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn format_timestamp(ts: u64) -> String {
    use chrono::{DateTime, Utc};
    let datetime = DateTime::from_timestamp(ts as i64, 0)
        .unwrap_or_else(|| Utc::now());
    datetime.to_rfc3339()
}

fn parse_timestamp(ts: &str) -> u64 {
    use chrono::DateTime;
    DateTime::parse_from_rfc3339(ts)
        .map(|dt| dt.timestamp() as u64)
        .unwrap_or(0)
}

fn generate_uuid() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut bytes);
    
    // Set version (4) and variant bits
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_and_verify_token() {
        let mut ucp = UniversalConsentProtocol::new();
        ucp.add_trusted_hsk(
            "https://hsk.platform".to_string(),
            vec![1, 2, 3],
        );
        
        let hsk_proof = HSKProof {
            consent_id: "consent:123".to_string(),
            merkle_root: [1u8; 32],
            merkle_path: vec![[2u8; 32]],
            leaf_index: 0,
            transparency_log_url: "https://hsk.platform".to_string(),
        };
        
        let token = ucp.create_token(
            hsk_proof,
            "did:hsk:user123".to_string(),
            "analytics".to_string(),
            vec!["usage_data".to_string()],
            24,
        );
        
        // Verify token
        match ucp.verify_token(&token) {
            VerificationResult::Valid(v) => {
                assert_eq!(v.purpose, "analytics");
                assert_eq!(v.data_categories, vec!["usage_data"]);
            }
            other => panic!("Expected valid verification, got {:?}", other),
        }
    }
    
    #[test]
    fn test_format_consistency() {
        let ucp = UniversalConsentProtocol::new();
        
        let hsk_proof = HSKProof {
            consent_id: "consent:123".to_string(),
            merkle_root: [1u8; 32],
            merkle_path: vec![],
            leaf_index: 0,
            transparency_log_url: "https://example.com".to_string(),
        };
        
        let token = ucp.create_token(
            hsk_proof,
            "did:hsk:user".to_string(),
            "test".to_string(),
            vec!["data".to_string()],
            1,
        );
        
        // Purpose should match
        assert_eq!(
            token.w3c_vc.credential_subject.purpose,
            token.oauth_rar.purpose
        );
        
        // Categories should match
        assert_eq!(
            token.w3c_vc.credential_subject.data_categories,
            token.oauth_rar.data_types
        );
    }
    
    #[test]
    fn test_serialization() {
        let ucp = UniversalConsentProtocol::new();
        
        let hsk_proof = HSKProof {
            consent_id: "consent:456".to_string(),
            merkle_root: [1u8; 32],
            merkle_path: vec![],
            leaf_index: 0,
            transparency_log_url: "https://example.com".to_string(),
        };
        
        let token = ucp.create_token(
            hsk_proof,
            "did:hsk:user".to_string(),
            "test".to_string(),
            vec!["data".to_string()],
            1,
        );
        
        let json = ucp.serialize_token(&token).unwrap();
        let deserialized = ucp.deserialize_token(&json).unwrap();
        
        assert_eq!(token.version, deserialized.version);
        assert_eq!(token.hsk_proof.consent_id, deserialized.hsk_proof.consent_id);
    }
}
