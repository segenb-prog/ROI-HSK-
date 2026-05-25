# RI-0 Platform Build Guide

Complete step-by-step guide to building and deploying the RI-0 Human Sovereignty Kernel platform.

---

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Project Structure](#project-structure)
3. [Build Instructions](#build-instructions)
4. [Local Development](#local-development)
5. [Production Deployment](#production-deployment)
6. [Verification & Testing](#verification--testing)

---

## Prerequisites

### Required Tools

```bash
# Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup update
rustup default 1.75.0

# Docker & Docker Compose
# https://docs.docker.com/get-docker/

# Kubernetes tools
# https://kubernetes.io/docs/tasks/tools/

# PostgreSQL client (optional, for debugging)
apt-get install postgresql-client  # Debian/Ubuntu
brew install libpq                 # macOS
```

### Environment Variables

Create a `.env` file:

```bash
# Database
DATABASE_URL=postgresql://consent:consent_password@localhost:5432/consent_ledger

# HSK Verifier
HSK_KEYRING_PATH=./keyring.json
HSK_SYSTEM_PUBLIC_KEY=./system.pub
HSK_EMPTY_STATE_ROOT=0000000000000000000000000000000000000000000000000000000000000000
HSK_ALLOWED_ISSUERS=hskernel.gov

# Logging
RUST_LOG=info
```

---

## Project Structure

```
ri0-platform/
├── rust-hs-verifier/              # Component 1: HSK Falsification Machine
│   ├── src/                       # Complete Rust implementation
│   │   ├── main.rs                # CLI with 8 commands
│   │   ├── verifiers.rs           # 4 cryptographic verifiers
│   │   ├── certificate.rs         # Violation certificate issuance
│   │   ├── issuer.rs              # Key management with rotation
│   │   ├── transparency.rs        # Log integration
│   │   └── server.rs              # HTTP API server
│   └── Cargo.toml
│
├── k8s-deployments/               # Component 2: Kubernetes infrastructure
│   ├── namespace.yaml             # hsk-verifier namespace
│   ├── verifier-deployment.yaml   # 3-replica verifier service
│   ├── transparency-log-deployment.yaml  # StatefulSet for logs
│   ├── database.yaml              # PostgreSQL with init scripts
│   ├── configmaps.yaml            # Configuration
│   └── monitoring.yaml            # Prometheus + Grafana
│
├── database-schemas/              # Component 3: Database schemas
│   ├── consent_ledger.sql         # Full schema with functions
│   └── transparency_log.sql       # Merkle tree + audit tables
│
└── prototype-digital-identity/    # Component 4: Working prototype
    ├── src/
    │   ├── main.rs                # Axum server with all endpoints
    │   ├── models.rs              # Citizen + ConsentEntry types
    │   ├── consent.rs             # Consent logic
    │   └── verification.rs        # HSK proof generation
    ├── Dockerfile
    └── docker-compose.yml         # Full stack with monitoring
```

---

## Build Instructions

### Step 1: Build HSK Verifier CLI

```bash
cd rust-hs-verifier

# Build debug version
cargo build

# Build optimized release
cargo build --release

# The binary is at:
# ./target/release/hs-verifier
```

**What you get:**
- `hs-verifier` CLI tool with 8 commands
- 4 cryptographic verifiers (Consent, Passport, Deletion, Prediction)
- HTTP server mode (`hs-verifier server`)
- Full key management with rotation

### Step 2: Build Digital Identity Prototype

```bash
cd prototype-digital-identity

# Build
cargo build --release

# The binary is at:
# ./target/release/did-server
```

**What you get:**
- REST API server for Digital Identity + Consent Ledger
- PostgreSQL integration with sqlx
- Hash chain verification
- HSK proof generation

### Step 3: Build Docker Images

```bash
# Build HSK Verifier image
cd rust-hs-verifier
docker build -t hskernel/hs-verifier:0.1.0 .

# Build Digital Identity image
cd ../prototype-digital-identity
docker build -t hskernel/did-server:0.1.0 .
```

---

## Local Development

### Option 1: Docker Compose (Recommended)

```bash
cd prototype-digital-identity

# Start all services
docker-compose up -d

# Services will be available at:
# - Digital Identity API: http://localhost:8080
# - HSK Verifier: http://localhost:8081
# - Transparency Log: http://localhost:8082
# - PostgreSQL: localhost:5432
# - Grafana: http://localhost:3000 (admin/admin)
# - Prometheus: http://localhost:9091

# View logs
docker-compose logs -f did-server

# Stop everything
docker-compose down
```

### Option 2: Native Development

```bash
# Terminal 1: Start PostgreSQL
docker run -d \
  --name postgres \
  -e POSTGRES_USER=consent \
  -e POSTGRES_PASSWORD=consent_password \
  -e POSTGRES_DB=consent_ledger \
  -p 5432:5432 \
  postgres:15-alpine

# Apply schemas
psql -h localhost -U consent -d consent_ledger -f database-schemas/consent_ledger.sql
psql -h localhost -U consent -d consent_ledger -f database-schemas/transparency_log.sql

# Terminal 2: Start Digital Identity Server
cd prototype-digital-identity
cargo run

# Terminal 3: Start HSK Verifier Server
cd rust-hs-verifier

# Generate keys first
cargo run -- generate-keys --output keyring.json

# Start server
cargo run -- server --port 8081 --keyring keyring.json
```

### Testing the APIs

```bash
# 1. Register a citizen
curl -X POST http://localhost:8080/citizens \
  -H "Content-Type: application/json" \
  -d '{
    "did": "did:hsk:citizen:123",
    "public_key": "BASE64_ENCODED_ED25519_PUBLIC_KEY"
  }'

# 2. Grant consent
curl -X POST http://localhost:8080/consent/grant \
  -H "Content-Type: application/json" \
  -d '{
    "citizen_did": "did:hsk:citizen:123",
    "scope": ["health_data", "location"],
    "purpose": "medical_research",
    "duration_seconds": 2592000,
    "citizen_signature": "BASE64_SIGNATURE"
  }'

# 3. Verify hash chain
curl http://localhost:8080/verify/chain/did:hsk:citizen:123

# 4. Get HSK proofs
curl http://localhost:8080/hsk/proofs/did:hsk:citizen:123

# 5. Challenge a system (using HSK verifier)
curl -X POST http://localhost:8081/challenge \
  -H "Content-Type: application/json" \
  -d '{
    "system_id": "my-ai-system",
    "timeout_hours": 72
  }'
```

---

## Production Deployment

### Step 1: Prepare Kubernetes Cluster

```bash
# Verify cluster access
kubectl cluster-info

# Create namespace
kubectl apply -f k8s-deployments/namespace.yaml

# Set context
kubectl config set-context --current --namespace=hsk-verifier
```

### Step 2: Configure Secrets

```bash
# IMPORTANT: Generate real keys in an air-gapped environment!

# Generate system keypair
cd rust-hs-verifier
cargo run -- generate-keys --output /tmp/keyring.json --offline

# Extract public key
# (You'll need to implement this or use openssl)

# Create Kubernetes secret
kubectl create secret generic hsk-keyring \
  --from-file=keyring.json=/tmp/keyring.json \
  --from-file=system.pub=/tmp/system.pub \
  -n hsk-verifier

# Create database credentials
kubectl create secret generic transparency-db-credentials \
  --from-literal=username=transparency \
  --from-literal=password='STRONG_PASSWORD_HERE' \
  -n hsk-verifier

kubectl create secret generic transparency-log-db \
  --from-literal=url='postgresql://transparency:STRONG_PASSWORD@transparency-db:5432/transparency' \
  -n hsk-verifier
```

### Step 3: Deploy Infrastructure

```bash
cd k8s-deployments

# Apply in order
kubectl apply -f configmaps.yaml
kubectl apply -f database.yaml

# Wait for database to be ready
kubectl wait --for=condition=ready pod -l app=transparency-db --timeout=120s

# Deploy verifier and transparency logs
kubectl apply -f verifier-deployment.yaml
kubectl apply -f transparency-log-deployment.yaml

# Deploy monitoring
kubectl apply -f monitoring.yaml
```

### Step 4: Verify Deployment

```bash
# Check all pods are running
kubectl get pods -n hsk-verifier

# Expected output:
# NAME                               READY   STATUS
did-server-xxx                      1/1     Running
hs-verifier-xxx                     1/1     Running
transparency-db-0                   1/1     Running
transparency-log-0                  1/1     Running
transparency-log-1                  1/1     Running
transparency-log-2                  1/1     Running

# Check services
kubectl get svc -n hsk-verifier

# Test verifier endpoint
kubectl port-forward svc/hs-verifier 8080:8080 -n hsk-verifier &
curl http://localhost:8080/health
```

### Step 5: Configure Ingress (Production)

```bash
# Update ingress with your domain
kubectl apply -f - <<EOF
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: hs-verifier
  namespace: hsk-verifier
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: "letsencrypt-prod"
spec:
  tls:
    - hosts:
        - verifier.YOURDOMAIN.com
      secretName: hs-verifier-tls
  rules:
    - host: verifier.YOURDOMAIN.com
      http:
        paths:
          - path: /
            pathType: Prefix
            backend:
              service:
                name: hs-verifier
                port:
                  number: 8080
EOF
```

---

## Verification & Testing

### Run Unit Tests

```bash
# HSK Verifier tests
cd rust-hs-verifier
cargo test

# Digital Identity tests
cd ../prototype-digital-identity
cargo test
```

### Run Integration Tests

```bash
# Start services
cd prototype-digital-identity
docker-compose up -d

# Wait for initialization
sleep 15

# Run integration tests
cargo test --features integration

# Or test manually
./scripts/integration-test.sh
```

### Security Audit

```bash
# Run cargo audit
cargo install cargo-audit
cargo audit

# Run clippy
cargo clippy --all-targets --all-features -- -D warnings

# Check for outdated dependencies
cargo install cargo-outdated
cargo outdated
```

### Load Testing

```bash
# Install k6
# https://k6.io/docs/getting-started/installation/

# Run load test
k6 run --vus 100 --duration 30s scripts/load-test.js
```

---

## Troubleshooting

### Common Issues

**Issue:** `error: could not find native static library`  
**Solution:** Install PostgreSQL development libraries
```bash
# Ubuntu/Debian
sudo apt-get install libpq-dev

# macOS
brew install libpq
```

**Issue:** `database connection failed`  
**Solution:** Check DATABASE_URL and ensure PostgreSQL is running
```bash
# Test connection
psql $DATABASE_URL -c "SELECT 1"
```

**Issue:** `keyring not found`  
**Solution:** Generate keys first
```bash
cargo run -- generate-keys --output keyring.json
```

**Issue:** Pods stuck in `Pending`  
**Solution:** Check resource limits and storage classes
```bash
kubectl describe pod <pod-name> -n hsk-verifier
kubectl get storageclass
```

---

## Next Steps

1. **Customize for your environment**
   - Update domain names in ingress
   - Configure real SSL certificates
   - Set up proper monitoring alerts

2. **Security hardening**
   - Enable HSM for key storage
   - Implement multi-party key generation
   - Add rate limiting

3. **Scale up**
   - Increase replica counts
   - Add read replicas for database
   - Deploy to multiple regions

4. **Integrate with your AI systems**
   - Implement system adapters
   - Configure proof generation
   - Set up automated monitoring

---

## Support

For issues and questions:
- GitHub Issues: https://github.com/hskernel/hs-verifier/issues
- Documentation: See README.md in each component directory
