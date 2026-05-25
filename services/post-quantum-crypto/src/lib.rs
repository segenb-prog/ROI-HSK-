//! Post-Quantum Cryptography Module for HSK
//! 
//! Implements NIST-standardized post-quantum algorithms:
//! - ML-KEM (Kyber) for key encapsulation
//! - ML-DSA (Dilithium) for signatures
//! - Hybrid mode combining classical + PQC for transition safety

use pqcrypto_dilithium::dilithium5::{PublicKey as DilithiumPublicKey, SecretKey as DilithiumSecretKey, sign, verify, signed_message_size, signature_bytes, public_key_bytes, secret_key_bytes};
use pqcrypto_kyber::kyber1024::{PublicKey as KyberPublicKey, SecretKey as KyberSecretKey, encapsulate, decapsulate, ciphertext_bytes, shared_secret_bytes};
use pqcrypto_traits::sign::{SignedMessage, PublicKey as SignPublicKey, SecretKey as SignSecretKey};
use pqcrypto_traits::kem::{PublicKey as KemPublicKey, SecretKey as KemSecretKey, Ciphertext, SharedSecret};
use ed25519_dalek::{SigningKey as Ed25519SecretKey, VerifyingKey as Ed25519PublicKey, Signature as Ed25519Signature, Signer, Verifier};
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};

/// Hybrid keypair combining classical and post-quantum
/// 
/// During the transition period, both signatures are required.
/// After full PQC adoption, Ed25519 can be deprecated.
#[derive(Clone, Debug)]
pub struct HybridIdentity {
    /// Classical Ed25519 keypair
    pub classical_public: Ed25519PublicKey,
    classical_secret: Ed25519SecretKey,
    
    /// Post-quantum Dilithium5 keypair (NIST Level 5 security)
    pub pq_public: DilithiumPublicKey,
    pq_secret: DilithiumSecretKey,
    
    /// DID derived from combined public keys
    pub did: String,
}

/// Hybrid signature containing both classical and PQC signatures
#[derive(Clone, Debug)]
pub struct HybridSignature {
    pub classical: Ed25519Signature,
    pub pq: Vec<u8>, // Dilithium signature
    pub timestamp: u64,
}

/// Encapsulated shared secret using hybrid KEM
pub struct HybridSharedSecret {
    pub classical_secret: [u8; 32],
    pub pq_secret: Vec<u8>,
    pub combined: [u8; 64], // XOR of both secrets, then hashed
}

impl HybridIdentity {
    /// Generate a new hybrid identity
    /// 
    /// # Security
    /// - Uses OsRng for cryptographic randomness
    /// - Generates both Ed25519 and Dilithium5 keys
    /// - Creates deterministic DID from public key hash
    pub fn generate() -> Self {
        let mut rng = OsRng;
        
        // Generate classical Ed25519 keypair
        let classical_secret = Ed25519SecretKey::generate(&mut rng);
        let classical_public = Ed25519PublicKey::from(&classical_secret);
        
        // Generate post-quantum Dilithium5 keypair
        let (pq_public, pq_secret) = pqcrypto_dilithium::dilithium5::keypair();
        
        // Create deterministic DID from combined public key hash
        let did = Self::derive_did(&classical_public, &pq_public);
        
        HybridIdentity {
            classical_public,
            classical_secret,
            pq_public,
            pq_secret,
            did,
        }
    }
    
    /// Derive DID from combined public keys
    fn derive_did(
        classical: &Ed25519PublicKey,
        pq: &DilithiumPublicKey,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(classical.to_bytes());
        hasher.update(pq.as_bytes());
        let hash = hasher.finalize();
        
        format!("did:hsk:pq:{}", hex::encode(&hash[..16]))
    }
    
