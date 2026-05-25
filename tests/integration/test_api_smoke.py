from pathlib import Path

from fastapi.testclient import TestClient

from services.hsk_api.app import create_app


def test_api_smoke_flow(tmp_path: Path) -> None:
    system_key_path = tmp_path / "system_key.pem"
    app = create_app(db_path=":memory:", key_path=system_key_path)
    client = TestClient(app)

    health = client.get("/v1/health")
    assert health.status_code == 200
    assert health.json()["status"] == "healthy"

    identity = client.post("/v1/identities", json={}).json()
    assert identity["did"].startswith("did:ri0:")

    consent = client.post(
        "/v1/consents",
        json={"did": identity["did"], "scope": ["profile"], "purpose": "smoke-test"},
    )
    assert consent.status_code == 201

    proof = client.post("/v1/proofs", json={"entry_id": consent.json()["entry_id"]})
    assert proof.status_code == 200

    verify = client.post("/v1/proofs/verify", json=proof.json())
    assert verify.status_code == 200
    assert verify.json()["valid"] is True

    entry_id = consent.json()["entry_id"]
    revoke = client.post(
        f"/v1/consents/{entry_id}/revoke",
        json={"reason": "user-requested"},
    )
    assert revoke.status_code == 201

    audit = client.get(f"/v1/audit/{identity['did']}")
    assert audit.status_code == 200
    assert isinstance(audit.json(), list)
