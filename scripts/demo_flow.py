#!/usr/bin/env python3
"""RI-0 HSK demo flow using the local FastAPI service."""
from __future__ import annotations

import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

import httpx

REPO_ROOT = Path(__file__).resolve().parent.parent
API_HOST = "127.0.0.1"
API_PORT = 8000
API_BASE_URL = f"http://{API_HOST}:{API_PORT}"
DEMO_AUDIT_PATH = Path(os.environ.get("HSK_DEMO_AUDIT_PATH", "reports/demo_audit.json"))


def start_service() -> subprocess.Popen[bytes]:
    env = os.environ.copy()
    env["PYTHONPATH"] = str(REPO_ROOT)
    command = [sys.executable, "-m", "uvicorn", "services.hsk_api.app:app", "--host", API_HOST, "--port", str(API_PORT), "--log-level", "warning"]
    process = subprocess.Popen(command, cwd=REPO_ROOT, env=env)
    wait_for_service_ready(timeout=15)
    return process


def wait_for_service_ready(timeout: int = 15) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            response = httpx.get(f"{API_BASE_URL}/v1/health", timeout=1.0)
            if response.status_code == 200:
                return
        except Exception:
            pass
        time.sleep(0.2)
    raise RuntimeError("HSK service did not become healthy in time")


def stop_service(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        return
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=5)


def request_json(client: httpx.Client, method: str, path: str, **kwargs: Any) -> Any:
    response = client.request(method, f"{API_BASE_URL}{path}", timeout=15.0, **kwargs)
    response.raise_for_status()
    return response.json()


def run_demo() -> dict[str, Any]:
    service_process = start_service()
    try:
        with httpx.Client() as client:
            health = request_json(client, "GET", "/v1/health")
            identity = request_json(client, "POST", "/v1/identities", json={})
            grant = request_json(
                client,
                "POST",
                "/v1/consents",
                json={"did": identity["did"], "scope": ["profile", "usage_metrics"], "purpose": "api-governance-demo"},
            )
            proof = request_json(client, "POST", "/v1/proofs", json={"entry_id": grant["entry_id"]})
            verification = request_json(client, "POST", "/v1/proofs/verify", json=proof)
            revoke = request_json(
                client,
                "POST",
                f"/v1/consents/{grant['entry_id']}/revoke",
                json={"reason": "user-requested"},
            )
            revoke_proof = request_json(client, "POST", "/v1/proofs", json={"entry_id": revoke["entry_id"]})
            revoke_verification = request_json(client, "POST", "/v1/proofs/verify", json=revoke_proof)
            audit_all = request_json(client, "GET", "/v1/audit")
            audit_for_identity = request_json(client, "GET", f"/v1/audit/{identity['did']}")

            result = {
                "status": "passed" if verification.get("valid") and revoke_verification.get("valid") else "failed",
                "health": health,
                "identity": identity,
                "grant": grant,
                "proof": proof,
                "proof_valid_before_revoke": verification.get("valid"),
                "verification": verification,
                "revoke": revoke,
                "revoke_proof": revoke_proof,
                "revoke_proof_valid": revoke_verification.get("valid"),
                "revoke_verification": revoke_verification,
                "audit": audit_all,
                "audit_all": audit_all,
                "audit_for_identity": audit_for_identity,
            }
            return result
    finally:
        stop_service(service_process)


def main() -> int:
    DEMO_AUDIT_PATH.parent.mkdir(parents=True, exist_ok=True)
    result = run_demo()
    DEMO_AUDIT_PATH.write_text(json.dumps(result, indent=2, sort_keys=True))
    print("RI-0 HSK API-backed demo flow")
    print(f"status: {result['status']}")
    print(f"identity: {result['identity']['did']}")
    print(f"grant_entry_id: {result['grant']['entry_id']}")
    print(f"revoke_entry_id: {result['revoke']['entry_id']}")
    print(f"audit_events: {len(result['audit_all'])}")
    print(f"audit_path: {DEMO_AUDIT_PATH}")
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
