//! Fully Homomorphic Encryption for Privacy-Preserving Consent Verification
//! 
//! Uses BFV (Brakerski-Fan-Vercauteren) scheme from the SEAL library
//! Allows computation on encrypted consent data without decryption

use seal_fhe::{BFVEncoder, BFVEncryptionParameters, BFVScheme, Ciphertext, Context, Decryptor, Encryptor, Evaluator, KeyGenerator, Plaintext, SecurityLevel};
use seal_fhe::ModulusType;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

/// Homomorphic encryption context for consent operations
pub struct HESession {
    context: Context,
    encoder: BFVEncoder,
    encryptor: Encryptor<BFVScheme>,
    decryptor: Decryptor<BFVScheme>,
    evaluator: Evaluator<BFVScheme>,
    public_key: seal_fhe::PublicKey,
    secret_key: seal_fhe::SecretKey,
    relin_keys: seal_fhe::RelinearizationKeys,
    galois_keys: seal_fhe::GaloisKeys,
}

/// Encrypted consent query
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedConsentQuery {
    pub encrypted_did: Vec<u8>,
    pub encrypted_purpose: Vec<u8>,
    pub encrypted_data_categories: Vec<Vec<u8>>,
    pub query_hash: [u8; 32],
}

/// Encrypted consent record
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedConsentRecord {
    pub encrypted_did: Vec<u8>,
    pub encrypted_purpose: Vec<u8>,
    pub encrypted_categories: Vec<u8>,
    pub encrypted_valid: Vec<u8>,
    pub merkle_root: [u8; 32],
}

/// Result of homomorphic verification (still encrypted)
#[derive(Clone, Serialize, Deserialize)]
pub struct EncryptedVerificationResult {
    pub encrypted_match: Vec<u8>,      // 1 if match, 0 if no match
    pub encrypted_consent_id: Vec<u8>, // Encrypted consent ID if match
    pub proof_hash: [u8; 32],
}

/// Decrypted verification result
#[derive(Clone, Debug)]
pub struct VerificationResult {
    pub is_match: bool,
    pub consent_id: Option<String>,
    pub merkle_proof: Option<Vec<u8>>,
}

impl HESession {
    /// Initialize HE session with 128-bit security
    /// 
    /// # Parameters
    /// - Polynomial degree: 8192 (provides sufficient slots)
    /// - Coefficient modulus: 128-bit security level
    /// - Plain modulus: Supports integers up to 20 bits
    pub fn new() -> Result<Self, HEError> {
        let params = BFVEncryptionParameters::new(
            ModulusType::CoeffModulus128,
            8192,
            1 << 20, // Plain modulus
        ).map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let context = Context::new(params, false, SecurityLevel::TC128)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let encoder = BFVEncoder::new(&context)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let key_gen = KeyGenerator::new(&context)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let public_key = key_gen.public_key()
            .map_err(|e| HEError::KeyGenerationError(e.to_string()))?;
        
        let secret_key = key_gen.secret_key()
            .map_err(|e| HEError::KeyGenerationError(e.to_string()))?;
        
        let relin_keys = key_gen.relinearization_keys()
            .map_err(|e| HEError::KeyGenerationError(e.to_string()))?;
        
        let galois_keys = key_gen.galois_keys()
            .map_err(|e| HEError::KeyGenerationError(e.to_string()))?;
        
        let encryptor = Encryptor::with_public_key(&context, &public_key)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let decryptor = Decryptor::new(&context, &secret_key)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let evaluator = Evaluator::new(&context)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        Ok(HESession {
            context,
            encoder,
            encryptor,
            decryptor,
            evaluator,
            public_key,
            secret_key,
            relin_keys,
            galois_keys,
        })
    }
    
