use axum::{
    routing::{get, post},
    Router,
    Json,
    extract::{State, Path as AxumPath},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::sync::Arc;
use tracing::{info, error, warn};
use chrono::{DateTime, Utc, Duration};
use ed25519_dalek::{Keypair, Signer, PublicKey, Signature};
use sha2::{Sha256, Digest};

mod models;
mod consent;
mod verification;

use models::*;
use consent::*;
use verification::*;

pub struct AppState {
    db: PgPool,
    system_keypair: Keypair,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting Digital Identity + Consent Ledger Prototype");

    // Database connection
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/consent_ledger".to_string());
    
    let db = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await?;

    info!("Connected to database");

    // Generate or load system keypair
    let system_keypair = load_or_generate_keypair().await?;
    info!("System public key: {}", hex::encode(&system_keypair.public.to_bytes()[..8]));

    let state = Arc::new(AppState {
        db,
        system_keypair,
    });

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        // Citizen endpoints
        .route("/citizens", post(register_citizen))
        .route("/citizens/:did", get(get_citizen))
        .route("/citizens/:did/consents", get(get_citizen_consents))
        // Consent endpoints
        .route("/consent/grant", post(grant_consent))
        .route("/consent/revoke", post(revoke_consent))
        .route("/consent/verify/:entry_id", get(verify_consent_entry))
        // Verification endpoints
        .route("/verify/chain/:did", get(verify_citizen_chain))
        .route("/verify/access", post(verify_access))
        // HSK integration
        .route("/hsk/proofs/:did", get(get_hsk_proofs))
        .with_state(state);

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let addr = format!("0.0.0.0:{}", port);
    info!("Server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn load_or_generate_keypair() -> Result<Keypair, Box<dyn std::error::Error>> {
    use rand::rngs::OsRng;
    
    let key_path = std::env::var("SYSTEM_KEY_PATH")
        .unwrap_or_else(|_| "system_key.bin".to_string());
    
    if std::path::Path::new(&key_path).exists() {
        let key_data = std::fs::read(&key_path)?;
        let secret = ed25519_dalek::SecretKey::from_bytes(&key_data)?;
        let public = ed25519_dalek::PublicKey::from(&secret);
        Ok(Keypair { secret, public })
    } else {
        let mut csprng = OsRng;
        let keypair = Keypair::generate(&mut csprng);
        std::fs::write(&key_path, keypair.secret.to_bytes())?;
        info!("Generated new system keypair at {}", key_path);
        Ok(keypair)
    }
}

async fn root() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "Digital Identity + Consent Ledger Prototype",
        "version": "0.1.0",
        "endpoints": {
            "citizens": {
                "POST /citizens": "Register a new citizen",
                "GET /citizens/:did": "Get citizen info",
                "GET /citizens/:did/consents": "Get citizen's consent history"
            },
            "consent": {
                "POST /consent/grant": "Grant consent",
                "POST /consent/revoke": "Revoke consent",
                "GET /consent/verify/:entry_id": "Verify a consent entry"
            },
            "verification": {
                "GET /verify/chain/:did": "Verify citizen's hash chain",
                "POST /verify/access": "Check if access is consented"
            },
            "hsk": {
                "GET /hsk/proofs/:did": "Get HSK proofs for citizen"
            }
        }
    }))
}

