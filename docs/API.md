# RI-0 HSK API Contract — v1

Status: pilot-ready / production-hardening in progress.

This repository currently contains two implementation tracks:

1. **Local deterministic demo** (`make demo`) — no external services required.
2. **Service APIs** for the HSK verifier and digital identity prototype — intended to be normalized under `/v1` before external integration.

## Canonical demo flow

The demo proves the following sequence:

```text
create identity → grant consent → generate cryptographic proof → verify proof → revoke consent → show audit trail
```

Run:

```bash
make setup
make demo
```

Run the service locally:

```bash
make api
```

Run the service via Docker Compose:

```bash
make docker-up
make docker-logs
make docker-down
```

Output audit file:

```text
reports/demo_audit.json
```

## Canonical `/v1` resource model

| Area | Method | Path | Purpose |
|---|---:|---|---|
| Health | GET | `/v1/health` | Service health and version status |
| Identity | POST | `/v1/identities` | Create/register an identity with an Ed25519 public key |
| Identity | GET | `/v1/identities/{did}` | Retrieve identity metadata |
| Consent | POST | `/v1/consents` | Grant consent for a DID with scope, purpose, and expiry |
| Consent | POST | `/v1/consents/{entry_id}/revoke` | Revoke a previously granted consent entry |
| Proof | POST | `/v1/proofs` | Generate proof for a ledger entry |
| Proof | POST | `/v1/proofs/verify` | Verify proof signature and ledger root |
| Audit | GET | `/v1/audit` | Retrieve all audit events |
| Audit | GET | `/v1/audit/{did}` | Retrieve audit trail for a specific DID |

## Current implementation note

The local MVP service is implemented in `services/hsk_api/` using FastAPI and SQLite for persistence. Ed25519 signing is performed through the Python `cryptography` package. Local key storage is MVP-only; production deployments require secure, managed key storage and hardened secret management. Docker Compose support is included for local deployment proof.

The Rust `hs-verifier` service currently exposes challenge/certificate endpoints without a `/v1` prefix. The digital identity prototype currently exposes citizen/consent endpoints directly. These are retained for backward compatibility, but the external contract above is the target contract for SDKs, docs, and partner integration.

## Security notes

- No production secret should be committed to the repository.
- `.env.example` contains only local placeholder values.
- Production deployments must enforce authentication, rate limiting, TLS/mTLS, audit persistence, and secret management through Vault or the cloud secret manager.
