from __future__ import annotations

import json
import sqlite3
import uuid
from pathlib import Path
from typing import Any

from .crypto import GENESIS, canonical_json, now_iso, sha256_hex


class HskStorage:
    def __init__(self, db_path: str | Path):
        self.db_path = Path(db_path)
        self.db_path.parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(str(self.db_path), check_same_thread=False)
        self.conn.row_factory = sqlite3.Row
        self.conn.execute("PRAGMA foreign_keys = ON")
        self.conn.execute("PRAGMA journal_mode = WAL")
        self.conn.execute("PRAGMA synchronous = NORMAL")
        self._initialize_schema()

    def _initialize_schema(self) -> None:
        self.conn.execute(
            """
            CREATE TABLE IF NOT EXISTS identities (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                did TEXT UNIQUE NOT NULL,
                public_key_b64 TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            """
        )
        self.conn.execute(
            """
            CREATE TABLE IF NOT EXISTS ledger_entries (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                entry_id TEXT UNIQUE NOT NULL,
                previous_entry_id TEXT NOT NULL,
                did TEXT NOT NULL,
                action TEXT NOT NULL,
                scope_json TEXT NOT NULL,
                purpose TEXT NOT NULL,
                issued_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                signature_b64 TEXT NOT NULL,
                created_at TEXT NOT NULL
            )
            """
        )
        self.conn.execute(
            """
            CREATE TABLE IF NOT EXISTS audit_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT UNIQUE NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                payload_hash TEXT NOT NULL,
                payload_json TEXT NOT NULL
            )
            """
        )
        self.conn.commit()

    def create_identity(self, did: str, public_key_b64: str, created_at: str) -> None:
        self.conn.execute(
            "INSERT INTO identities (did, public_key_b64, created_at) VALUES (?, ?, ?)",
            (did, public_key_b64, created_at),
        )
        self.conn.commit()

    def get_identity(self, did: str) -> dict[str, Any] | None:
        row = self.conn.execute("SELECT did, public_key_b64, created_at FROM identities WHERE did = ?", (did,)).fetchone()
        return dict(row) if row else None

    def insert_ledger_entry(self, entry: dict[str, Any]) -> None:
        self.conn.execute(
            "INSERT INTO ledger_entries (entry_id, previous_entry_id, did, action, scope_json, purpose, issued_at, expires_at, signature_b64, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                entry["entry_id"],
                entry["previous_entry_id"],
                entry["did"],
                entry["action"],
                json.dumps(entry["scope"], sort_keys=True, separators=(",", ":")),
                entry["purpose"],
                entry["issued_at"],
                entry["expires_at"],
                entry["signature_b64"],
                now_iso(),
            ),
        )
        self.conn.commit()

    def get_ledger_entry(self, entry_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            "SELECT entry_id, previous_entry_id, did, action, scope_json, purpose, issued_at, expires_at, signature_b64 FROM ledger_entries WHERE entry_id = ?",
            (entry_id,),
        ).fetchone()
        return self._row_to_entry(row) if row else None

    def list_ledger_entries(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            "SELECT entry_id, previous_entry_id, did, action, scope_json, purpose, issued_at, expires_at, signature_b64 FROM ledger_entries ORDER BY id ASC"
        ).fetchall()
        return [self._row_to_entry(row) for row in rows]

    def get_last_entry_id(self) -> str:
        row = self.conn.execute("SELECT entry_id FROM ledger_entries ORDER BY id DESC LIMIT 1").fetchone()
        return row["entry_id"] if row else GENESIS

    def ledger_root(self) -> str:
        entries = self.list_ledger_entries()
        return sha256_hex(canonical_json(entries))

    def add_audit_event(self, event_type: str, payload: dict[str, Any]) -> None:
        payload_json = json.dumps(payload, sort_keys=True, separators=(",", ":"))
        payload_hash = sha256_hex(canonical_json(payload))
        self.conn.execute(
            "INSERT INTO audit_events (event_id, event_type, timestamp, payload_hash, payload_json) VALUES (?, ?, ?, ?, ?)",
            (str(uuid.uuid4()), event_type, now_iso(), payload_hash, payload_json),
        )
        self.conn.commit()

    def list_audit_events(self, did: str | None = None) -> list[dict[str, Any]]:
        if did:
            pattern = f'%"did":"{did}"%'
            rows = self.conn.execute(
                "SELECT event_id, event_type, timestamp, payload_hash, payload_json FROM audit_events WHERE payload_json LIKE ? ORDER BY id ASC",
                (pattern,),
            ).fetchall()
        else:
            rows = self.conn.execute(
                "SELECT event_id, event_type, timestamp, payload_hash, payload_json FROM audit_events ORDER BY id ASC"
            ).fetchall()
        return [self._row_to_audit(row) for row in rows]

    @staticmethod
    def _row_to_entry(row: sqlite3.Row) -> dict[str, Any]:
        if row is None:
            return None
        return {
            "entry_id": row["entry_id"],
            "previous_entry_id": row["previous_entry_id"],
            "did": row["did"],
            "action": row["action"],
            "scope": json.loads(row["scope_json"]),
            "purpose": row["purpose"],
            "issued_at": row["issued_at"],
            "expires_at": row["expires_at"],
            "signature_b64": row["signature_b64"],
        }

    @staticmethod
    def _row_to_audit(row: sqlite3.Row) -> dict[str, Any]:
        return {
            "event_id": row["event_id"],
            "event_type": row["event_type"],
            "timestamp": row["timestamp"],
            "payload_hash": row["payload_hash"],
            "payload": json.loads(row["payload_json"]),
        }
