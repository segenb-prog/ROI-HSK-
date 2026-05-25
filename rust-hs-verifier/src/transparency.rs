use crate::certificate::Certificate;
use serde::{Deserialize, Serialize};
use tracing::{info, warn, error};

#[derive(Debug, Serialize, Deserialize)]
pub struct SubmitResponse {
    pub position: u64,
    pub merkle_root: String,
    pub inclusion_proof: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogEntry {
    pub certificate_id: String,
    pub system_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub compliant: bool,
    pub certificate: Option<Certificate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MonitorEvent {
    pub log_url: String,
    pub entry: LogEntry,
    pub merkle_root: String,
}

pub async fn submit_certificate(
    log_url: &str,
    cert: &Certificate,
) -> Result<SubmitResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let response = client
        .post(format!("{}/submit", log_url))
        .json(cert)
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("Submit failed: {}", response.status()).into());
    }
    
    let submit_response: SubmitResponse = response.json().await?;
    info!(
        "Certificate submitted to {} at position {}",
        log_url, submit_response.position
    );
    
    Ok(submit_response)
}

pub async fn query_log(
    log_url: &str,
    certificate_id: Option<&str>,
    system_id: Option<&str>,
) -> Result<Vec<LogEntry>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let mut url = format!("{}/query", log_url);
    
    if let Some(id) = certificate_id {
        url.push_str(&format!("?certificate_id={}", id));
    }
    
    if let Some(id) = system_id {
        let sep = if certificate_id.is_some() { "&" } else { "?" };
        url.push_str(&format!("{}system_id={}", sep, id));
    }
    
    let response = client.get(&url).send().await?;
    
    if !response.status().is_success() {
        return Err(format!("Query failed: {}", response.status()).into());
    }
    
    let entries: Vec<LogEntry> = response.json().await?;
    Ok(entries)
}

pub async fn verify_inclusion(
    log_url: &str,
    certificate_id: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let response = client
        .get(format!("{}/verify/{}", log_url, certificate_id))
        .send()
        .await?;
    
    if response.status().is_success() {
        let result: serde_json::Value = response.json().await?;
        Ok(result.get("found").and_then(|v| v.as_bool()).unwrap_or(false))
    } else {
        Ok(false)
    }
}

pub async fn get_log_head(log_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let response = client
        .get(format!("{}/head", log_url))
        .send()
        .await?;
    
    if !response.status().is_success() {
        return Err(format!("Failed to get head: {}", response.status()).into());
    }
    
    let head: serde_json::Value = response.json().await?;
    Ok(head.get("merkle_root")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
}

pub async fn monitor_logs(
    log_urls: &[String],
    webhook: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting transparency log monitor for {} logs", log_urls.len());
    
    let mut last_heads: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    
    for log_url in log_urls {
        match get_log_head(log_url).await {
            Ok(head) => {
                last_heads.insert(log_url.clone(), head);
            }
            Err(e) => {
                warn!("Failed to get initial head for {}: {}", log_url, e);
            }
        }
    }
    
    let interval = std::time::Duration::from_secs(60);
    
    loop {
        tokio::time::sleep(interval).await;
        
        for log_url in log_urls {
            match check_for_new_entries(log_url, &last_heads).await {
                Ok(Some((new_head, entries))) => {
                    info!("Detected {} new entries in {}", entries.len(), log_url);
                    
                    for entry in entries {
                        if !entry.compliant {
                            warn!(
                                "New violation detected: {} - {}",
                                entry.system_id, entry.certificate_id
                            );
                            
                            if let Some(webhook_url) = webhook {
                                if let Err(e) = send_webhook(webhook_url, &entry).await {
                                    error!("Failed to send webhook: {}", e);
                                }
                            }
                        }
                    }
                    
                    last_heads.insert(log_url.clone(), new_head);
                }
                Ok(None) => {}
                Err(e) => {
                    error!("Error checking {}: {}", log_url, e);
                }
            }
        }
    }
}

async fn check_for_new_entries(
    log_url: &str,
    last_heads: &std::collections::HashMap<String, String>,
) -> Result<Option<(String, Vec<LogEntry>)>, Box<dyn std::error::Error>> {
    let current_head = get_log_head(log_url).await?;
    
    if let Some(last_head) = last_heads.get(log_url) {
        if current_head == *last_head {
            return Ok(None);
        }
    }
    
    let entries = query_log(log_url, None, None).await?;
    
    Ok(Some((current_head, entries)))
}

async fn send_webhook(
    webhook_url: &str,
    entry: &LogEntry,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    let payload = serde_json::json!({
        "event": "violation_detected",
        "system_id": entry.system_id,
        "certificate_id": entry.certificate_id,
        "timestamp": entry.timestamp,
    });
    
    client.post(webhook_url)
        .json(&payload)
        .send()
        .await?;
    
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GossipMessage {
    pub from: String,
    pub merkle_root: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub signature: String,
}

pub async fn gossip_sync(
    log_urls: &[String],
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting gossip sync between {} log servers", log_urls.len());
    
    let mut roots = Vec::new();
    
    for log_url in log_urls {
        match get_log_head(log_url).await {
            Ok(root) => roots.push((log_url.clone(), root)),
            Err(e) => warn!("Failed to get head from {}: {}", log_url, e),
        }
    }
    
    if roots.len() < 2 {
        return Ok(());
    }
    
    let first_root = &roots[0].1;
    let all_match = roots.iter().all(|(_, r)| r == first_root);
    
    if all_match {
        info!("All log servers in sync (merkle root: {})", &first_root[..16]);
    } else {
        warn!("Log servers out of sync!");
        for (url, root) in &roots {
            warn!("  {}: {}", url, &root[..16]);
        }
    }
    
    Ok(())
}
