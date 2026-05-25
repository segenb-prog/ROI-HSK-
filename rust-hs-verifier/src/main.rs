use clap::{Parser, Subcommand};
use hs_verifier::{challenge, evaluate, certificate, issuer, transparency, server};
use std::path::PathBuf;
use tracing::{info, error};

#[derive(Parser)]
#[command(name = "hs-verifier")]
#[command(about = "HSK Falsification Machine v0.1.0", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Challenge a system and evaluate response
    Verify {
        system_id: String,
        #[arg(default_value_t = 72)]
        timeout_hours: i64,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        endpoint: Option<String>,
    },
    /// Evaluate proofs against a request
    Evaluate {
        request_file: PathBuf,
        response_file: Option<PathBuf>,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Verify a certificate file
    VerifyCert {
        certificate_file: PathBuf,
    },
    /// Generate new issuer keys (offline)
    GenerateKeys {
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        offline: bool,
    },
    /// Show current issuer keyring
    Keys,
    /// Submit certificate to transparency log
    Submit {
        certificate_file: PathBuf,
        #[arg(short, long)]
        log_url: String,
    },
    /// Query transparency log
    Query {
        #[arg(short, long)]
        certificate_id: Option<String>,
        #[arg(short, long)]
        system_id: Option<String>,
        #[arg(short, long)]
        log_url: String,
    },
    /// Start verifier server
    Server {
        #[arg(short, long, default_value = "8080")]
        port: u16,
        #[arg(short, long)]
        keyring: PathBuf,
    },
    /// Monitor transparency logs for new violations
    Monitor {
        #[arg(short, long)]
        log_urls: Vec<String>,
        #[arg(short, long)]
        webhook: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    
    match cli.command {
        Commands::Verify { system_id, timeout_hours, output, endpoint } => {
            info!("Generating proof request for system: {}", system_id);
            
            let request = challenge::generate_proof_request(&system_id, timeout_hours);
            let path = output.unwrap_or_else(|| {
                PathBuf::from(format!("{}.request.json", system_id))
            });
            
            challenge::save_proof_request(&request, &path)?;
            
            println!("🔍 Proof request for {} saved to: {:?}", system_id, path);
            println!("⏰ Deadline: {}", request.deadline);
            
            if let Some(endpoint) = endpoint {
                println!("📡 Sending challenge to: {}", endpoint);
                match challenge::send_challenge(&endpoint, &request).await {
                    Ok(response) => {
                        println!("✅ Challenge accepted by system");
                        if let Some(response_path) = output.map(|p| p.with_extension("response.json")) {
                            std::fs::write(&response_path, serde_json::to_string_pretty(&response)?)?;
                            println!("💾 Response saved to: {:?}", response_path);
                        }
                    }
                    Err(e) => {
                        error!("Failed to send challenge: {}", e);
                        println!("❌ Failed to send challenge: {}", e);
                    }
                }
            } else {
                println!("💡 Use: hs-verifier evaluate {:?} <response.json>", path);
            }
        }
        
        Commands::Evaluate { request_file, response_file, output } => {
            info!("Evaluating proofs from request: {:?}", request_file);
            
            let request_data = std::fs::read_to_string(&request_file)?;
            let request: challenge::ProofRequest = serde_json::from_str(&request_data)?;
            
            let response = if let Some(response_file) = response_file {
                let response_data = std::fs::read_to_string(response_file)?;
                Some(serde_json::from_str(&response_data)?)
            } else {
                None
            };
            
            let result = evaluate::evaluate_proofs(&request, response)?;
            
            match &result {
                evaluate::VerificationResult::Compliant => {
                    println!("✅ System is HSK compliant");
                    println!("📋 All required proofs verified successfully");
                }
                evaluate::VerificationResult::Violation { missing_proofs, invalid_proofs, reason } => {
                    println!("❌ HSK VIOLATION DETECTED");
                    println!("   Missing proofs: {:?}", missing_proofs);
                    println!("   Invalid proofs: {:?}", invalid_proofs);
                    println!("   Reason: {}", reason);
                    
                    let cert = certificate::ViolationCertificate::issue(
                        &request.system_id,
                        vec!["LAW_1".to_string(), "LAW_2".to_string(), "LAW_3".to_string()],
                        reason,
                        &issuer::get_signing_key()?,                    );
                    
                    let cert_file = output.unwrap_or_else(|| {
                        PathBuf::from(format!("{}.violation.json", request.system_id))
                    });
                    
                    std::fs::write(&cert_file, serde_json::to_string_pretty(&cert)?)?;
                    println!("📜 Violation certificate saved to: {:?}", cert_file);
                    println!("   Certificate ID: {}", hex::encode(&cert.certificate_id[..8]));
                }
            }
        }
        
        Commands::VerifyCert { certificate_file } => {
            let cert_data = std::fs::read_to_string(certificate_file)?;
            let cert: certificate::Certificate = serde_json::from_str(&cert_data)?;
            
            match cert.verify() {
                Ok(true) => {
                    println!("✅ Certificate is valid");
                    println!("   System: {}", cert.system_id);
                    println!("   Compliant: {}", cert.hs_compliant);
                    println!("   Violations: {:?}", cert.violations);
                    println!("   Certificate ID: {}", hex::encode(&cert.certificate_id[..8]));
                    println!("   Issued at: {}", cert.evaluation_time);
                }
                Ok(false) => {
                    println!("❌ Certificate signature is invalid");
                }
                Err(e) => {
                    println!("❌ Certificate verification failed: {}", e);
                }
            }
        }
        
        Commands::GenerateKeys { output, offline } => {
            if offline {
                println!("⚠️  DISCONNECT FROM NETWORK BEFORE PROCEEDING");
                println!("   This operation should be performed in an air-gapped environment.");
                println!("   Press Enter to continue...");
                let mut line = String::new();
                std::io::stdin().read_line(&mut line).unwrap();
            }
            
            let keyring = issuer::generate_keyring()?;
            keyring.save(&output)?;
            
            println!("🔑 Keyring generated and saved to: {:?}", output);
            println!("   Public key: {}", hex::encode(&keyring.active_public_key()[..16]));
            println!("   Key ID: {}", keyring.current_key_id());
            println!("   ⚠️  Store the private key securely and never commit it to version control!");
        }
        
        Commands::Keys => {
            match issuer::load_keyring() {
                Ok(keyring) => {
                    println!("🔑 Current Keyring");
                    println!("   Key ID: {}", keyring.current_key_id());
                    println!("   Public key: {}", hex::encode(&keyring.active_public_key()[..16]));
                    println!("   Created: {}", keyring.created_at);
                    println!("   Keys in ring: {}", keyring.keys.len());
                }
                Err(e) => {
                    println!("❌ No keyring found: {}", e);
                    println!("   Generate one with: hs-verifier generate-keys --output keyring.json");
                }
            }
        }
        
        Commands::Submit { certificate_file, log_url } => {
            let cert_data = std::fs::read_to_string(certificate_file)?;
            let cert: certificate::Certificate = serde_json::from_str(&cert_data)?;
            
            match transparency::submit_certificate(&log_url, &cert).await {
                Ok(response) => {
                    println!("✅ Certificate submitted to transparency log");
                    println!("   Position: {}", response.position);
                    println!("   Merkle root: {}", &response.merkle_root[..16]);
                }
                Err(e) => {
                    error!("Failed to submit certificate: {}", e);
                    println!("❌ Failed to submit: {}", e);
                }
            }
        }
        
        Commands::Query { certificate_id, system_id, log_url } => {
            match transparency::query_log(&log_url, certificate_id.as_deref(), system_id.as_deref()).await {
                Ok(entries) => {
                    println!("📜 Found {} entries in transparency log", entries.len());
                    for entry in entries {
                        println!("   [{}] {} - Compliant: {}", 
                            &entry.certificate_id[..8],
                            entry.system_id,
                            entry.compliant
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to query log: {}", e);
                    println!("❌ Failed to query: {}", e);
                }
            }
        }
        
        Commands::Server { port, keyring } => {
            info!("Starting HSK verifier server on port {}", port);
            server::start_server(port, keyring).await?;
        }
        
        Commands::Monitor { log_urls, webhook } => {
            info!("Starting transparency log monitor");
            transparency::monitor_logs(&log_urls, webhook.as_deref()).await?;
        }
    }
    
    Ok(())
}
