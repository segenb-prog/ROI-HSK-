# RI-0 Platform Architecture

## Complete System Architecture

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              USER INTERFACE LAYER                                │
│                                                                                  │
│   ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────────┐              │
│   │ Citizens │  │Enterprises│  │Government│  │International Partners│              │
│   └────┬─────┘  └────┬─────┘  └────┬─────┘  └────────┬─────────┘              │
│        │             │             │                  │                          │
│        └─────────────┴─────────────┴──────────────────┘                          │
│                          │                                                       │
└──────────────────────────┼───────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         DIGITAL IDENTITY + CONSENT LEDGER                        │
│                                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────┐  │
│   │                         DID Server (Axum)                                │  │
│   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │  │
│   │  │   POST      │  │    POST     │  │    GET      │  │    GET      │   │  │
│   │  │  /citizens  │  │/consent/grant│  │/verify/chain│  │/hsk/proofs  │   │  │
│   │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘   │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                             │
│                                    ▼                                             │
│   ┌─────────────────────────────────────────────────────────────────────────┐  │
│   │                    PostgreSQL (consent_ledger schema)                    │  │
│   │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌─────────────┐ │  │
│   │  │   citizens   │  │   entries    │  │  hash_chain  │  │verification_│ │  │
│   │  │   (DIDs)     │  │(hash chain)  │  │   (state)    │  │    log      │ │  │
│   │  └──────────────┘  └──────────────┘  └──────────────┘  └─────────────┘ │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                           │
                           │ HSK Proofs
                           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                    CRYPTOGRAPHIC VERIFICATION LAYER (HSK)                        │
│                                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────┐  │
│   │                     INTEGRATED VERIFIER PIPELINE                         │  │
│   │                                                                          │  │
│   │  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐        │  │
│   │  │ ConsentLedger   │  │ MemoryPassport  │  │  DeletionProof  │        │  │
│   │  │   Verifier      │  │   Verifier      │  │   Verifier      │        │  │
│   │  │                 │  │                 │  │                 │        │  │
│   │  │ • Hash chain    │  │ • JWT EdDSA     │  │ • Merkle tree   │        │  │
│   │  │ • Ed25519 sigs  │  │ • Scope check   │  │ • Empty root    │        │  │
│   │  │ • Monotonic time│  │ • no_derivatives│  │ • 72h window    │        │  │
│   │  └─────────────────┘  └─────────────────┘  └─────────────────┘        │  │
│   │                                                                          │  │
│   │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│   │  │                    PredictionScope Verifier                      │  │  │
│   │  │                                                                  │  │  │
│   │  │  • Model weight hashing  • Training data attestation            │  │  │
│   │  │  • Inference log auditing                                    │  │  │
│   │  └─────────────────────────────────────────────────────────────────┘  │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                           │
                           │ Challenge / Response
                           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         FALSIFICATION MACHINE (HSK Verifier)                     │
│                                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────┐  │
│   │                         Rust CLI + HTTP Server                           │  │
│   │                                                                          │  │
│   │  Commands:                    Endpoints:                                 │  │
│   │  ─────────                    ──────────                                 │  │
│   │  verify <system>              POST /challenge                            │  │
│   │  evaluate <request>           POST /response                             │  │
│   │  verify-cert <cert>           GET  /certificates                         │  │
│   │  generate-keys                GET  /verify/:id                           │  │
│   │  submit <cert>                POST /transparency/submit                  │  │
│   │  query                        GET  /transparency/query                   │  │
│   │  server                                                                    │  │
│   │  monitor                                                                   │  │
│   │                                                                          │  │
│   │  ┌─────────────────────────────────────────────────────────────────┐   │  │
│   │  │                    Certificate Issuance                          │   │  │
│   │  │                                                                  │   │  │
│   │  │  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │   │  │
│   │  │  │  Compliant  │────▶│   Signed    │────▶│   Stored    │       │   │  │
│   │  │  │  (no cert)  │     │  Certificate│     │  in Log     │       │   │  │
│   │  │  └─────────────┘     └─────────────┘     └─────────────┘       │   │  │
│   │  │                                                                  │   │  │
│   │  │  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐       │   │  │
│   │  │  │  Violation  │────▶│  Violation  │────▶│  Published  │       │   │  │
│   │  │  │  Detected   │     │  Certificate│     │  to Logs    │       │   │  │
│   │  │  └─────────────┘     └─────────────┘     └─────────────┘       │   │  │
│   │  └─────────────────────────────────────────────────────────────────┘   │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                           │
                           │ Submit Certificate
                           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         TRANSPARENCY LOG INFRASTRUCTURE                          │
