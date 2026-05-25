from __future__ import annotations

import base64
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey, Ed25519PublicKey

UTC = timezone.utc
GENESIS = "0" * 64


def now_iso() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat()


def canonical_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def sha256_hex(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def generate_ed25519_keypair() -> Ed25519PrivateKey:
    return Ed25519PrivateKey.generate()


def private_key_to_pem(private_key: Ed25519PrivateKey) -> bytes:
    return private_key.private_bytes(
        encoding=serialization.Encoding.PEM,
        format=serialization.PrivateFormat.PKCS8,
        encryption_algorithm=serialization.NoEncryption(),
    )


def load_private_key_pem(data: bytes) -> Ed25519PrivateKey:
    return serialization.load_pem_private_key(data, password=None)


def public_key_b64_from_private(private_key: Ed25519PrivateKey) -> str:
    raw = private_key.public_key().public_bytes(
        encoding=serialization.Encoding.Raw,
        format=serialization.PublicFormat.Raw,
    )
    return base64.b64encode(raw).decode("ascii")


def public_key_b64_from_raw(raw: bytes) -> str:
    return base64.b64encode(raw).decode("ascii")


def load_public_key_b64(raw_b64: str) -> Ed25519PublicKey:
    raw = base64.b64decode(raw_b64)
    return Ed25519PublicKey.from_public_bytes(raw)


def sign_payload(private_key: Ed25519PrivateKey, payload: dict[str, Any]) -> str:
    message = canonical_json(payload)
    signature = private_key.sign(message)
    return base64.b64encode(signature).decode("ascii")


def verify_signature(public_key_b64: str, signature_b64: str, payload: dict[str, Any]) -> bool:
    public_key = load_public_key_b64(public_key_b64)
    signature = base64.b64decode(signature_b64)
    public_key.verify(signature, canonical_json(payload))
    return True
