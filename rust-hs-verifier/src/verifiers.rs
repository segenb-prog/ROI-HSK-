use crate::{
    ConsentEntry, MemoryPassport, DeletionProof, PredictionAttestation,
    VerifyError, Hash, types::InferenceLogEntry,
};
use ed25519_dalek::{PublicKey, Signature, Verifier};
use sha2::{Sha256, Digest};
use chrono::DateTime;

pub struct ConsentLedgerVerifier {
    pub system_public_key: PublicKey,
}

impl ConsentLedgerVerifier {
    pub fn verify(&self, data: &[u8]) -> Result<Hash, VerifyError> {
        let entries: Vec<ConsentEntry> = serde_json::from_slice(data)
            .map_err(|e| VerifyError::Serialization(e.to_string()))?;
        
        if entries.is_empty() {
            return Err(VerifyError::MissingField);
        }
        
        let mut prev_hash = [0u8; 32];
        
        for (i, entry) in entries.iter().enumerate() {
            let computed_id = Self::compute_entry_hash(entry, prev_hash);
            
            let entry_id_bytes = hex::decode(&entry.entry_id)
                .map_err(|_| VerifyError::HashChainBroken)?;
            if entry_id_bytes != computed_id.to_vec() {
                return Err(VerifyError::HashChainBroken);
            }
            
            let sig_bytes = base64::decode(&entry.signature)
                .map_err(|_| VerifyError::InvalidSignature)?;
            let sig = Signature::from_bytes(&sig_bytes)
                .map_err(|_| VerifyError::InvalidSignature)?;
            
            let pk_bytes = base64::decode(&entry.public_key)
                .map_err(|_| VerifyError::InvalidSignature)?;
            let pk = PublicKey::from_bytes(&pk_bytes)
                .map_err(|_| VerifyError::InvalidSignature)?;
            
            pk.verify(entry.entry_id.as_bytes(), &sig)
                .map_err(|_| VerifyError::InvalidSignature)?;
            
            if i == 0 {
                if pk != self.system_public_key {
                    return Err(VerifyError::InvalidSignature);
                }
            }
            
            if i > 0 {
                let prev_time = DateTime::parse_from_rfc3339(&entries[i-1].timestamp)
                    .map_err(|_| VerifyError::Ambiguous)?;
                let curr_time = DateTime::parse_from_rfc3339(&entry.timestamp)
                    .map_err(|_| VerifyError::Ambiguous)?;
                
                if curr_time < prev_time {
                    return Err(VerifyError::HashChainBroken);
                }
            }
            
            prev_hash = computed_id;
        }
        
        Ok(prev_hash)
    }
    
    fn compute_entry_hash(entry: &ConsentEntry, prev_hash: Hash) -> Hash {
        let mut hasher = Sha256::new();
        
        hasher.update(&prev_hash);
        hasher.update(entry.timestamp.as_bytes());
        hasher.update(entry.action.as_bytes());
        hasher.update(serde_json::to_string(&entry.scope).unwrap().as_bytes());
        hasher.update(entry.purpose.as_bytes());
        hasher.update(&entry.duration_seconds.to_be_bytes());
        hasher.update(entry.constraints.as_bytes());
        
        if let Ok(pk_bytes) = base64::decode(&entry.public_key) {
            hasher.update(&pk_bytes);
        }
        
        hasher.finalize().into()
    }
}

pub struct MemoryPassportVerifier {
    pub allowed_issuers: Vec<String>,
    pub max_duration_days: u64,
}

impl MemoryPassportVerifier {
    pub fn verify(&self, jwt_token: &str) -> Result<PassportData, VerifyError> {
        use jsonwebtoken::{decode, decode_header, Algorithm, Validation};
        
        let header = decode_header(jwt_token)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let validation = Validation::new(Algorithm::EdDSA);
        let token = decode::<serde_json::Value>(jwt_token, &jsonwebtoken::DecodingKey::from_secret(&[]), &validation)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let claims = token.claims;
        
        let iss = claims.get("iss")
            .and_then(|v| v.as_str())
            .ok_or(VerifyError::MissingField)?;
            
        if !self.allowed_issuers.contains(&iss.to_string()) {
            return Err(VerifyError::InvalidSignature);
        }
        
        let exp = claims.get("exp")
            .and_then(|v| v.as_u64())
            .ok_or(VerifyError::Expired)?;
            
        let now = chrono::Utc::now().timestamp() as u64;
        if exp < now {
            return Err(VerifyError::Expired);
        }
        
        let iat = claims.get("iat")
            .and_then(|v| v.as_u64())
            .ok_or(VerifyError::MissingField)?;
            
        let duration_days = (exp - iat) / (24 * 3600);
        if duration_days > self.max_duration_days {
            return Err(VerifyError::Expired);
        }
        
        let scope = claims.get("scope")
            .and_then(|v| v.as_str())
            .ok_or(VerifyError::MissingField)?;
            
        if !Self::validate_scopes(scope) {
            return Err(VerifyError::Ambiguous);
        }
        
        Ok(PassportData {
            issuer: iss.to_string(),
            expires_at: exp,
            scope: scope.to_string(),
        })
    }
    
    fn validate_scopes(scope: &str) -> bool {
        let parts: Vec<&str> = scope.split(':').collect();
        parts.len() == 3 && 
        !parts[0].is_empty() && 
        !parts[1].is_empty() &&
        parts[2].contains("no_derivatives")
    }
}

#[derive(Debug)]
pub struct PassportData {
    pub issuer: String,
    pub expires_at: u64,
    pub scope: String,
}

pub struct DeletionProofVerifier {
    pub empty_state_root: Hash,
}

