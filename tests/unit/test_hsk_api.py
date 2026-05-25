from pathlib import Path

from fastapi.testclient import TestClient

from services.hsk_api.app import create_app


def make_client(tmp_path: Path) -> TestClient:
    system_key_path = tmp_path / "system_key.pem"
    app = create_app(db_path=":memory:", key_path=system_key_path)
    return TestClient(app)


def test_health_check(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    response = client.get("/v1/health")
    assert response.status_code == 200
    assert response.json()["status"] == "healthy"
    assert response.json()["version"] == "0.1.0"


def test_identity_creation(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    response = client.post("/v1/identities", json={})
    assert response.status_code == 201
    body = response.json()
    assert body["did"].startswith("did:ri0:")
    assert body["public_key_b64"]
    assert body["created_at"]


def test_consent_grant_creates_ledger_entry(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    identity = client.post("/v1/identities", json={}).json()
    grant = client.post(
        "/v1/consents",
        json={"did": identity["did"], "scope": ["profile"], "purpose": "api-test"},
    )
    assert grant.status_code == 201
    payload = grant.json()
    assert payload["action"] == "grant"
    assert payload["did"] == identity["did"]
    assert payload["signature_b64"]


def test_proof_generation_and_verification_success(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    identity = client.post("/v1/identities", json={}).json()
    grant = client.post(
        "/v1/consents",
        json={"did": identity["did"], "scope": ["profile"], "purpose": "proof-test"},
    ).json()
    proof = client.post("/v1/proofs", json={"entry_id": grant["entry_id"]})
    assert proof.status_code == 200
    proof_body = proof.json()
    assert proof_body["did"] == identity["did"]
    verify = client.post("/v1/proofs/verify", json=proof_body)
    assert verify.status_code == 200
    assert verify.json()["valid"] is True


def test_proof_verification_failure_for_tampered_proof(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    identity = client.post("/v1/identities", json={}).json()
    grant = client.post(
        "/v1/consents",
        json={"did": identity["did"], "scope": ["profile"], "purpose": "tamper-test"},
    ).json()
    proof = client.post("/v1/proofs", json={"entry_id": grant["entry_id"]}).json()
    proof["ledger_root"] = "deadbeef" * 8
    verify = client.post("/v1/proofs/verify", json=proof)
    assert verify.status_code == 200
    assert verify.json()["valid"] is False
    assert "ledger" in verify.json()["reason"]


def test_consent_revocation_and_audit_events(tmp_path: Path) -> None:
    client = make_client(tmp_path)
    identity = client.post("/v1/identities", json={}).json()
    grant = client.post(
        "/v1/consents",
        json={"did": identity["did"], "scope": ["profile"], "purpose": "revoke-test"},
    ).json()
    revoke = client.post(
        f"/v1/consents/{grant['entry_id']}/revoke",
        json={"reason": "user-requested"},
    )
    assert revoke.status_code == 201
    revoke_body = revoke.json()
    assert revoke_body["action"] == "revoke"
    assert revoke_body["did"] == identity["did"]
    audit_all = client.get("/v1/audit").json()
    audit_identity = client.get(f"/v1/audit/{identity['did']}").json()
    assert any(event["event_type"] == "identity.created" for event in audit_all)
    assert any(event["event_type"] == "consent.revoked" for event in audit_identity)
    assert isinstance(audit_identity, list)