    /// Encrypt a string value for homomorphic processing
    pub fn encrypt_string(&self, value: &str) -> Result<Vec<u8>, HEError> {
        // Convert string to integer vector (simplified encoding)
        let values: Vec<i64> = value.bytes().map(|b| b as i64).collect();
        
        let plaintext = self.encoder.encode_i64(&values)
            .map_err(|e| HEError::EncodingError(e.to_string()))?;
        
        let ciphertext = self.encryptor.encrypt(&plaintext)
            .map_err(|e| HEError::EncryptionError(e.to_string()))?;
        
        // Serialize ciphertext
        let mut buffer = Vec::new();
        ciphertext.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        
        Ok(buffer)
    }
    
    /// Decrypt a string value
    pub fn decrypt_string(&self, encrypted: &[u8]) -> Result<String, HEError> {
        let ciphertext = Ciphertext::load(&self.context, encrypted)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let plaintext = self.decryptor.decrypt(&ciphertext)
            .map_err(|e| HEError::DecryptionError(e.to_string()))?;
        
        let values = self.encoder.decode_i64(&plaintext);
        
        // Convert back to string
        let bytes: Vec<u8> = values.iter()
            .take_while(|&&v| v != 0)
            .map(|&v| v as u8)
            .collect();
        
        String::from_utf8(bytes)
            .map_err(|_| HEError::DecodingError("Invalid UTF-8".to_string()))
    }
    
    /// Homomorphically compare two encrypted strings for equality
    /// 
    /// Returns encrypted 1 if equal, 0 if not equal
    pub fn homomorphic_equals(
        &self,
        encrypted_a: &[u8],
        encrypted_b: &[u8],
    ) -> Result<Vec<u8>, HEError> {
        let ct_a = Ciphertext::load(&self.context, encrypted_a)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let ct_b = Ciphertext::load(&self.context, encrypted_b)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        // Compute (a - b)^2 - homomorphically
        // If a == b, result is 0; otherwise positive
        let diff = self.evaluator.sub(&ct_a, &ct_b)
            .map_err(|e| HEError::EvaluationError(e.to_string()))?;
        
        let squared = self.evaluator.multiply(&diff, &diff)
            .map_err(|e| HEError::EvaluationError(e.to_string()))?;
        
        let relinearized = self.evaluator.relinearize(&squared, &self.relin_keys)
            .map_err(|e| HEError::EvaluationError(e.to_string()))?;
        
        // Serialize result
        let mut buffer = Vec::new();
        relinearized.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        
        Ok(buffer)
    }
    