impl DeletionProofVerifier {
    pub fn verify(&self, data: &[u8], system_key: &PublicKey) -> Result<bool, VerifyError> {
        let proof: DeletionProof = serde_cbor::from_slice(data)
            .map_err(|e| VerifyError::Serialization(e.to_string()))?;
        
        if proof.storage_root_post != self.empty_state_root {
            return Err(VerifyError::HashChainBroken);
        }
        
        for merkle_proof in &proof.merkle_proofs {
            if !self.verify_merkle_proof(merkle_proof) {
                return Err(VerifyError::HashChainBroken);
            }
            
            if merkle_proof.root_hash != proof.storage_root_pre {
                return Err(VerifyError::HashChainBroken);
            }
        }
        
        let sig_bytes = base64::decode(&proof.signature)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let mut message = Vec::new();
        message.extend_from_slice(&proof.storage_root_post);
        message.extend_from_slice(&proof.timestamp.to_be_bytes());
        
        system_key.verify(&message, &sig)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let now = chrono::Utc::now().timestamp() as u64;
        if proof.timestamp > now || now - proof.timestamp > 72 * 3600 {
            return Err(VerifyError::Expired);
        }
        
        Ok(true)
    }
    
    fn verify_merkle_proof(&self, proof: &crate::types::MerkleProof) -> bool {
        let mut current_hash = proof.leaf_hash;
        
        for (sibling_hash, is_left) in &proof.path {
            let mut hasher = Sha256::new();
            
            if *is_left {
                hasher.update(sibling_hash);
                hasher.update(&current_hash);
            } else {
                hasher.update(&current_hash);
                hasher.update(sibling_hash);
            }
            
            current_hash = hasher.finalize().into();
        }
        
        current_hash == proof.root_hash
    }
}

pub struct PredictionScopeVerifier {
    pub permitted_inferences: Vec<String>,
}

impl PredictionScopeVerifier {
    pub fn verify(&self, data: &[u8], system_key: &PublicKey) -> Result<bool, VerifyError> {
        let attestation: PredictionAttestation = serde_cbor::from_slice(data)
            .map_err(|e| VerifyError::Serialization(e.to_string()))?;
        
        let sig_bytes = base64::decode(&attestation.attestation_signature)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let mut message = Vec::new();
        message.extend_from_slice(&attestation.model_hash);
        message.extend_from_slice(&attestation.timestamp.to_be_bytes());
        
        system_key.verify(&message, &sig)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        for log in &attestation.inference_logs {
            if !log.scope_match {
                return Err(VerifyError::Ambiguous);
            }
            
            if !self.permitted_inferences.contains(&log.inference_type) {
                return Err(VerifyError::Ambiguous);
            }
        }
        
        let mut prev_timestamp = 0;
        for log in &attestation.inference_logs {
            if log.timestamp < prev_timestamp {
                return Err(VerifyError::Ambiguous);
            }
            prev_timestamp = log.timestamp;
        }
        
        Ok(true)
    }
    
    pub fn compute_model_hash(model_weights: &[u8]) -> Hash {
        let mut hasher = Sha256::new();
        hasher.update(model_weights);
        hasher.finalize().into()
    }
}

pub struct IntegratedVerifier {
    pub system_public_key: PublicKey,
    pub allowed_issuers: Vec<String>,
    pub empty_state_root: Hash,
}

impl IntegratedVerifier {
    pub fn verify_all(
        &self,
        proofs: &[(crate::ProofType, Vec<u8>)],
    ) -> crate::VerificationResult {
        use crate::{ProofType, VerificationResult};
        
        let required = vec![
            ProofType::ConsentLedger,
            ProofType::MemoryPassport,
            ProofType::DeletionProof,
            ProofType::PredictionScope,
        ];
        
        let mut missing = Vec::new();
        let mut invalid = Vec::new();
        
        for proof_type in &required {
            let proof_data = proofs.iter()
                .find(|(pt, _)| pt == proof_type)
                .map(|(_, data)| data);
            
            match proof_data {
                Some(data) => {
                    let result = match proof_type {
                        ProofType::ConsentLedger => {
                            let verifier = ConsentLedgerVerifier {
                                system_public_key: self.system_public_key,
                            };
                            verifier.verify(data).map(|_| ())
                        }
                        ProofType::MemoryPassport => {
                            let verifier = MemoryPassportVerifier {
                                allowed_issuers: self.allowed_issuers.clone(),
                                max_duration_days: 30,
                            };
                            let jwt = std::str::from_utf8(data)
                                .map_err(|_| VerifyError::Ambiguous)?;
                            verifier.verify(jwt).map(|_| ())
                        }
                        ProofType::DeletionProof => {
                            let verifier = DeletionProofVerifier {
                                empty_state_root: self.empty_state_root,
                            };
                            verifier.verify(data, &self.system_public_key).map(|_| ())
                        }
                        ProofType::PredictionScope => {
                            let verifier = PredictionScopeVerifier {
                                permitted_inferences: vec![],
                            };
                            verifier.verify(data, &self.system_public_key).map(|_| ())
                        }
                    };
                    
                    if result.is_err() {
                        invalid.push(proof_type.clone());
                    }
                }
                None => {
                    missing.push(proof_type.clone());
                }
            }
        }
        
        if missing.is_empty() && invalid.is_empty() {
            VerificationResult::Compliant
        } else {
            VerificationResult::Violation {
                missing_proofs: missing,
                invalid_proofs: invalid,
                reason: format!("Missing: {:?}, Invalid: {:?}", missing, invalid),
            }
        }
    }
}
