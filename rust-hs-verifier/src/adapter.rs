use crate::{ProofRequest, SystemResponse, VerificationResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[async_trait]
pub trait SystemAdapter: Send + Sync {
    async fn challenge(&self, request: &ProofRequest) -> Result<SystemResponse, AdapterError>;
    async fn health_check(&self) -> Result<SystemHealth, AdapterError>;
    fn system_type(&self) -> &str;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealth {
    pub status: HealthStatus,
    pub version: String,
    pub last_check: chrono::DateTime<chrono::Utc>,
    pub proof_availability: HashMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Timeout")]
    Timeout,
    #[error("Authentication failed")]
    Authentication,
    #[error("System not found")]
    NotFound,
}

pub struct HttpAdapter {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
    system_type: String,
}

impl HttpAdapter {
    pub fn new(base_url: String, api_key: Option<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap(),
            base_url,
            api_key,
            system_type: "http".to_string(),
        }
    }
    
    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(key) = &self.api_key {
            headers.insert(
                "Authorization",
                format!("Bearer {}", key).parse().unwrap(),
            );
        }
        headers
    }
}

#[async_trait]
impl SystemAdapter for HttpAdapter {
    async fn challenge(&self, request: &ProofRequest) -> Result<SystemResponse, AdapterError> {
        let response = self.client
            .post(format!("{}/hsk/challenge", self.base_url))
            .headers(self.auth_headers())
            .json(request)
            .send()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;
        
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(AdapterError::NotFound);
        }
        
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(AdapterError::Authentication);
        }
        
        if !response.status().is_success() {
            return Err(AdapterError::InvalidResponse(
                format!("HTTP {}", response.status())
            ));
        }
        
        response.json::<SystemResponse>()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))
    }
    
    async fn health_check(&self) -> Result<SystemHealth, AdapterError> {
        let response = self.client
            .get(format!("{}/health", self.base_url))
            .headers(self.auth_headers())
            .send()
            .await
            .map_err(|e| AdapterError::Connection(e.to_string()))?;
        
        if !response.status().is_success() {
            return Ok(SystemHealth {
                status: HealthStatus::Unhealthy,
                version: "unknown".to_string(),
                last_check: chrono::Utc::now(),
                proof_availability: HashMap::new(),
            });
        }
        
        let health: SystemHealth = response.json()
            .await
            .map_err(|e| AdapterError::InvalidResponse(e.to_string()))?;
        
        Ok(health)
    }
    
    fn system_type(&self) -> &str {
        &self.system_type
    }
}

pub struct GrpcAdapter {
    endpoint: String,
    system_type: String,
}

impl GrpcAdapter {
    pub fn new(endpoint: String) -> Self {
        Self {
            endpoint,
            system_type: "grpc".to_string(),
        }
    }
}

#[async_trait]
impl SystemAdapter for GrpcAdapter {
    async fn challenge(&self, _request: &ProofRequest) -> Result<SystemResponse, AdapterError> {
        Err(AdapterError::Connection("gRPC not yet implemented".to_string()))
    }
    
    async fn health_check(&self) -> Result<SystemHealth, AdapterError> {
        Ok(SystemHealth {
            status: HealthStatus::Degraded,
            version: "unknown".to_string(),
            last_check: chrono::Utc::now(),
            proof_availability: HashMap::new(),
        })
    }
    
    fn system_type(&self) -> &str {
        &self.system_type
    }
}

pub struct AdapterRegistry {
    adapters: HashMap<String, Box<dyn SystemAdapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, system_id: String, adapter: Box<dyn SystemAdapter>) {
        self.adapters.insert(system_id, adapter);
    }
    
    pub fn get(&self, system_id: &str) -> Option<&Box<dyn SystemAdapter>> {
        self.adapters.get(system_id)
    }
    
    pub async fn challenge_all(
        &self,
        request: &ProofRequest,
    ) -> Vec<(String, Result<SystemResponse, AdapterError>)> {
        let mut results = Vec::new();
        
        for (system_id, adapter) in &self.adapters {
            let result = adapter.challenge(request).await;
            results.push((system_id.clone(), result));
        }
        
        results
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_registry() {
        let mut registry = AdapterRegistry::new();
        
        let adapter = HttpAdapter::new("http://localhost:8080".to_string(), None);
        registry.register("test-system".to_string(), Box::new(adapter));
        
        assert!(registry.get("test-system").is_some());
        assert!(registry.get("nonexistent").is_none());
    }
}
