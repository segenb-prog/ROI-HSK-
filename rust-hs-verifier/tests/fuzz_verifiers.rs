#![cfg(feature = "fuzzing")]

use arbitrary::Arbitrary;
use hs_verifier::types::{ConsentEntry, MemoryPassport, DeletionProof, PredictionAttestation};

#[derive(Debug, Arbitrary)]
struct FuzzConsentEntry {
    entry_id: String,
    timestamp: String,
    action: String,
    scope: Vec<String>,
    purpose: String,
    duration_seconds: u64,
    constraints: String,
    public_key: String,
    signature: String,
    previous_entry_id: String,
}

impl From<FuzzConsentEntry> for ConsentEntry {
    fn from(f: FuzzConsentEntry) -> Self {
        Self {
            entry_id: f.entry_id,
            timestamp: f.timestamp,
            action: f.action,
            scope: f.scope,
            purpose: f.purpose,
            duration_seconds: f.duration_seconds,
            constraints: f.constraints,
            public_key: f.public_key,
            signature: f.signature,
            previous_entry_id: f.previous_entry_id,
        }
    }
}

#[test]
fn fuzz_consent_entry_parsing() {
    use hs_verifier::verifiers::ConsentLedgerVerifier;
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;
    
    let mut csprng = OsRng;
    let system_keypair = Keypair::generate(&mut csprng);
    
    let verifier = ConsentLedgerVerifier {
        system_public_key: system_keypair.public,
    };
    
    // Test with various malformed inputs
    let test_cases = vec![
        vec![], // Empty
        vec![0u8; 100], // Random bytes
        b"not json".to_vec(),
        b"[]".to_vec(), // Empty array
        b"null".to_vec(),
    ];
    
    for case in test_cases {
        let _ = verifier.verify(&case);
        // Should not panic
    }
}

#[test]
fn fuzz_jwt_parsing() {
    use hs_verifier::verifiers::MemoryPassportVerifier;
    
    let verifier = MemoryPassportVerifier {
        allowed_issuers: vec!["test".to_string()],
        max_duration_days: 30,
    };
    
    let test_cases = vec![
        "",
        "not.a.jwt",
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        "invalid.base64.here",
        &"a".repeat(10000),
    ];
    
    for case in test_cases {
        let _ = verifier.verify(case);
        // Should not panic
    }
}

#[test]
fn test_signature_boundary_conditions() {
    use hs_verifier::verifiers::verify_signature;
    
    // Test various signature sizes
    let test_cases = vec![
        vec![],                    // Empty
        vec![0u8; 63],            // Too short
        vec![0u8; 64],            // Correct size
        vec![0u8; 65],            // Too long
        vec![0u8; 1000],          // Way too long
    ];
    
    let message = b"test message";
    let public_key = vec![0u8; 32];
    
    for sig in test_cases {
        let _ = verify_signature(message, &sig, &public_key);
        // Should not panic
    }
}

#[test]
fn test_hash_chain_edge_cases() {
    use hs_verifier::verifiers::verify_hash_chain;
    
    // Empty chain
    let empty: Vec<hs_verifier::types::ConsentEntry> = vec![];
    let result = verify_hash_chain(&empty);
    assert!(result.is_ok());
    
    // Single entry
    // Would need valid entry construction
}

#[test]
fn test_merkle_proof_edge_cases() {
    use hs_verifier::types::MerkleProof;
    
    let test_proofs = vec![
        MerkleProof {
            leaf_hash: [0u8; 32],
            path: vec![],
            root_hash: [0u8; 32],
        },
        MerkleProof {
            leaf_hash: [0xff; 32],
            path: vec![([0u8; 32], true), ([0xff; 32], false)],
            root_hash: [0u8; 32],
        },
    ];
    
    for proof in test_proofs {
        // Verify proof structure doesn't cause panics
        let _ = proof.leaf_hash;
        let _ = proof.path.len();
    }
}

#[test]
fn test_certificate_boundary_values() {
    use hs_verifier::certificate::{Certificate, ViolationCertificate};
    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;
    
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    
    // Very long system ID
    let long_id = "a".repeat(10000);
    let cert = Certificate::new_compliant(&long_id, &keypair);
    assert!(cert.verify().unwrap());
    
    // Empty violations list
    let violation = ViolationCertificate::issue(
        "test",
        vec![],
        "",
        &keypair,
    );
    assert!(violation.verify().unwrap());
    
    // Many violations
    let many_violations: Vec<String> = (0..1000).map(|i| format!("LAW_{}", i)).collect();
    let violation = ViolationCertificate::issue(
        "test",
        many_violations,
        "reason",
        &keypair,
    );
    assert!(violation.verify().unwrap());
}

#[test]
fn test_timestamp_edge_cases() {
    use chrono::{DateTime, Utc, Duration};
    
    // Far future
    let future = Utc::now() + Duration::days(365 * 100);
    let _ = future.timestamp();
    
    // Far past
    let past = Utc::now() - Duration::days(365 * 100);
    let _ = past.timestamp();
    
    // Unix epoch
    let epoch: DateTime<Utc> = DateTime::from_timestamp(0, 0).unwrap();
    let _ = epoch.timestamp();
}
