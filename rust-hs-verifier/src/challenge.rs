use crate::{ProofRequest, ProofType, SystemResponse, types::Hash};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use std::path::Path;
use uuid::Uuid;

pub fn generate_proof_request(system_id: &str, timeout_hours: i64) -> ProofRequest {
    let issued_at = Utc::now();
    let deadline = issued_at + Duration::hours(timeout_hours);
    
    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    
    ProofRequest {
        request_id: Uuid::new_v4().to_string(),
        system_id: system_id.to_string(),
        requested_proofs: vec![
            ProofType::ConsentLedger,
            ProofType::MemoryPassport,
            ProofType::DeletionProof,
            ProofType::PredictionScope,
        ],
        issued_at,
        deadline,
        nonce,
    }
}

pub fn save_proof_request(request: &ProofRequest, path: &Path) -> Result<(), std::io::Error> {
    let json = serde_json::to_string_pretty(request)?;
    std::fs::write(path, json)
}

pub fn load_proof_request(path: &Path) -> Result<ProofRequest, Box<dyn std::error::Error>> {
    let data = std::fs::read_to_string(path)?;
    let request: ProofRequest = serde_json::from_str(&data)?;
    Ok(request)
}

pub async fn send_challenge(
    endpoint: &str,
    request: &ProofRequest,
) -> Result<SystemResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .post(format!("{}/challenge", endpoint))
        .json(request)
        .send()
        .await?;
    
    if response.status().is_success() {
        let system_response: SystemResponse = response.json().await?;
        Ok(system_response)
    } else {
        Err(format!("Challenge rejected: {}", response.status()).into())
    }
}

pub fn is_within_deadline(request: &ProofRequest) -> bool {
    Utc::now() <= request.deadline
}

pub fn time_remaining(request: &ProofRequest) -> Duration {
    let now = Utc::now();
    if request.deadline > now {
        request.deadline - now
    } else {
        Duration::zero()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_generate_proof_request() {
        let request = generate_proof_request("test-system", 72);
        
        assert_eq!(request.system_id, "test-system");
        assert_eq!(request.requested_proofs.len(), 4);
        assert!(request.requested_proofs.contains(&ProofType::ConsentLedger));
        assert!(request.requested_proofs.contains(&ProofType::MemoryPassport));
        assert!(request.requested_proofs.contains(&ProofType::DeletionProof));
        assert!(request.requested_proofs.contains(&ProofType::PredictionScope));
        
        let duration = request.deadline - request.issued_at;
        assert_eq!(duration.num_hours(), 72);
    }

    #[test]
    fn test_save_and_load_proof_request() {
        let request = generate_proof_request("test-system", 24);
        let temp_file = NamedTempFile::new().unwrap();
        
        save_proof_request(&request, temp_file.path()).unwrap();
        let loaded = load_proof_request(temp_file.path()).unwrap();
        
        assert_eq!(loaded.system_id, request.system_id);
        assert_eq!(loaded.requested_proofs, request.requested_proofs);
    }
}
