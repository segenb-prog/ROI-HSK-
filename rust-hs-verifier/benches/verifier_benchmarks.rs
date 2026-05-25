use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ed25519_dalek::Keypair;
use rand::rngs::OsRng;
use sha2::{Sha256, Digest};

fn bench_hash_computation(c: &mut Criterion) {
    let mut group = c.benchmark_group("hash_computation");
    
    let data_sizes = vec![32, 64, 128, 256, 512, 1024, 4096, 16384];
    
    for size in data_sizes {
        let data = vec![0u8; size];
        
        group.bench_with_input(
            BenchmarkId::new("sha256", size),
            &data,
            |b, data| {
                b.iter(|| {
                    let mut hasher = Sha256::new();
                    hasher.update(black_box(data));
                    hasher.finalize()
                });
            },
        );
    }
    
    group.finish();
}

fn bench_signature_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("signature_operations");
    
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    let message = b"benchmark message";
    let signature = keypair.sign(message);
    
    group.bench_function("sign", |b| {
        b.iter(|| {
            keypair.sign(black_box(message))
        });
    });
    
    group.bench_function("verify", |b| {
        b.iter(|| {
            keypair.public.verify(black_box(message), &signature).unwrap()
        });
    });
    
    group.finish();
}

fn bench_consent_ledger_verification(c: &mut Criterion) {
    use hs_verifier::{
        types::ConsentEntry,
        verifiers::ConsentLedgerVerifier,
    };
    
    let mut group = c.benchmark_group("consent_ledger_verification");
    
    let mut csprng = OsRng;
    let system_keypair = Keypair::generate(&mut csprng);
    
    let verifier = ConsentLedgerVerifier {
        system_public_key: system_keypair.public,
    };
    
    // Create test entries of various sizes
    let entry_counts = vec![1, 10, 100, 1000];
    
    for count in entry_counts {
        let entries: Vec<ConsentEntry> = (0..count)
            .map(|i| ConsentEntry {
                entry_id: format!("entry_{}", i),
                timestamp: chrono::Utc::now().to_rfc3339(),
                action: "grant".to_string(),
                scope: vec!["resource1".to_string()],
                purpose: "test".to_string(),
                duration_seconds: 3600,
                constraints: "{}".to_string(),
                public_key: base64::encode(system_keypair.public.to_bytes()),
                signature: base64::encode(system_keypair.sign(b"test").to_bytes()),
                previous_entry_id: if i == 0 { "0".repeat(64) } else { format!("entry_{}", i - 1) },
            })
            .collect();
        
        let data = serde_json::to_vec(&entries).unwrap();
        
        group.bench_with_input(
            BenchmarkId::new("verify_entries", count),
            &data,
            |b, data| {
                b.iter(|| {
                    let _ = verifier.verify(black_box(data));
                });
            },
        );
    }
    
    group.finish();
}

fn bench_certificate_operations(c: &mut Criterion) {
    use hs_verifier::certificate::{Certificate, ViolationCertificate};
    
    let mut group = c.benchmark_group("certificate_operations");
    
    let mut csprng = OsRng;
    let keypair = Keypair::generate(&mut csprng);
    
    group.bench_function("create_compliant", |b| {
        b.iter(|| {
            Certificate::new_compliant(black_box("test-system"), black_box(&keypair))
        });
    });
    
    group.bench_function("create_violation", |b| {
        b.iter(|| {
            ViolationCertificate::issue(
                black_box("test-system"),
                black_box(vec!["LAW_1".to_string()]),
                black_box("test reason"),
                black_box(&keypair),
            )
        });
    });
    
    let cert = Certificate::new_compliant("test-system", &keypair);
    
    group.bench_function("verify_certificate", |b| {
        b.iter(|| {
            cert.verify().unwrap()
        });
    });
    
    group.finish();
}

fn bench_key_operations(c: &mut Criterion) {
    use hs_verifier::issuer::{generate_keyring, Keyring};
    
    let mut group = c.benchmark_group("key_operations");
    
    group.bench_function("generate_keyring", |b| {
        b.iter(|| {
            generate_keyring().unwrap()
        });
    });
    
    let keyring = generate_keyring().unwrap();
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    keyring.save(temp_file.path()).unwrap();
    
    group.bench_function("load_keyring", |b| {
        b.iter(|| {
            Keyring::load(black_box(temp_file.path())).unwrap()
        });
    });
    
    group.bench_function("get_keypair", |b| {
        b.iter(|| {
            keyring.get_active_keypair().unwrap()
        });
    });
    
    group.finish();
}

fn bench_merkle_operations(c: &mut Criterion) {
    use hs_verifier::types::MerkleProof;
    use sha2::{Sha256, Digest};
    
    let mut group = c.benchmark_group("merkle_operations");
    
    // Benchmark proof verification
    let proof_sizes = vec![1, 2, 4, 8, 16, 32];
    
    for size in proof_sizes {
        let path: Vec<([u8; 32], bool)> = (0..size)
            .map(|i| ([i as u8; 32], i % 2 == 0))
            .collect();
        
        let proof = MerkleProof {
            leaf_hash: [0u8; 32],
            path,
            root_hash: [0u8; 32],
        };
        
        group.bench_with_input(
            BenchmarkId::new("verify_proof", size),
            &proof,
            |b, proof| {
                b.iter(|| {
                    let mut current = proof.leaf_hash;
                    for (sibling, is_left) in &proof.path {
                        let mut hasher = Sha256::new();
                        if *is_left {
                            hasher.update(sibling);
                            hasher.update(&current);
                        } else {
                            hasher.update(&current);
                            hasher.update(sibling);
                        }
                        current = hasher.finalize().into();
                    }
                    current
                });
            },
        );
    }
    
    group.finish();
}

criterion_group!(
    benches,
    bench_hash_computation,
    bench_signature_operations,
    bench_consent_ledger_verification,
    bench_certificate_operations,
    bench_key_operations,
    bench_merkle_operations
);
criterion_main!(benches);
