use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Path as AxumPath},
    http::StatusCode,
};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use serde_json::json;

use crate::{
    challenge::{generate_proof_request, save_proof_request},
    evaluate::evaluate_proofs,
    certificate::{Certificate, ViolationCertificate},
    issuer::{load_keyring, Keyring},
    types::{ProofRequest, SystemResponse, VerificationResult},
};

pub struct AppState {
    pub keyring: RwLock<Keyring>,
    pub challenges: RwLock<std::collections::HashMap<String, ProofRequest>>,
    pub certificates: RwLock<Vec<Certificate>>,
}

pub async fn start_server(port: u16, keyring_path: std::path::PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let keyring = load_keyring().unwrap_or_else(|_| {
        info!("No keyring found, generating new one");
        crate::issuer::generate_keyring().expect("Failed to generate keyring")
    });
    
    let state = Arc::new(AppState {
        keyring: RwLock::new(keyring),
        challenges: RwLock::new(std::collections::HashMap::new()),
        certificates: RwLock::new(Vec::new()),
    });
    
    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/challenge", post(create_challenge))
        .route("/challenge/:request_id", get(get_challenge))
        .route("/response", post(submit_response))
        .route("/certificates", get(list_certificates))
        .route("/certificates/:cert_id", get(get_certificate))
        .route("/verify/:cert_id", get(verify_certificate))
        .with_state(state);
    
    let addr = format!("0.0.0.0:{}", port);
    info!("HSK Verifier server starting on http://{}", addr);
    
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "name": "HSK Falsification Machine",
        "version": "0.1.0",
        "endpoints": [
            "/health",
            "/challenge",
            "/response",
            "/certificates",
            "/verify/:cert_id"
        ]
    }))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let keyring = state.keyring.read().await;
    
    Json(json!({
        "status": "healthy",
        "key_id": keyring.current_key_id(),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

#[derive(serde::Deserialize)]
struct CreateChallengeRequest {
    system_id: String,
    #[serde(default = "default_timeout")]
    timeout_hours: i64,
}

fn default_timeout() -> i64 {
    72
}

async fn create_challenge(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateChallengeRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let request = generate_proof_request(&req.system_id, req.timeout_hours);
    let request_id = request.request_id.clone();
    
    {
        let mut challenges = state.challenges.write().await;
        challenges.insert(request_id.clone(), request.clone());
    }
    
    info!("Created challenge {} for system {}", request_id, req.system_id);
    
    Ok(Json(json!({
        "request_id": request_id,
        "system_id": req.system_id,
        "deadline": request.deadline,
        "requested_proofs": request.requested_proofs,
    })))
}

async fn get_challenge(
    State(state): State<Arc<AppState>>,
    AxumPath(request_id): AxumPath<String>,
) -> Result<Json<ProofRequest>, StatusCode> {
    let challenges = state.challenges.read().await;
    
    match challenges.get(&request_id) {
        Some(req) => Ok(Json(req.clone())),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn submit_response(
    State(state): State<Arc<AppState>>,
    Json(response): Json<SystemResponse>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let challenges = state.challenges.read().await;
    
    let request = match challenges.get(&response.request_id) {
        Some(req) => req.clone(),
        None => return Err(StatusCode::NOT_FOUND),
    };
    
    drop(challenges);
    
    let result = evaluate_proofs(&request, Some(response));
    
    match result {
        Ok(VerificationResult::Compliant) => {
            let keyring = state.keyring.read().await;
            let signing_key = keyring.get_active_keypair()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let cert = Certificate::new_compliant(&request.system_id, &signing_key);
            
            {
                let mut certificates = state.certificates.write().await;
                certificates.push(cert.clone());
            }
            
            info!("System {} is compliant", request.system_id);
            
            Ok(Json(json!({
                "status": "compliant",
                "certificate_id": hex::encode(&cert.certificate_id[..8]),
            })))
        }
        Ok(VerificationResult::Violation { missing_proofs, invalid_proofs, reason }) => {
            let keyring = state.keyring.read().await;
            let signing_key = keyring.get_active_keypair()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            let cert = ViolationCertificate::issue(
                &request.system_id,
                vec!["LAW_1".to_string(), "LAW_2".to_string(), "LAW_3".to_string()],
                &reason,
                &signing_key,
            );
            
            info!("System {} violated HSK: {}", request.system_id, reason);
            
            Ok(Json(json!({
                "status": "violation",
                "reason": reason,
                "missing_proofs": missing_proofs,
                "invalid_proofs": invalid_proofs,
                "certificate_id": hex::encode(&cert.certificate_id[..8]),
            })))
        }
        Err(e) => {
            error!("Evaluation error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn list_certificates(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<serde_json::Value>> {
    let certificates = state.certificates.read().await;
    
    let result: Vec<serde_json::Value> = certificates
        .iter()
        .map(|c| json!({
            "certificate_id": hex::encode(&c.certificate_id[..8]),
            "system_id": c.system_id,
            "compliant": c.hs_compliant,
            "evaluation_time": c.evaluation_time,
        }))
        .collect();
    
    Json(result)
}

async fn get_certificate(
    State(state): State<Arc<AppState>>,
    AxumPath(cert_id): AxumPath<String>,
) -> Result<Json<Certificate>, StatusCode> {
    let certificates = state.certificates.read().await;
    
    for cert in certificates.iter() {
        let short_id = hex::encode(&cert.certificate_id[..8]);
        if short_id == cert_id {
            return Ok(Json(cert.clone()));
        }
    }
    
    Err(StatusCode::NOT_FOUND)
}

async fn verify_certificate(
    State(state): State<Arc<AppState>>,
    AxumPath(cert_id): AxumPath<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let certificates = state.certificates.read().await;
    
    for cert in certificates.iter() {
        let short_id = hex::encode(&cert.certificate_id[..8]);
        if short_id == cert_id {
            let valid = cert.verify()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            
            return Ok(Json(json!({
                "certificate_id": short_id,
                "valid": valid,
                "system_id": cert.system_id,
                "compliant": cert.hs_compliant,
            })));
        }
    }
    
    Err(StatusCode::NOT_FOUND)
}