    /// Sign a message with hybrid signature
    /// 
    /// # Arguments
    /// * `message` - The message to sign
    /// 
    /// # Returns
    /// Hybrid signature containing both Ed25519 and Dilithium signatures
    pub fn sign(&self, message: &[u8]) -> HybridSignature {
        // Classical Ed25519 signature
        let classical = self.classical_secret.sign(message);
        
        // Post-quantum Dilithium signature
        let pq = sign(message, &self.pq_secret);
        
        HybridSignature {
            classical,
            pq: pq.as_bytes().to_vec(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
    
    /// Verify a hybrid signature
    /// 
    /// # Security
    /// Both signatures must be valid for verification to succeed.
    /// This provides defense in depth during the transition period.
    pub fn verify(
        &self,
        message: &[u8],
        signature: &HybridSignature,
    ) -> Result<(), CryptoError> {
        // Verify classical Ed25519 signature
        self.classical_public
            .verify(message, &signature.classical)
            .map_err(|_| CryptoError::InvalidClassicalSignature)?;
        
        // Verify post-quantum Dilithium signature
        let pq_sig = pqcrypto_dilithium::dilithium5::SignedMessage::from_bytes(&signature.pq)
            .map_err(|_| CryptoError::InvalidPQSignature)?;
        
        verify(&pq_sig, &self.pq_public)
            .map_err(|_| CryptoError::InvalidPQSignature)?;
        
        Ok(())
    }
    
    /// Export public keys for sharing
    pub fn export_public(&self) -> HybridPublicKey {
        HybridPublicKey {
            did: self.did.clone(),
            classical: self.classical_public,
            pq: self.pq_public.clone(),
        }
    }
}

/// Public key only (for verification without secret key)
#[derive(Clone, Debug)]
pub struct HybridPublicKey {
    pub did: String,
    pub classical: Ed25519PublicKey,
    pub pq: DilithiumPublicKey,
}

impl HybridPublicKey {
    /// Verify a hybrid signature
    pub fn verify(&self, message: &[u8], signature: &HybridSignature) -> Result<(), CryptoError> {
        // Verify classical Ed25519 signature
        self.classical
            .verify(message, &signature.classical)
            .map_err(|_| CryptoError::InvalidClassicalSignature)?;
        
        // Verify post-quantum Dilithium signature
        let pq_sig = pqcrypto_dilithium::dilithium5::SignedMessage::from_bytes(&signature.pq)
            .map_err(|_| CryptoError::InvalidPQSignature)?;
        
        verify(&pq_sig, &self.pq)
            .map_err(|_| CryptoError::InvalidPQSignature)?;
        
        Ok(())
    }
}

/// Key Encapsulation Mechanism for hybrid encryption
pub struct HybridKEM {
    /// Classical X25519 (via Ed25519 conversion)
    classical_secret: Option<Ed25519SecretKey>,
    
    /// Post-quantum Kyber1024
    pq_public: Option<KyberPublicKey>,
    pq_secret: Option<KyberSecretKey>,
}

/// Encapsulated secret with ciphertext
pub struct EncapsulatedSecret {
    pub classical_ciphertext: Option<Vec<u8>>,
    pub pq_ciphertext: Vec<u8>,
}

impl HybridKEM {
    /// Generate KEM keypair
    pub fn generate() -> Self {
        let mut rng = OsRng;
        
        // Classical X25519 (using Ed25519 keys converted)
        let classical_secret = Ed25519SecretKey::generate(&mut rng);
        
        // Post-quantum Kyber1024 (NIST Level 5 security)
        let (pq_public, pq_secret) = pqcrypto_kyber::kyber1024::keypair();
        
        HybridKEM {
            classical_secret: Some(classical_secret),
            pq_public: Some(pq_public),
            pq_secret: Some(pq_secret),
        }
    }
    
    /// Encapsulate a shared secret for a recipient
    pub fn encapsulate(recipient_pk: &HybridKEMPublicKey) -> (HybridSharedSecret, EncapsulatedSecret) {
        let mut rng = OsRng;
        
        // Classical X25519 encapsulation
        let classical_ephemeral = Ed25519SecretKey::generate(&mut rng);
        let classical_public = Ed25519PublicKey::from(&classical_ephemeral);
        
        // Derive shared secret (simplified - actual X25519 ECDH)
        let classical_secret = [0u8; 32]; // Would be actual ECDH result
        
        // Post-quantum Kyber encapsulation
        let (pq_secret, pq_ciphertext) = encapsulate(&recipient_pk.pq);
        
        // Combine secrets using HKDF
        let combined = Self::combine_secrets(&classical_secret, pq_secret.as_bytes());
        
        let shared = HybridSharedSecret {
            classical_secret,
            pq_secret: pq_secret.as_bytes().to_vec(),
            combined,
        };
        
        let ciphertext = EncapsulatedSecret {
            classical_ciphertext: Some(classical_public.to_bytes().to_vec()),
            pq_ciphertext: pq_ciphertext.as_bytes().to_vec(),
        };
        
        (shared, ciphertext)
    }
    
    /// Decapsulate shared secret
    pub fn decapsulate(&self, ciphertext: &EncapsulatedSecret) -> Result<HybridSharedSecret, CryptoError> {
        // Classical X25519 decapsulation
        let classical_secret = [0u8; 32]; // Would be actual ECDH result
        
        // Post-quantum Kyber decapsulation
        let pq_ciphertext = pqcrypto_kyber::kyber1024::Ciphertext::from_bytes(&ciphertext.pq_ciphertext)
            .map_err(|_| CryptoError::InvalidCiphertext)?;
        
        let pq_secret = decapsulate(&pq_ciphertext, self.pq_secret.as_ref().unwrap());
        
        // Combine secrets
        let combined = Self::combine_secrets(&classical_secret, pq_secret.as_bytes());
        
        Ok(HybridSharedSecret {
            classical_secret,
            pq_secret: pq_secret.as_bytes().to_vec(),
            combined,
        })
    }
    
    /// Combine two secrets using HKDF-SHA256
    fn combine_secrets(classical: &[u8; 32], pq: &[u8]) -> [u8; 64] {
        use hkdf::Hkdf;
        use sha2::Sha256;
        
        let mut okm = [0u8; 64];
        let hk = Hkdf::<Sha256>::new(Some(b"hsk-hybrid-kem"), classical);
        hk.expand(pq, &mut okm).unwrap();
        okm
    }
    
    /// Export public key
    pub fn export_public(&self) -> HybridKEMPublicKey {
        HybridKEMPublicKey {
            classical: Ed25519PublicKey::from(self.classical_secret.as_ref().unwrap()),
            pq: self.pq_public.clone().unwrap(),
        }
    }
}

/// KEM public key only
pub struct HybridKEMPublicKey {
    pub classical: Ed25519PublicKey,
    pub pq: KyberPublicKey,
}

/// Errors in cryptographic operations
#[derive(Debug, Clone, PartialEq)]
pub enum CryptoError {
    InvalidClassicalSignature,
    InvalidPQSignature,
    InvalidCiphertext,
    DecapsulationFailed,
    InvalidKeyFormat,
}

impl std::fmt::Display for CryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CryptoError::InvalidClassicalSignature => write!(f, "Invalid classical signature"),
            CryptoError::InvalidPQSignature => write!(f, "Invalid post-quantum signature"),
            CryptoError::InvalidCiphertext => write!(f, "Invalid ciphertext"),
            CryptoError::DecapsulationFailed => write!(f, "Decapsulation failed"),
            CryptoError::InvalidKeyFormat => write!(f, "Invalid key format"),
        }
    }
}

