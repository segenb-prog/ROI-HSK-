from __future__ import annotations

import os
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path
from typing import Any

from fastapi import Body, FastAPI, HTTPException, status

from cryptography.hazmat.primitives import serialization

from .crypto import (
    canonical_json,
    generate_ed25519_keypair,
    load_private_key_pem,
    now_iso,
    public_key_b64_from_private,
    sign_payload,
    verify_signature,
    sha256_hex,
)
from .db import HskStorage
from .schemas import (
    AuditEventResponse,
    HealthResponse,
    IdentityCreateRequest,
    IdentityResponse,
    LedgerEntryRequest,
    LedgerEntryResponse,
    ProofRequest,
    ProofResponse,
    ProofVerifyRequest,
    ProofVerifyResponse,
    ConsentRevokeRequest,
)

APP_VERSION = "0.1.0"
UTC = timezone.utc


def get_default_db_path() -> Path:
    return Path(os.getenv("HSK_DB_PATH", Path(__file__).resolve().parent / "data" / "hsk.db"))


def get_default_key_path() -> Path:
    return Path(os.getenv("HSK_SYSTEM_KEY_PATH", Path(__file__).resolve().parent / "data" / "system_key.pem"))


def build_signed_entry(
    previous_entry_id: str,
    did: str,
    action: str,
    scope: list[str],
    purpose: str,
    issued_at: str,
    expires_at: str,
    signing_key: Any,
) -> dict[str, Any]:
    unsigned = {
        "previous_entry_id": previous_entry_id,
        "did": did,
        "action": action,
        "scope": scope,
        "purpose": purpose,
        "issued_at": issued_at,
        "expires_at": expires_at,
    }
    entry_id = sha256_hex(canonical_json(unsigned))
    payload = {"entry_id": entry_id, **unsigned}
    signature_b64 = sign_payload(signing_key, payload)
    return {**payload, "signature_b64": signature_b64}


