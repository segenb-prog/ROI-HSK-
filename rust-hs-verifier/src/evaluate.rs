use crate::{
    ProofRequest, SystemResponse, VerificationResult, VerifyError,
    verifiers::{IntegratedVerifier, ConsentLedgerVerifier, MemoryPassportVerifier, DeletionProofVerifier, PredictionScopeVerifier},
};
use ed25519_dalek::PublicKey;
use tracing::{info, warn, error};

pub fn evaluate_proofs(
    request: &ProofRequest,
    response: Option<SystemResponse>,
) -> Result<VerificationResult, VerifyError> {
    info!("Evaluating proofs for system: {}", request.system_id);
    
    let response = match response {
        Some(r) => r,
        None => {
            warn!("No response received before deadline");
            return Ok(VerificationResult::Violation {
                missing_proofs: request.requested_proofs.clone(),
                invalid_proofs: vec![],
                reason: "No response before deadline".to_string(),
            });
        }
    };
    
    if response.request_id != request.request_id {
        return Err(VerifyError::Ambiguous);
    }
    
    if response.submitted_at > request.deadline {
        warn!("Response submitted after deadline");
        return Ok(VerificationResult::Violation {
            missing_proofs: request.requested_proofs.clone(),
            invalid_proofs: vec![],
            reason: "Response submitted after deadline".to_string(),
        });
    }
    
    let system_pk = load_system_public_key()?;
    let empty_root = load_empty_state_root()?;
    let allowed_issuers = load_allowed_issuers()?;
    
    let verifier = IntegratedVerifier {
        system_public_key: system_pk,
        allowed_issuers,
        empty_state_root: empty_root,
    };
    
    let result = verifier.verify_all(&response.provided_proofs);
    
    match &result {
        VerificationResult::Compliant => {
            info!("System {} is HSK compliant", request.system_id);
        }
        VerificationResult::Violation { missing_proofs, invalid_proofs, reason } => {
            warn!(
                "System {} violated HSK: missing={:?}, invalid={:?}",
                request.system_id, missing_proofs, invalid_proofs
            );
        }
    }
    
    Ok(result)
}

pub fn evaluate_single_proof(
    proof_type: &crate::ProofType,
    data: &[u8],
    system_key: &PublicKey,
) -> Result<(), VerifyError> {
    match proof_type {
        crate::ProofType::ConsentLedger => {
            let verifier = ConsentLedgerVerifier {
                system_public_key: *system_key,
            };
            verifier.verify(data)?;
            Ok(())
        }
        crate::ProofType::MemoryPassport => {
            let verifier = MemoryPassportVerifier {
                allowed_issuers: load_allowed_issuers()?,
                max_duration_days: 30,
            };
            let jwt = std::str::from_utf8(data)
                .map_err(|_| VerifyError::Ambiguous)?;
            verifier.verify(jwt)?;
            Ok(())
        }
        crate::ProofType::DeletionProof => {
            let verifier = DeletionProofVerifier {
                empty_state_root: load_empty_state_root()?,
            };
            verifier.verify(data, system_key)?;
            Ok(())
        }
        crate::ProofType::PredictionScope => {
            let verifier = PredictionScopeVerifier {
                permitted_inferences: vec![],
            };
            verifier.verify(data, system_key)?;
            Ok(())
        }
    }
}

fn load_system_public_key() -> Result<PublicKey, VerifyError> {
    let key_path = std::env::var("HSK_SYSTEM_PUBLIC_KEY")
        .unwrap_or_else(|_| "system.pub".to_string());
    
    let key_data = std::fs::read(&key_path)
        .map_err(|_| VerifyError::MissingField)?;
    
    let key_bytes = if key_data.len() == 32 {
        key_data.try_into().unwrap()
    } else {
        hex::decode(&key_data)
            .map_err(|_| VerifyError::InvalidSignature)?
            .try_into()
            .map_err(|_| VerifyError::InvalidSignature)?
    };
    
    PublicKey::from_bytes(&key_bytes)
        .map_err(|_| VerifyError::InvalidSignature)
}

fn load_empty_state_root() -> Result<[u8; 32], VerifyError> {
    let root_hex = std::env::var("HSK_EMPTY_STATE_ROOT")
        .unwrap_or_else(|_| "0".repeat(64));
    
    hex::decode(&root_hex)
        .map_err(|_| VerifyError::Ambiguous)?
        .try_into()
        .map_err(|_| VerifyError::Ambiguous)
}

fn load_allowed_issuers() -> Result<Vec<String>, VerifyError> {
    let issuers_str = std::env::var("HSK_ALLOWED_ISSUERS")
        .unwrap_or_else(|_| "hskernel.gov".to_string());
    
    Ok(issuers_str.split(',').map(|s| s.to_string()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::challenge::generate_proof_request;

    #[test]
    fn test_evaluate_no_response() {
        let request = generate_proof_request("test-system", 72);
        
        let result = evaluate_proofs(&request, None).unwrap();
        
        match result {
            VerificationResult::Violation { reason, .. } => {
                assert!(reason.contains("No response"));
            }
            _ => panic!("Expected violation"),
        }
    }
}