async fn health_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let db_status = match sqlx::query("SELECT 1").fetch_one(&state.db).await {
        Ok(_) => "healthy",
        Err(_) => "unhealthy",
    };

    Json(serde_json::json!({
        "status": "healthy",
        "database": db_status,
        "timestamp": Utc::now().to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct RegisterCitizenRequest {
    did: String,
    public_key: String, // Base64 encoded Ed25519 public key
}

async fn register_citizen(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterCitizenRequest>,
) -> Result<Json<CitizenResponse>, StatusCode> {
    info!("Registering citizen: {}", req.did);

    let public_key = match base64::decode(&req.public_key) {
        Ok(pk) => pk,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if public_key.len() != 32 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let result = sqlx::query_as::<_, Citizen>(
        "INSERT INTO consent_ledger.citizens (did, public_key) VALUES ($1, $2) RETURNING *"
    )
    .bind(&req.did)
    .bind(&public_key)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(citizen) => {
            info!("Citizen registered: {}", req.did);
            Ok(Json(CitizenResponse::from(citizen)))
        }
        Err(e) => {
            error!("Failed to register citizen: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn get_citizen(
    State(state): State<Arc<AppState>>,
    AxumPath(did): AxumPath<String>,
) -> Result<Json<CitizenResponse>, StatusCode> {
    let result = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE did = $1"
    )
    .bind(&did)
    .fetch_one(&state.db)
    .await;

    match result {
        Ok(citizen) => Ok(Json(CitizenResponse::from(citizen))),
        Err(sqlx::Error::RowNotFound) => Err(StatusCode::NOT_FOUND),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}

async fn get_citizen_consents(
    State(state): State<Arc<AppState>>,
    AxumPath(did): AxumPath<String>,
) -> Result<Json<Vec<ConsentEntryResponse>>, StatusCode> {
    let entries = sqlx::query_as::<_, ConsentEntry>(
        r#"
        SELECT e.* FROM consent_ledger.entries e
        JOIN consent_ledger.citizens c ON e.citizen_id = c.id
        WHERE c.did = $1
        ORDER BY e.created_at ASC
        "#
    )
    .bind(&did)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses: Vec<ConsentEntryResponse> = entries
        .into_iter()
        .map(ConsentEntryResponse::from)
        .collect();

    Ok(Json(responses))
}

#[derive(Deserialize)]
struct GrantConsentRequest {
    citizen_did: String,
    scope: Vec<String>,
    purpose: String,
    duration_seconds: u64,
    constraints: Option<serde_json::Value>,
    citizen_signature: String, // Base64 signature over entry hash
}

async fn grant_consent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<GrantConsentRequest>,
) -> Result<Json<ConsentEntryResponse>, StatusCode> {
    info!("Granting consent for: {}", req.citizen_did);

    // Get citizen
    let citizen = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE did = $1"
    )
    .bind(&req.citizen_did)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get previous entry hash
    let previous_entry: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT entry_id FROM consent_ledger.entries 
        WHERE citizen_id = $1 
        ORDER BY created_at DESC 
        LIMIT 1
        "#
    )
    .bind(citizen.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let previous_entry_id = previous_entry.map(|(id,)| id)
        .unwrap_or_else(|| "0".repeat(64));

    // Compute entry hash
    let granted_at = Utc::now();
    let expires_at = granted_at + Duration::seconds(req.duration_seconds as i64);

    let entry_hash = compute_entry_hash(
        &hex::decode(&previous_entry_id).unwrap_or_else(|_| vec![0; 32]),
        &granted_at,
        "grant",
        &req.scope,
        &req.purpose,
        req.duration_seconds,
        &req.constraints,
        &citizen.public_key,
    );

    let entry_id = hex::encode(entry_hash);

    // Verify citizen signature
    let citizen_sig = match base64::decode(&req.citizen_signature) {
        Ok(sig) => sig,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let citizen_pk = match PublicKey::from_bytes(&citizen.public_key) {
        Ok(pk) => pk,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let signature = match Signature::from_bytes(&citizen_sig) {
        Ok(sig) => sig,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if citizen_pk.verify(entry_id.as_bytes(), &signature).is_err() {
        warn!("Invalid citizen signature for consent grant");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // System attestation signature
    let system_signature = state.system_keypair.sign(entry_id.as_bytes());

    // Insert entry
    let entry = sqlx::query_as::<_, ConsentEntry>(
        r#"
        INSERT INTO consent_ledger.entries (
            entry_id, citizen_id, action, scope, purpose, duration_seconds,
            granted_at, expires_at, constraints, previous_entry_id,
            public_key, signature, system_signature
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING *
        "#
    )
    .bind(&entry_id)
    .bind(citizen.id)
    .bind("grant")
    .bind(serde_json::json!(req.scope))
    .bind(&req.purpose)
    .bind(req.duration_seconds as i64)
    .bind(granted_at)
    .bind(expires_at)
    .bind(req.constraints)
    .bind(&previous_entry_id)
    .bind(&citizen.public_key)
    .bind(&citizen_sig)
    .bind(system_signature.to_bytes().to_vec())
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        error!("Failed to insert consent entry: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    info!("Consent granted: {}", entry_id);

    Ok(Json(ConsentEntryResponse::from(entry)))
}

#[derive(Deserialize)]
struct RevokeConsentRequest {
    citizen_did: String,
    entry_id_to_revoke: String,
    citizen_signature: String,
}

async fn revoke_consent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RevokeConsentRequest>,
) -> Result<Json<ConsentEntryResponse>, StatusCode> {
    info!("Revoking consent for: {}", req.citizen_did);

    // Get citizen
    let citizen = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE did = $1"
    )
    .bind(&req.citizen_did)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get previous entry hash
    let previous_entry: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT entry_id FROM consent_ledger.entries 
        WHERE citizen_id = $1 
        ORDER BY created_at DESC 
        LIMIT 1
        "#
    )
    .bind(citizen.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let previous_entry_id = previous_entry.map(|(id,)| id)
        .unwrap_or_else(|| "0".repeat(64));

    // Compute entry hash for revocation
    let revoked_at = Utc::now();
    let entry_hash = compute_entry_hash(
        &hex::decode(&previous_entry_id).unwrap_or_else(|_| vec![0; 32]),
        &revoked_at,
        "revoke",
        &[],
        &format!("Revocation of {}", req.entry_id_to_revoke),
        0,
        &None::<serde_json::Value>,
        &citizen.public_key,
    );

    let entry_id = hex::encode(entry_hash);

    // Verify citizen signature
    let citizen_sig = match base64::decode(&req.citizen_signature) {
        Ok(sig) => sig,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let citizen_pk = match PublicKey::from_bytes(&citizen.public_key) {
        Ok(pk) => pk,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    let signature = match Signature::from_bytes(&citizen_sig) {
        Ok(sig) => sig,
        Err(_) => return Err(StatusCode::BAD_REQUEST),
    };

    if citizen_pk.verify(entry_id.as_bytes(), &signature).is_err() {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // System attestation
    let system_signature = state.system_keypair.sign(entry_id.as_bytes());

    // Insert revocation entry
    let entry = sqlx::query_as::<_, ConsentEntry>(
        r#"
        INSERT INTO consent_ledger.entries (
            entry_id, citizen_id, action, scope, purpose, duration_seconds,
            granted_at, expires_at, constraints, previous_entry_id,
            public_key, signature, system_signature
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING *
        "#
    )
    .bind(&entry_id)
    .bind(citizen.id)
    .bind("revoke")
    .bind(serde_json::json!([] as Vec<String>))
    .bind(format!("Revocation of {}", req.entry_id_to_revoke))
    .bind(0i64)
    .bind(revoked_at)
    .bind(revoked_at)
    .bind(serde_json::json!({}))
    .bind(&previous_entry_id)
    .bind(&citizen.public_key)
    .bind(&citizen_sig)
    .bind(system_signature.to_bytes().to_vec())
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    info!("Consent revoked: {}", entry_id);

    Ok(Json(ConsentEntryResponse::from(entry)))
}

async fn verify_consent_entry(
    State(state): State<Arc<AppState>>,
    AxumPath(entry_id): AxumPath<String>,
) -> Result<Json<VerificationResult>, StatusCode> {
    let entry = sqlx::query_as::<_, ConsentEntry>(
        "SELECT * FROM consent_ledger.entries WHERE entry_id = $1"
    )
    .bind(&entry_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get citizen's public key
    let citizen = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE id = $1"
    )
    .bind(entry.citizen_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Verify signature
    let signature = match Signature::from_bytes(&entry.signature) {
        Ok(sig) => sig,
        Err(_) => {
            return Ok(Json(VerificationResult {
                valid: false,
                entry_id,
                error: Some("Invalid signature format".to_string()),
                chain_valid: false,
            }));
        }
    };

    let public_key = match PublicKey::from_bytes(&citizen.public_key) {
        Ok(pk) => pk,
        Err(_) => {
            return Ok(Json(VerificationResult {
                valid: false,
                entry_id,
                error: Some("Invalid public key".to_string()),
                chain_valid: false,
            }));
        }
    };

    let valid = public_key.verify(entry.entry_id.as_bytes(), &signature).is_ok();

    Ok(Json(VerificationResult {
        valid,
        entry_id,
        error: if valid { None } else { Some("Signature verification failed".to_string()) },
        chain_valid: false, // Would need to verify full chain
    }))
}

async fn verify_citizen_chain(
    State(state): State<Arc<AppState>>,
    AxumPath(did): AxumPath<String>,
) -> Result<Json<ChainVerificationResult>, StatusCode> {
    let citizen = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE did = $1"
    )
    .bind(&did)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let entries = sqlx::query_as::<_, ConsentEntry>(
        "SELECT * FROM consent_ledger.entries WHERE citizen_id = $1 ORDER BY created_at ASC"
    )
    .bind(citizen.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut valid = true;
    let mut invalid_entries = Vec::new();
    let mut previous_hash = vec![0u8; 32];

    for entry in &entries {
        let computed_hash = compute_entry_hash(
            &previous_hash,
            &entry.granted_at,
            &entry.action,
            &entry.scope,
            &entry.purpose,
            entry.duration_seconds as u64,
            &entry.constraints,
            &entry.public_key,
        );

        if hex::encode(&computed_hash) != entry.entry_id {
            valid = false;
            invalid_entries.push(entry.entry_id.clone());
        }

        previous_hash = computed_hash.to_vec();
    }

    Ok(Json(ChainVerificationResult {
        citizen_did: did,
        valid,
        entry_count: entries.len(),
        invalid_entries: if invalid_entries.is_empty() { None } else { Some(invalid_entries) },
    }))
}

#[derive(Deserialize)]
struct VerifyAccessRequest {
    citizen_did: String,
    resource: String,
    purpose: String,
}

async fn verify_access(
    State(state): State<Arc<AppState>>,
    Json(req): Json<VerifyAccessRequest>,
) -> Result<Json<AccessCheckResult>, StatusCode> {
    // Query using the database function
    let has_consent: Option<(bool,)> = sqlx::query_as(
        "SELECT consent_ledger.is_access_consented($1, $2, $3)"
    )
    .bind(&req.citizen_did)
    .bind(&req.resource)
    .bind(&req.purpose)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let consented = has_consent.map(|(b,)| b).unwrap_or(false);

    Ok(Json(AccessCheckResult {
        citizen_did: req.citizen_did,
        resource: req.resource,
        purpose: req.purpose,
        consented,
    }))
}

async fn get_hsk_proofs(
    State(state): State<Arc<AppState>>,
    AxumPath(did): AxumPath<String>,
) -> Result<Json<HSKProofs>, StatusCode> {
    let citizen = sqlx::query_as::<_, Citizen>(
        "SELECT * FROM consent_ledger.citizens WHERE did = $1"
    )
    .bind(&did)
    .fetch_one(&state.db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let entries = sqlx::query_as::<_, ConsentEntry>(
        "SELECT * FROM consent_ledger.entries WHERE citizen_id = $1 ORDER BY created_at ASC"
    )
    .bind(citizen.id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get hash chain state
    let chain_state: Option<(String, i64)> = sqlx::query_as(
        "SELECT latest_entry_id, entry_count FROM consent_ledger.hash_chain WHERE citizen_id = $1"
    )
    .bind(citizen.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let entry_responses: Vec<ConsentEntryResponse> = entries
        .into_iter()
        .map(ConsentEntryResponse::from)
        .collect();

    Ok(Json(HSKProofs {
        citizen_did: did,
        public_key: base64::encode(&citizen.public_key),
        entry_count: entry_responses.len(),
        latest_entry_id: chain_state.as_ref().map(|(id, _)| id.clone()),
        entries: entry_responses,
        proof_type: "ConsentLedger".to_string(),
    }))
}

fn compute_entry_hash(
    previous_hash: &[u8],
    timestamp: &DateTime<Utc>,
    action: &str,
    scope: &[String],
    purpose: &str,
    duration_seconds: u64,
    constraints: &Option<impl Serialize>,
    public_key: &[u8],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    
    hasher.update(previous_hash);
    hasher.update(timestamp.to_rfc3339().as_bytes());
    hasher.update(action.as_bytes());
    hasher.update(serde_json::to_string(scope).unwrap().as_bytes());
    hasher.update(purpose.as_bytes());
    hasher.update(&duration_seconds.to_be_bytes());
    
    if let Some(c) = constraints {
        hasher.update(serde_json::to_string(c).unwrap().as_bytes());
    }
    
    hasher.update(public_key);
    
    hasher.finalize().into()
}
