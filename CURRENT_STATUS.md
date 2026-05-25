# Current Status — RI-0 HSK

## Status Label

**Pilot-ready / production-hardening in progress.**

RI-0 HSK now has a real local MVP API service for the core Human Sovereignty Kernel flow. The repository has been cleaned into a single baseline commit, tested locally, and verified through GitHub Actions CI.

This repository should not yet be described as fully production-ready infrastructure. It is a working MVP baseline that is ready for technical review and further production hardening.

## Current Verified State

- Main branch has a clean baseline commit.
- GitHub Actions CI is passing.
- `make test` passes.
- `make demo` passes.
- `make demo` uses the real FastAPI service-backed flow.
- Dockerfile and Docker Compose support exist.
- Runtime/generated reports are ignored and not committed.

## What Works Now

- FastAPI MVP service under `services/hsk_api/`
- SQLite local MVP persistence
- Ed25519 signing through the Python `cryptography` package
- Identity creation
- Consent grant
- Cryptographic proof generation
- Proof verification
- Tampered proof failure handling
- Consent revocation
- Audit trail generation
- API-backed demo flow
- Local test suite
- YAML validation
- Docker Compose local service support
- GitHub Actions CI validation

## Verified Commands

```bash
make setup
make test
make demo
```

Latest verified local result:

```text
YAML validation: 73 passed
Pytest: 56 passed, 24 skipped
Demo: passed
Output: reports/demo_audit.json
```

## Main API Service

The local MVP API is implemented in:

```text
services/hsk_api/
```

Main endpoints include:

```text
GET  /v1/health
POST /v1/identities
POST /v1/consents
POST /v1/proofs
POST /v1/proofs/verify
POST /v1/consents/{entry_id}/revoke
GET  /v1/audit
GET  /v1/audit/{did}
```

See:

```text
docs/API.md
```

## Docker Support

The repository includes:

```text
Dockerfile
docker-compose.yml
requirements.txt
```

Useful commands:

```bash
make docker-up
make docker-logs
make docker-down
```

## CI Status

GitHub Actions validates the repository on push and pull request by running setup, tests, demo, and Docker/API smoke checks where available.

## Important Limits

The current implementation is still MVP-level.

Known limits:

- SQLite is used for local persistence only.
- Local file-based system key storage is MVP-only and not production key management.
- API authentication is not yet fully hardened.
- Production audit storage must become append-only, persistent, tamper-evident, and backed by stronger infrastructure.
- Rust services still require a full compile/test pass in a Rust toolchain.
- Mobile SDKs still require proper build verification.
- Production deployment security still needs a full review.

## Remaining Engineering Priorities

1. Add API key authentication using `HSK_API_KEY`.
2. Protect write and audit endpoints.
3. Add production-grade key management design.
4. Add PostgreSQL-backed service mode.
5. Add stronger integration tests.
6. Add persistent append-only audit storage.
7. Complete Rust service compile/test verification.
8. Validate mobile SDK builds.
9. Rename repository from `ROI-HSK-` to `ROI-HSK`.
10. Prepare a technical review brief for partners/investors.

## Accurate External Positioning

Use this wording externally:

```text
RI-0 HSK is a pilot-ready AI governance and cryptographic consent MVP with a working FastAPI service, Ed25519 proof flow, SQLite local persistence, Docker support, passing tests, and green CI. It is currently in production hardening.
```

Do not claim:

```text
Fully production-ready government-grade infrastructure
```

until production database, key management, authentication, audit durability, deployment, and security review are complete.
