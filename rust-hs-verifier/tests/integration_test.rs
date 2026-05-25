use hs_verifier::{
    challenge::{generate_proof_request, save_proof_request, load_proof_request},
    evaluate::evaluate_proofs,
    certificate::{Certificate, ViolationCertificate},
    issuer::{generate_keyring, Keyring},
    types::{ProofRequest, ProofType, SystemResponse, VerificationResult},
    verifiers::{
        ConsentLedgerVerifier, MemoryPassportVerifier, DeletionProofVerifier,
        PredictionScopeVerifier, IntegratedVerifier
    },
};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use tempfile::NamedTempFile;

#[test]
fn test_full_verification_flow() {
    // Generate issuer keys
    let mut csprng = OsRng;
    let issuer_keypair = Keypair::generate(&mut csprng);
    
    // Generate system keys
    let system_keypair = Keypair::generate(&mut csprng);
    
    // Create a challenge
    let request = generate_proof_request("test-system", 72);
    assert_eq!(request.system_id, "test-system");
    assert_eq!(request.requested_proofs.len(), 4);
    
    // Create a compliant response
    let response = SystemResponse {
        request_id: request.request_id.clone(),
        system_id: "test-system".to_string(),
        provided_proofs: vec![], // Empty for this test
        submitted_at: chrono::Utc::now(),
    };
    
    // Evaluate (should fail due to missing proofs)
    let result = evaluate_proofs(&request, Some(response));
    
    match result {
        Ok(VerificationResult::Violation { missing_proofs, .. }) => {
            assert_eq!(missing_proofs.len(), 4);
        }
        _ => panic!("Expected violation due to missing proofs"),
    }
}

#[test]
fn test_certificate_creation_and_verification() {
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    
    // Create compliant certificate
    let cert = Certificate::new_compliant("test-system", &keypair);
    assert!(cert.hs_compliant);
    assert!(cert.verify().unwrap());
    
    // Create violation certificate
    let violation = ViolationCertificate::issue(
        "test-system",
        vec!["LAW_1".to_string()],
        "Test violation",
        &keypair,
    );
    assert!(!violation.hs_compliant);
    assert!(violation.verify().unwrap());
}

#[test]
fn test_keyring_operations() {
    // Generate keyring
    let keyring = generate_keyring().unwrap();
    assert_eq!(keyring.keys.len(), 1);
    
    let temp_file = NamedTempFile::new().unwrap();
    keyring.save(temp_file.path()).unwrap();
    
    // Load keyring
    let loaded = Keyring::load(temp_file.path()).unwrap();
    assert_eq!(loaded.current_key_id(), keyring.current_key_id());
}

#[test]
fn test_proof_request_serialization() {
    let request = generate_proof_request("test-system", 72);
    
    let temp_file = NamedTempFile::new().unwrap();
    save_proof_request(&request, temp_file.path()).unwrap();
    
    let loaded = load_proof_request(temp_file.path()).unwrap();
    assert_eq!(loaded.system_id, request.system_id);
    assert_eq!(loaded.requested_proofs, request.requested_proofs);
}

#[test]
fn test_consent_ledger_verifier() {
    use hs_verifier::types::ConsentEntry;
    
    let mut csprng = OsRng;
    let system_keypair = Keypair::generate(&mut csprng);
    
    let verifier = ConsentLedgerVerifier {
        system_public_key: system_keypair.public,
    };
    
    // Create valid consent entries
    let entries = vec![
        ConsentEntry {
            entry_id: "test".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            action: "grant".to_string(),
            scope: vec!["resource1".to_string()],
            purpose: "test".to_string(),
            duration_seconds: 3600,
            constraints: "{}".to_string(),
            public_key: base64::encode(system_keypair.public.to_bytes()),
            signature: base64::encode(system_keypair.sign(b"test").to_bytes()),
            previous_entry_id: "0".repeat(64),
        }
    ];
    
    let data = serde_json::to_vec(&entries).unwrap();
    
    // This will fail because entry_id doesn't match computed hash
    // but it tests the verifier structure
    let result = verifier.verify(&data);
    assert!(result.is_err());
}

#[test]
fn test_integrated_verifier() {
    let mut csprng = OsRng;
    let system_keypair = Keypair::generate(&mut csprng);
    
    let verifier = IntegratedVerifier {
        system_public_key: system_keypair.public,
        allowed_issuers: vec!["test".to_string()],
        empty_state_root: [0u8; 32],
    };
    
    // Empty proofs should result in all missing
    let proofs: Vec<(ProofType, Vec<u8>)> = vec![];
    let result = verifier.verify_all(&proofs);
    
    match result {
        VerificationResult::Violation { missing_proofs, .. } => {
            assert_eq!(missing_proofs.len(), 4);
        }
        _ => panic!("Expected violation"),
    }
}

#[tokio::test]
async fn test_server_health_endpoint() {
    use hs_verifier::server;
    
    // This would require a full server test setup
    // For now, just verify the module compiles
}
