# Development Notes — RI-0 HSK

## Architecture intent

RI-0 HSK is organized as a sovereignty/governance layer for AI systems. Its core primitives are:

- verifiable identity,
- explicit consent grant and revocation,
- cryptographic proof generation,
- proof verification,
- tamper-evident audit trails,
- future support for transparency logs, post-quantum crypto, and federation.

## Repository reality

The repository is a broad platform package. It contains Rust service code, SQL schemas, Kubernetes manifests, SDK prototypes, docs, CI definitions, monitoring material, and advanced cryptographic service concepts.

The immediate engineering priority is not to add more concepts. The priority is to stabilize one end-to-end path and make it undeniable.

## Recommended next engineering steps

1. Install Rust and run `cargo check --workspace`.
2. Migrate all Rust code to `ed25519-dalek` v2 or pin all crates consistently to v1-compatible APIs.
3. Add a database-backed `/v1` identity/consent/proof service matching `docs/API.md`.
4. Add integration tests that start PostgreSQL and verify the real server flow.
5. Align Python, TypeScript, Android, iOS, Flutter, and React Native SDKs to the same `/v1` API contract.
6. Replace demo file audit with append-only database audit plus transparency-log anchoring.
7. Reintroduce strict CI stages one by one after each stage is proven green.

## Current demo boundary

`make demo` is a local cryptographic proof-of-flow, not a claim that the whole platform is production-deployed. It is intentionally deterministic, dependency-light, and easy for a reviewer to run.

## Claim discipline

Use:

> pilot-ready / production-hardening in progress

Do not use:

> fully production-ready government-grade infrastructure

until builds, security tests, deployment, and operational evidence prove it.