def create_app(db_path: str | Path | None = None, key_path: str | Path | None = None) -> FastAPI:
    app = FastAPI(title="RI-0 HSK API", version=APP_VERSION)
    storage = HskStorage(db_path or get_default_db_path())
    app.state.storage = storage

    key_path = Path(key_path or get_default_key_path())
    key_path.parent.mkdir(parents=True, exist_ok=True)
    if key_path.exists():
        signing_key = load_private_key_pem(key_path.read_bytes())
    else:
        signing_key = generate_ed25519_keypair()
        key_path.write_bytes(signing_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        ))

    app.state.signing_key = signing_key
    app.state.system_public_key_b64 = public_key_b64_from_private(signing_key)

    @app.get("/v1/health", response_model=HealthResponse)
    def health() -> HealthResponse:
        return HealthResponse(status="healthy", version=APP_VERSION)

    @app.post("/v1/identities", response_model=IdentityResponse, status_code=status.HTTP_201_CREATED)
    def create_identity(request: IdentityCreateRequest = Body(default_factory=IdentityCreateRequest)) -> IdentityResponse:
        public_key_b64 = request.public_key_b64
        if public_key_b64 is None:
            public_key_b64 = public_key_b64_from_private(generate_ed25519_keypair())
        did = f"did:ri0:{uuid.uuid4()}"
        created_at = now_iso()
        storage.create_identity(did, public_key_b64, created_at)
        storage.add_audit_event("identity.created", {"did": did, "public_key_b64": public_key_b64})
        return IdentityResponse(did=did, public_key_b64=public_key_b64, created_at=created_at)

    @app.get("/v1/identities/{did}", response_model=IdentityResponse)
    def get_identity(did: str) -> IdentityResponse:
        identity = storage.get_identity(did)
        if identity is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Identity not found")
        return IdentityResponse(**identity)

    def parse_expiry(expires_at: str | None) -> str:
        if expires_at:
            return expires_at
        return (datetime.now(UTC).replace(microsecond=0) + timedelta(minutes=60)).isoformat()

    @app.post("/v1/consents", response_model=LedgerEntryResponse, status_code=status.HTTP_201_CREATED)
    def grant_consent(request: LedgerEntryRequest) -> LedgerEntryResponse:
        identity = storage.get_identity(request.did)
        if identity is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Identity not found")
        issued_at = now_iso()
        expires_at = parse_expiry(request.expires_at)
        entry = build_signed_entry(
            storage.get_last_entry_id(),
            request.did,
            "grant",
            request.scope,
            request.purpose,
            issued_at,
            expires_at,
            app.state.signing_key,
        )
        storage.insert_ledger_entry(entry)
        storage.add_audit_event("consent.granted", entry)
        return LedgerEntryResponse(**entry)

    @app.post("/v1/proofs", response_model=ProofResponse)
    def create_proof(request: ProofRequest) -> ProofResponse:
        entry = storage.get_ledger_entry(request.entry_id)
        if entry is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Ledger entry not found")
        proof = {
            "proof_type": "RI0-HSK-ConsentProof-v1",
            "entry_id": entry["entry_id"],
            "did": entry["did"],
            "action": entry["action"],
            "ledger_root": storage.ledger_root(),
            "public_key_b64": app.state.system_public_key_b64,
            "signature_b64": entry["signature_b64"],
        }
        storage.add_audit_event("proof.generated", proof)
        return ProofResponse(**proof)

    @app.post("/v1/proofs/verify", response_model=ProofVerifyResponse)
    def verify_proof_endpoint(request: ProofVerifyRequest) -> ProofVerifyResponse:
        entry = storage.get_ledger_entry(request.entry_id)
        if entry is None:
            storage.add_audit_event("proof.verified", {"entry_id": request.entry_id, "valid": False, "reason": "entry not found"})
            return ProofVerifyResponse(valid=False, reason="entry not found")
        if entry["did"] != request.did:
            storage.add_audit_event("proof.verified", {"entry_id": request.entry_id, "valid": False, "reason": "did mismatch"})
            return ProofVerifyResponse(valid=False, reason="did mismatch")
        payload = {k: v for k, v in entry.items() if k != "signature_b64"}
        try:
            verify_signature(request.public_key_b64, request.signature_b64, payload)
        except Exception:
            storage.add_audit_event("proof.verified", {"entry_id": request.entry_id, "valid": False, "reason": "invalid signature"})
            return ProofVerifyResponse(valid=False, reason="invalid signature")
        valid = request.ledger_root == storage.ledger_root()
        storage.add_audit_event("proof.verified", {"entry_id": request.entry_id, "valid": valid})
        if not valid:
            return ProofVerifyResponse(valid=False, reason="ledger root mismatch")
        return ProofVerifyResponse(valid=True)

    @app.post("/v1/consents/{entry_id}/revoke", response_model=LedgerEntryResponse, status_code=status.HTTP_201_CREATED)
    def revoke_consent(entry_id: str, request: ConsentRevokeRequest) -> LedgerEntryResponse:
        grant_entry = storage.get_ledger_entry(entry_id)
        if grant_entry is None:
            raise HTTPException(status_code=status.HTTP_404_NOT_FOUND, detail="Ledger entry not found")
        issued_at = now_iso()
        revoke_entry = build_signed_entry(
            storage.get_last_entry_id(),
            grant_entry["did"],
            "revoke",
            grant_entry["scope"],
            request.reason,
            issued_at,
            issued_at,
            app.state.signing_key,
        )
        storage.insert_ledger_entry(revoke_entry)
        storage.add_audit_event("consent.revoked", {"revoked_entry_id": entry_id, **revoke_entry})
        return LedgerEntryResponse(**revoke_entry)

    @app.get("/v1/audit", response_model=list[AuditEventResponse])
    def audit_all() -> list[AuditEventResponse]:
        return [AuditEventResponse(**event) for event in storage.list_audit_events()]

    @app.get("/v1/audit/{did}", response_model=list[AuditEventResponse])
    def audit_for_did(did: str) -> list[AuditEventResponse]:
        return [AuditEventResponse(**event) for event in storage.list_audit_events(did)]

    return app


app = create_app()
