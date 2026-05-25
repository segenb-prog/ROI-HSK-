# Current Status — RI-0 HSK

## Status label

**Pilot-ready / production-hardening in progress.**

The repository contains real architecture, service code, schemas, Kubernetes manifests, SDK material, and a deterministic cryptographic demo. It should not yet be described as fully production-ready until the Rust services, database-backed flow, CI, and deployment path are fully verified in a complete toolchain.

## What works now

- `make demo` runs a service-backed identity → consent grant → Ed25519 proof generation → proof verification → consent revocation → audit trail flow.
- The local service is implemented in `services/hsk_api/` using FastAPI and SQLite for persistence.
- Ed25519 signing is implemented through the Python `cryptography` package.
- Docker Compose support is available through `docker-compose.yml` for local service deployment.
- GitHub Actions CI validates `make setup`, `make test`, and `make demo`, with a Docker Compose smoke test in CI.
- Demo audit output is written to `reports/demo_audit.json`.
- Python unit tests pass in this environment.
- YAML smoke validation is available.
- PostgreSQL consent schema was hardened for known migration blockers.
- GitHub Actions was reduced to a valid smoke CI workflow.

## What was fixed in this hardening pass

- Added deterministic `scripts/demo_flow.py` using real Ed25519 signatures through `cryptography`.
- Added `make setup`, `make test`, `make demo`, `make docker-up`, and `make docker-down` targets.
- Fixed PostgreSQL partial index using volatile `NOW()` in `database-schemas/consent_ledger.sql`.
- Adjusted consent revoke compatibility by allowing zero-duration revoke entries and same timestamp revoke windows.
- Added strict Istio mTLS `PeerAuthentication` so the existing unit test requirement is satisfied.
- Replaced large/fragile CI workflows with a focused smoke CI and archived legacy workflows under `docs/archive/workflows/`.
- Added `.env.example`, `.gitignore`, `docs/API.md`, `CURRENT_STATUS.md`, `DEVELOPMENT_NOTES.md`, and `FILE_INVENTORY.md`.
- Reduced external claims from production-ready to pilot-ready / production-hardening where appropriate.
- Pinned Ed25519 dependency intent toward the v1 API currently used by the Rust code. A full v2 migration remains recommended.

## Test evidence from this environment

```text
python3 -m pytest tests/unit -q
49 passed

make demo
status: passed
identity created
grant proof verified
revocation proof verified
audit written to reports/demo_audit.json
```

## Not run / blockers

- Rust build was not executed here because this sandbox does not have `cargo` or `rustc` installed.
- Docker Compose was not started here because a Docker daemon is not available in this execution environment.
- PostgreSQL migrations were statically patched but not applied against a live database in this environment.
- Mobile SDK builds were not executed here because Android, iOS, Flutter, and React Native toolchains are not installed.

## How to verify locally

```bash
make setup
make test
make demo
cat reports/demo_audit.json
```

For Rust service verification on a local machine with Rust installed:

```bash
make rust-check
```

For Docker verification:

```bash
make docker-up
make docker-down
```

## Remaining risks

- Rust code still needs a full compile/test pass in a Rust toolchain.
- Rust Ed25519 API should be migrated cleanly to `ed25519-dalek` v2 instead of relying on v1-compatible code.
- The service API paths need final implementation alignment with `docs/API.md`.
- Demo audit is file-based; production audit must be append-only, persistent, tamper-evident, and backed by database/transparency-log storage.
- Several advanced service folders are architecture/prototype modules, not proven production services.
