from __future__ import annotations

from typing import Any, List, Optional

from pydantic import BaseModel, Field


class HealthResponse(BaseModel):
    status: str = Field("healthy")
    version: str


class IdentityCreateRequest(BaseModel):
    public_key_b64: Optional[str] = None


class IdentityResponse(BaseModel):
    did: str
    public_key_b64: str
    created_at: str


class LedgerEntryRequest(BaseModel):
    did: str
    scope: List[str]
    purpose: str
    expires_at: Optional[str] = None


class LedgerEntryResponse(BaseModel):
    entry_id: str
    previous_entry_id: str
    did: str
    action: str
    scope: List[str]
    purpose: str
    issued_at: str
    expires_at: str
    signature_b64: str


class ProofRequest(BaseModel):
    entry_id: str


class ProofResponse(BaseModel):
    proof_type: str
    entry_id: str
    did: str
    action: str
    ledger_root: str
    public_key_b64: str
    signature_b64: str


class ProofVerifyRequest(BaseModel):
    proof_type: str
    entry_id: str
    did: str
    ledger_root: str
    public_key_b64: str
    signature_b64: str


class ProofVerifyResponse(BaseModel):
    valid: bool
    reason: Optional[str] = None


class ConsentRevokeRequest(BaseModel):
    reason: str = Field(default="user-requested")


class AuditEventResponse(BaseModel):
    event_id: str
    event_type: str
    timestamp: str
    payload_hash: str
    payload: dict[str, Any]