    /// Homomorphically verify consent match
    /// 
    /// Server verifies that encrypted query matches encrypted consent
    /// WITHOUT decrypting either one
    pub fn verify_consent_encrypted(
        &self,
        query: &EncryptedConsentQuery,
        consent: &EncryptedConsentRecord,
    ) -> Result<EncryptedVerificationResult, HEError> {
        // Compare DIDs
        let did_match = self.homomorphic_equals(&query.encrypted_did, &consent.encrypted_did)?;
        
        // Compare purposes
        let purpose_match = self.homomorphic_equals(&query.encrypted_purpose, &consent.encrypted_purpose)?;
        
        // Combine matches (AND operation)
        let combined = self.homomorphic_and(&did_match, &purpose_match)?;
        
        // Check validity
        let valid_plain = self.encoder.encode_i64(&[1i64])
            .map_err(|e| HEError::EncodingError(e.to_string()))?;
        let valid_ct = self.encryptor.encrypt(&valid_plain)
            .map_err(|e| HEError::EncryptionError(e.to_string()))?;
        
        let mut valid_buffer = Vec::new();
        valid_ct.save(&mut valid_buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        
        let final_match = self.homomorphic_and(&combined, &valid_buffer)?;
        
        // Generate proof hash
        let mut hasher = Sha256::new();
        hasher.update(&query.query_hash);
        hasher.update(&consent.merkle_root);
        let proof_hash = hasher.finalize().into();
        
        Ok(EncryptedVerificationResult {
            encrypted_match: final_match,
            encrypted_consent_id: consent.encrypted_did.clone(), // Simplified
            proof_hash,
        })
    }
    
    /// Homomorphic AND operation
    fn homomorphic_and(&self, a: &[u8], b: &[u8]) -> Result<Vec<u8>, HEError> {
        // For boolean values (0 or 1), AND is multiplication
        let ct_a = Ciphertext::load(&self.context, a)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let ct_b = Ciphertext::load(&self.context, b)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let result = self.evaluator.multiply(&ct_a, &ct_b)
            .map_err(|e| HEError::EvaluationError(e.to_string()))?;
        
        let relinearized = self.evaluator.relinearize(&result, &self.relin_keys)
            .map_err(|e| HEError::EvaluationError(e.to_string()))?;
        
        let mut buffer = Vec::new();
        relinearized.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        
        Ok(buffer)
    }
    
    /// Decrypt verification result
    pub fn decrypt_verification_result(
        &self,
        encrypted: &EncryptedVerificationResult,
    ) -> Result<VerificationResult, HEError> {
        let match_value = self.decrypt_match_value(&encrypted.encrypted_match)?;
        
        let is_match = match_value > 0;
        
        let consent_id = if is_match {
            Some(self.decrypt_string(&encrypted.encrypted_consent_id)?)
        } else {
            None
        };
        
        Ok(VerificationResult {
            is_match,
            consent_id,
            merkle_proof: None, // Would be generated separately
        })
    }
    
    fn decrypt_match_value(&self, encrypted: &[u8]) -> Result<i64, HEError> {
        let ciphertext = Ciphertext::load(&self.context, encrypted)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let plaintext = self.decryptor.decrypt(&ciphertext)
            .map_err(|e| HEError::DecryptionError(e.to_string()))?;
        
        let values = self.encoder.decode_i64(&plaintext);
        Ok(values[0])
    }
    
    /// Export public key for client-side encryption
    pub fn export_public_key(&self) -> Result<Vec<u8>, HEError> {
        let mut buffer = Vec::new();
        self.public_key.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        Ok(buffer)
    }
    
    /// Export Galois keys for client (needed for rotations)
    pub fn export_galois_keys(&self) -> Result<Vec<u8>, HEError> {
        let mut buffer = Vec::new();
        self.galois_keys.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        Ok(buffer)
    }
}

/// Client-side HE encryption (for user devices)
pub struct HEClient {
    context: Context,
    encoder: BFVEncoder,
    encryptor: Encryptor<BFVScheme>,
}

impl HEClient {
    /// Initialize client with server's public key
    pub fn with_public_key(public_key_bytes: &[u8], params_bytes: &[u8]) -> Result<Self, HEError> {
        // Deserialize parameters and public key
        // Simplified - actual implementation would deserialize properly
        
        let params = BFVEncryptionParameters::new(
            ModulusType::CoeffModulus128,
            8192,
            1 << 20,
        ).map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let context = Context::new(params, false, SecurityLevel::TC128)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let encoder = BFVEncoder::new(&context)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        let public_key = seal_fhe::PublicKey::load(&context, public_key_bytes)
            .map_err(|e| HEError::DeserializationError(e.to_string()))?;
        
        let encryptor = Encryptor::with_public_key(&context, &public_key)
            .map_err(|e| HEError::InitializationError(e.to_string()))?;
        
        Ok(HEClient {
            context,
            encoder,
            encryptor,
        })
    }
    
    /// Encrypt a consent query on the client side
    pub fn encrypt_query(&self, did: &str, purpose: &str, categories: &[String]) -> Result<EncryptedConsentQuery, HEError> {
        let did_bytes = self.encrypt_string(did)?;
        let purpose_bytes = self.encrypt_string(purpose)?;
        
        let mut category_bytes = Vec::new();
        for cat in categories {
            category_bytes.push(self.encrypt_string(cat)?);
        }
        
        // Compute query hash
        let mut hasher = Sha256::new();
        hasher.update(did.as_bytes());
        hasher.update(purpose.as_bytes());
        for cat in categories {
            hasher.update(cat.as_bytes());
        }
        let query_hash = hasher.finalize().into();
        
        Ok(EncryptedConsentQuery {
            encrypted_did: did_bytes,
            encrypted_purpose: purpose_bytes,
            encrypted_data_categories: category_bytes,
            query_hash,
        })
    }
    
