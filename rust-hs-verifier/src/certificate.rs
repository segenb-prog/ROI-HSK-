use crate::{Hash, VerifyError};
use chrono::{DateTime, Utc};
use ed25519_dalek::{Keypair, Signer, PublicKey, Signature};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Certificate {
    pub certificate_id: Hash,
    pub system_id: String,
    pub evaluation_time: DateTime<Utc>,
    pub hs_compliant: bool,
    pub violations: Vec<String>,
    pub missing_proofs: Vec<String>,
    pub invalid_proofs: Vec<String>,
    pub issuer_public_key: String,
    pub issuer_signature: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ViolationCertificate {
    pub certificate_id: Hash,
    pub system_id: String,
    pub evaluation_time: DateTime<Utc>,
    pub hs_compliant: bool,
    pub violations: Vec<String>,
    pub missing_proofs: Vec<String>,
    pub invalid_proofs: Vec<String>,
    pub details: String,
    pub issuer_public_key: String,
    pub issuer_signature: String,
}

impl ViolationCertificate {
    pub fn issue(
        system_id: &str,
        violations: Vec<String>,
        reason: &str,
        signing_key: &Keypair,
    ) -> Self {
        let evaluation_time = Utc::now();
        
        let mut cert = Self {
            certificate_id: [0u8; 32],
            system_id: system_id.to_string(),
            evaluation_time,
            hs_compliant: false,
            violations,
            missing_proofs: vec![],
            invalid_proofs: vec![],
            details: reason.to_string(),
            issuer_public_key: base64::encode(signing_key.public.to_bytes()),
            issuer_signature: String::new(),
        };
        
        cert.certificate_id = cert.compute_id();
        cert.issuer_signature = cert.sign(signing_key);
        
        cert
    }
    
    fn compute_id(&self) -> Hash {
        let mut hasher = Sha256::new();
        
        hasher.update(self.system_id.as_bytes());
        hasher.update(self.evaluation_time.to_rfc3339().as_bytes());
        hasher.update(&[self.hs_compliant as u8]);
        
        for v in &self.violations {
            hasher.update(v.as_bytes());
        }
        
        hasher.finalize().into()
    }
    
    fn sign(&self, signing_key: &Keypair) -> String {
        let message = self.certificate_id;
        let signature = signing_key.sign(&message);
        base64::encode(signature.to_bytes())
    }
    
    pub fn verify(&self) -> Result<bool, VerifyError> {
        let computed_id = self.compute_id();
        if computed_id != self.certificate_id {
            return Ok(false);
        }
        
        let pk_bytes = base64::decode(&self.issuer_public_key)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let pk = PublicKey::from_bytes(&pk_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let sig_bytes = base64::decode(&self.issuer_signature)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        pk.verify(&self.certificate_id, &sig)
            .map(|_| true)
            .map_err(|_| VerifyError::InvalidSignature)
    }
}

impl Certificate {
    pub fn new_compliant(
        system_id: &str,
        signing_key: &Keypair,
    ) -> Self {
        let evaluation_time = Utc::now();
        
        let mut cert = Self {
            certificate_id: [0u8; 32],
            system_id: system_id.to_string(),
            evaluation_time,
            hs_compliant: true,
            violations: vec![],
            missing_proofs: vec![],
            invalid_proofs: vec![],
            issuer_public_key: base64::encode(signing_key.public.to_bytes()),
            issuer_signature: String::new(),
        };
        
        cert.certificate_id = cert.compute_id();
        cert.issuer_signature = cert.sign(signing_key);
        
        cert
    }
    
    fn compute_id(&self) -> Hash {
        let mut hasher = Sha256::new();
        
        hasher.update(self.system_id.as_bytes());
        hasher.update(self.evaluation_time.to_rfc3339().as_bytes());
        hasher.update(&[self.hs_compliant as u8]);
        
        hasher.finalize().into()
    }
    
    fn sign(&self, signing_key: &Keypair) -> String {
        let message = self.certificate_id;
        let signature = signing_key.sign(&message);
        base64::encode(signature.to_bytes())
    }
    
    pub fn verify(&self) -> Result<bool, VerifyError> {
        let computed_id = self.compute_id();
        if computed_id != self.certificate_id {
            return Ok(false);
        }
        
        let pk_bytes = base64::decode(&self.issuer_public_key)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let pk = PublicKey::from_bytes(&pk_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        let sig_bytes = base64::decode(&self.issuer_signature)
            .map_err(|_| VerifyError::InvalidSignature)?;
        let sig = Signature::from_bytes(&sig_bytes)
            .map_err(|_| VerifyError::InvalidSignature)?;
        
        pk.verify(&self.certificate_id, &sig)
            .map(|_| true)
            .map_err(|_| VerifyError::InvalidSignature)
    }
    
    pub fn to_violation_certificate(&self) -> Option<ViolationCertificate> {
        if self.hs_compliant {
            return None;
        }
        
        Some(ViolationCertificate {
            certificate_id: self.certificate_id,
            system_id: self.system_id.clone(),
            evaluation_time: self.evaluation_time,
            hs_compliant: self.hs_compliant,
            violations: self.violations.clone(),
            missing_proofs: self.missing_proofs.clone(),
            invalid_proofs: self.invalid_proofs.clone(),
            details: "Violation detected".to_string(),
            issuer_public_key: self.issuer_public_key.clone(),
            issuer_signature: self.issuer_signature.clone(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CertificateTransparencyEntry {
    pub certificate_id: String,
    pub system_id: String,
    pub timestamp: DateTime<Utc>,
    pub compliant: bool,
    pub merkle_root: String,
    pub previous_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;

    #[test]
    fn test_certificate_creation_and_verification() {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        let cert = Certificate::new_compliant("test-system", &keypair);
        
        assert!(cert.hs_compliant);
        assert!(cert.verify().unwrap());
        assert_eq!(cert.system_id, "test-system");
    }

    #[test]
    fn test_violation_certificate() {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        
        let cert = ViolationCertificate::issue(
            "test-system",
            vec!["LAW_1".to_string()],
            "Test violation",
            &keypair,
        );
        
        assert!(!cert.hs_compliant);
        assert!(cert.verify().unwrap());
        assert_eq!(cert.violations, vec!["LAW_1"]);
    }
}