│                                                                                  │
│   ┌─────────────────────────────────────────────────────────────────────────┐  │
│   │                    Multi-Server Gossip Protocol                          │  │
│   │                                                                          │  │
│   │   ┌─────────────┐         ┌─────────────┐         ┌─────────────┐      │  │
│   │   │   Log 1     │◄───────►│   Log 2     │◄───────►│   Log 3     │      │  │
│   │   │  (Primary)  │  Gossip │  (Replica)  │  Gossip │  (Replica)  │      │  │
│   │   │             │         │             │         │             │      │  │
│   │   │ • Merkle    │         │ • Merkle    │         │ • Merkle    │      │  │
│   │   │   tree      │         │   tree      │         │   tree      │      │  │
│   │   │ • Hash chain│         │ • Hash chain│         │ • Hash chain│      │  │
│   │   │ • Inclusion │         │ • Inclusion │         │ • Inclusion │      │  │
│   │   │   proofs    │         │   proofs    │         │   proofs    │      │  │
│   │   └──────┬──────┘         └──────┬──────┘         └──────┬──────┘      │  │
│   │          │                       │                       │              │  │
│   │          └───────────────────────┼───────────────────────┘              │  │
│   │                                  │                                       │  │
│   │                                  ▼                                       │  │
│   │   ┌─────────────────────────────────────────────────────────────────┐  │  │
│   │   │                    PostgreSQL (transparency schema)              │  │  │
│   │   │                                                                  │  │  │
│   │   │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────┐ │  │  │
│   │   │  │ certificates│  │merkle_nodes │  │ signed_tree │  │  audit  │ │  │  │
│   │   │  │             │  │             │  │    heads    │  │   log   │ │  │  │
│   │   │  └─────────────┘  └─────────────┘  └─────────────┘  └─────────┘ │  │  │
│   │   └─────────────────────────────────────────────────────────────────┘  │  │
│   └─────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
                           │
                           │ Verify Certificate
                           ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              PUBLIC VERIFICATION                                 │
│                                                                                  │
│   Anyone can verify:                                                             │
│   • Certificate is in the log (inclusion proof)                                  │
│   • Log hasn't been tampered (consistency proof)                                 │
│   • Signature is valid (Ed25519 verification)                                    │
│   • Chain of trust is intact (hash verification)                                 │
│                                                                                  │
│   Tools:                                                                         │
│   • Web UI: https://hskernel.dev/verify                                          │
│   • CLI: hs-verifier verify-cert <certificate.json>                              │
│   • API: GET /verify/:certificate_id                                             │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Data Flow

### 1. Citizen Grants Consent
```
Citizen → Digital Identity Server → PostgreSQL (consent_ledger.entries)
                                         ↓
                                    Hash chain updated
                                         ↓
                                    HSK proof available
```

### 2. AI System Accesses Data
```
AI System → Check Consent → Digital Identity Server
                ↓
         Verify signature + hash chain
                ↓
         Access granted/denied
                ↓
         Memory Passport issued (JWT)
```

### 3. HSK Challenge
```
Verifier → Challenge AI System
              ↓
         System collects proofs
              ↓
         System responds before deadline
              ↓
         Integrated Verifier validates
              ↓
         Compliant OR Violation Certificate
              ↓
         Certificate submitted to Transparency Log
              ↓
         Log servers gossip to sync
```

### 4. Public Verification
```
Anyone → Query Transparency Log
            ↓
         Get inclusion proof
            ↓
         Verify Merkle path
            ↓
         Confirm certificate is authentic
```

---

## Security Properties

| Threat | Mitigation |
|--------|------------|
| Forged consent | Ed25519 signatures + hash chain |
| Backdated entries | Monotonic timestamp verification |
| Fake deletion | Merkle proofs + empty state root |
| Scope creep | JWT with explicit `no_derivatives` |
| Tampered logs | Multi-server gossip + Merkle trees |
| Compromised issuer | Key rotation + transparency |
| Replay attacks | Nonces in challenges |
| Split-view attacks | Multi-server verification |

---

## Deployment Architecture (Kubernetes)

```
┌─────────────────────────────────────────────────────────────────┐
│                         Ingress (nginx)                          │
│              TLS termination, rate limiting                      │
└─────────────────────────────┬───────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        │                     │                     │
        ▼                     ▼                     ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│  hs-verifier  │    │transparency-log│    │  grafana      │
│  (3 replicas) │    │  (3 replicas)  │    │  (1 pod)      │
│               │    │                │    │               │
│ • Challenge   │    │ • Merkle tree  │    │ • Dashboards  │
│ • Evaluate    │    │ • Gossip sync  │    │ • Alerts      │
│ • Certificates│    │ • Inclusion    │    │               │
└───────┬───────┘    └───────┬───────┘    └───────────────┘
        │                    │
        └────────────────────┘
                   │
                   ▼
        ┌───────────────────┐
        │  PostgreSQL       │
        │  (StatefulSet)    │
        │                   │
        │ • consent_ledger  │
        │ • transparency    │
        └───────────────────┘
```

---

## Technology Stack

| Layer | Technology |
|-------|------------|
| Language | Rust 1.75 |
| Web Framework | Axum |
| Database | PostgreSQL 15 |
| Cryptography | Ed25519, SHA-256 |
| Container | Docker |
| Orchestration | Kubernetes |
| Monitoring | Prometheus + Grafana |
| CI/CD | GitHub Actions |

---

## File Organization

```
ri0-platform/
├── rust-hs-verifier/          # Core Rust implementation
├── prototype-digital-identity/ # Working prototype
├── k8s-deployments/           # Kubernetes manifests
├── database-schemas/          # PostgreSQL schemas
├── .github/workflows/         # CI/CD pipelines
├── scripts/                   # Operational scripts
├── tests/                     # Load tests
├── examples/                  # Client SDKs
├── docs/                      # Documentation
└── monitoring/                # Alerting config
```

---

**Status: PRODUCTION READY** ✅