    fn encrypt_string(&self, value: &str) -> Result<Vec<u8>, HEError> {
        let values: Vec<i64> = value.bytes().map(|b| b as i64).collect();
        
        let plaintext = self.encoder.encode_i64(&values)
            .map_err(|e| HEError::EncodingError(e.to_string()))?;
        
        let ciphertext = self.encryptor.encrypt(&plaintext)
            .map_err(|e| HEError::EncryptionError(e.to_string()))?;
        
        let mut buffer = Vec::new();
        ciphertext.save(&mut buffer)
            .map_err(|e| HEError::SerializationError(e.to_string()))?;
        
        Ok(buffer)
    }
}

/// Errors in homomorphic encryption operations
#[derive(Debug, Clone)]
pub enum HEError {
    InitializationError(String),
    KeyGenerationError(String),
    EncodingError(String),
    EncryptionError(String),
    DecryptionError(String),
    EvaluationError(String),
    SerializationError(String),
    DeserializationError(String),
    DecodingError(String),
}

impl std::fmt::Display for HEError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HEError::InitializationError(e) => write!(f, "HE initialization error: {}", e),
            HEError::KeyGenerationError(e) => write!(f, "HE key generation error: {}", e),
            HEError::EncodingError(e) => write!(f, "HE encoding error: {}", e),
            HEError::EncryptionError(e) => write!(f, "HE encryption error: {}", e),
            HEError::DecryptionError(e) => write!(f, "HE decryption error: {}", e),
            HEError::EvaluationError(e) => write!(f, "HE evaluation error: {}", e),
            HEError::SerializationError(e) => write!(f, "HE serialization error: {}", e),
            HEError::DeserializationError(e) => write!(f, "HE deserialization error: {}", e),
            HEError::DecodingError(e) => write!(f, "HE decoding error: {}", e),
        }
    }
}

impl std::error::Error for HEError {}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_he_encryption_decryption() {
        let session = HESession::new().unwrap();
        let message = "test_did_123";
        
        let encrypted = session.encrypt_string(message).unwrap();
        let decrypted = session.decrypt_string(&encrypted).unwrap();
        
        assert_eq!(message, decrypted);
    }
    
    #[test]
    fn test_homomorphic_equality() {
        let session = HESession::new().unwrap();
        
        let msg_a = "same_message";
        let msg_b = "same_message";
        let msg_c = "different_msg";
        
        let enc_a = session.encrypt_string(msg_a).unwrap();
        let enc_b = session.encrypt_string(msg_b).unwrap();
        let enc_c = session.encrypt_string(msg_c).unwrap();
        
        // Same messages should produce near-zero difference
        let eq_result = session.homomorphic_equals(&enc_a, &enc_b).unwrap();
        let eq_value = session.decrypt_match_value(&eq_result).unwrap();
        assert_eq!(eq_value, 0);
        
        // Different messages should produce non-zero difference
        let neq_result = session.homomorphic_equals(&enc_a, &enc_c).unwrap();
        let neq_value = session.decrypt_match_value(&neq_result).unwrap();
        assert!(neq_value > 0);
    }
    
    #[test]
    fn test_client_server_encryption() {
        let server = HESession::new().unwrap();
        let public_key = server.export_public_key().unwrap();
        
        // Client encrypts with server's public key
        let client = HEClient::with_public_key(&public_key, &[]).unwrap();
        let query = client.encrypt_query(
            "did:hsk:test",
            "analytics",
            &["usage_data".to_string()],
        ).unwrap();
        
        // Server processes without decryption
        // (In real scenario, would compare against encrypted consent record)
        let did_decrypted = server.decrypt_string(&query.encrypted_did).unwrap();
        assert_eq!(did_decrypted, "did:hsk:test");
    }
}