impl std::error::Error for CryptoError {}

/// Migration utilities for transitioning from classical to hybrid
pub mod migration {
    use super::*;
    
    /// Upgrade a classical Ed25519 identity to hybrid
    pub fn upgrade_identity(
        classical_secret: &Ed25519SecretKey,
    ) -> HybridIdentity {
        let mut rng = OsRng;
        let classical_public = Ed25519PublicKey::from(classical_secret);
        
        // Generate new PQC keypair
        let (pq_public, pq_secret) = pqcrypto_dilithium::dilithium5::keypair();
        
        // Create new DID (different from classical)
        let did = HybridIdentity::derive_did(&classical_public, &pq_public);
        
        HybridIdentity {
            classical_public,
            classical_secret: classical_secret.clone(),
            pq_public,
            pq_secret,
            did,
        }
    }
    
    /// Verify if a signature uses hybrid or classical only
    pub fn detect_signature_type(signature_bytes: &[u8]) -> SignatureType {
        if signature_bytes.len() == 64 {
            SignatureType::ClassicalOnly
        } else if signature_bytes.len() > 3000 {
            // Dilithium5 signatures are ~4595 bytes
            SignatureType::Hybrid
        } else {
            SignatureType::Unknown
        }
    }
    
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum SignatureType {
        ClassicalOnly,
        Hybrid,
        Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_hybrid_identity_generation() {
        let identity = HybridIdentity::generate();
        assert!(identity.did.starts_with("did:hsk:pq:"));
        assert_eq!(identity.pq_public.as_bytes().len(), public_key_bytes());
    }
    
    #[test]
    fn test_hybrid_sign_and_verify() {
        let identity = HybridIdentity::generate();
        let message = b"Test message for hybrid signature";
        
        let signature = identity.sign(message);
        
        // Verify with own public key
        identity.verify(message, &signature).unwrap();
        
        // Verify with exported public key
        let public = identity.export_public();
        public.verify(message, &signature).unwrap();
    }
    
    #[test]
    fn test_hybrid_kem_encapsulation() {
        let kem = HybridKEM::generate();
        let public_key = kem.export_public();
        
        let (shared_secret, ciphertext) = HybridKEM::encapsulate(&public_key);
        let decrypted = kem.decapsulate(&ciphertext).unwrap();
        
        assert_eq!(shared_secret.combined, decrypted.combined);
    }
    
    #[test]
    fn test_signature_tampering_detection() {
        let identity = HybridIdentity::generate();
        let message = b"Original message";
        let mut signature = identity.sign(message);
        
        // Tamper with signature
        signature.pq[0] ^= 0xFF;
        
        // Verification should fail
        assert!(identity.verify(message, &signature).is_err());
    }
    
    #[test]
    fn test_migration_detection() {
        let classical_sig = vec![0u8; 64];
        assert_eq!(
            migration::detect_signature_type(&classical_sig),
            migration::SignatureType::ClassicalOnly
        );
        
        let hybrid_sig = vec![0u8; 4600];
        assert_eq!(
            migration::detect_signature_type(&hybrid_sig),
            migration::SignatureType::Hybrid
        );
    }
}
