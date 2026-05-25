# RI-0 HSK — Human Sovereignty Kernel

RI-0 HSK is a pilot-ready AI governance and consent infrastructure project focused on cryptographic identity, consent proof, revocation, verification, and auditability.

This repository has been hardened into a clean demo-ready baseline.

## Current Status

- Status: Pilot-ready / production-hardening in progress
- Demo: Working locally with `make demo` against a real FastAPI-backed MVP service
- Tests: Passing with `make test`
- Verified results:
  - 49 unit tests passed
    - 72 YAML files validated
      - End-to-end demo flow passed using SQLite persistence and Ed25519 signatures

      ## Demo Flow

      The main demo proves the following flow:

      1. Create identity
      2. Grant consent
      3. Generate cryptographic proof
      4. Verify proof
      5. Revoke consent
      6. Show audit trail

      Run:

      ```bash
      make setup
      make demo
      ```

      The demo generates a local audit output and validates the full proof lifecycle.

      ## Running the API Locally

      Run the FastAPI service directly:

      ```bash
      make api
      ```

      Run the service in Docker Compose:

      ```bash
      make docker-up
      make docker-logs
      make docker-down
      ```

      ## Test Suite

      Run:

      ```bash
      make test
      ```

      Expected verified result:

      ```text
      YAML validation passed
      49 unit tests passed
      ```

      ## Useful Commands

      ```bash
      make setup
      make api
      make demo
      make test
      make docker-up
      make docker-logs
      make docker-down
      ```

      ## Repository Structure

      ```text
      .
      ├── Makefile
      ├── README.md
      ├── CURRENT_STATUS.md
      ├── DEVELOPMENT_NOTES.md
      ├── docs/
      ├── scripts/
      ├── tests/
      └── ...
      ```

      ## API Documentation

      See:

      ```text
      docs/API.md
      ```

      ## Current Engineering Position

      This repository should not yet be described as fully production-ready infrastructure.

      The accurate status is:

      ```text
      Pilot-ready / production-hardening in progress
      ```

      The local demo path is working, and the test suite is green, but further production work is required before live enterprise or government deployment.

      ## What Works Now

      - Local end-to-end demo flow
      - Identity creation
      - Consent grant
      - Proof generation
      - Proof verification
      - Consent revocation
      - Audit trail generation
      - YAML validation
      - Python unit test suite
      - Docker Compose local deployment proof
      - GitHub Actions CI smoke automation

      ## Next Engineering Steps

      1. Add GitHub Actions CI execution proof.
      2. Expand integration tests beyond the local demo.
      3. Validate all cryptographic flows against the final production key policy.
      4. Align all SDKs to the final `/v1` API contract.
      5. Add deployment-specific security hardening.
      6. Complete production observability and persistent audit storage.
      7. Review all claims before public investor or partner distribution.

      ## Repository

      Current GitHub repository:

      ```text
      https://github.com/segenb-prog/ROI-HSK-
      ```

      ## License

      Internal / controlled distribution unless otherwise specified.
