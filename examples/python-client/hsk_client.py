#!/usr/bin/env python3
"""
HSK Falsification Machine Python Client

Official Python SDK for the HSK Falsification Machine API.
Compatible with OpenAPI 3.0 specification.

Example usage:
    from hsk_client import HSKClient, ConsentLedgerClient
    
    # HSK Verifier client
    hsk = HSKClient("https://verifier.hskernel.dev", api_key="your-api-key")
    
    # Challenge a system
    challenge = hsk.challenge("my-ai-system", timeout_hours=72)
    print(f"Challenge ID: {challenge.request_id}")
    print(f"Deadline: {challenge.deadline}")
    
    # Submit response
    result = hsk.submit_response(response_data)
    print(f"Status: {result.status}")
    
    # Consent Ledger client
    consent = ConsentLedgerClient("https://identity.hskernel.dev")
    
    # Register citizen
    citizen = consent.register_citizen("did:hsk:user:123", public_key)
    
    # Grant consent
    entry = consent.grant_consent(
        citizen_did="did:hsk:user:123",
        scope=["health_data", "location"],
        purpose="medical_research",
        duration_seconds=2592000,
        citizen_signature=signature
    )
"""

from __future__ import annotations

import requests
import json
from typing import Optional, Dict, Any, List, Union
from dataclasses import dataclass, field
from datetime import datetime
from enum import Enum
import base64


class HSKError(Exception):
    """Base exception for HSK client errors"""
    
    def __init__(self, message: str, status_code: Optional[int] = None, response: Optional[Dict] = None):
        super().__init__(message)
        self.status_code = status_code
        self.response = response


class AuthenticationError(HSKError):
    """Raised when authentication fails"""
    pass


class ValidationError(HSKError):
    """Raised when request validation fails"""
    pass


class NotFoundError(HSKError):
    """Raised when resource is not found"""
    pass


class ProofType(str, Enum):
    """Types of proofs that can be requested"""
    CONSENT_LEDGER = "ConsentLedger"
    MEMORY_PASSPORT = "MemoryPassport"
    DELETION_PROOF = "DeletionProof"
    PREDICTION_SCOPE = "PredictionScope"


@dataclass
class Challenge:
    """Represents an HSK challenge request"""
    request_id: str
    system_id: str
    deadline: datetime
    requested_proofs: List[ProofType]
    issued_at: Optional[datetime] = None
    nonce: Optional[str] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Challenge:
        """Create Challenge from API response dict"""
        return cls(
            request_id=data["request_id"],
            system_id=data["system_id"],
            deadline=datetime.fromisoformat(data["deadline"].replace("Z", "+00:00")),
            requested_proofs=[ProofType(p) for p in data.get("requested_proofs", [])],
            issued_at=datetime.fromisoformat(data["issued_at"].replace("Z", "+00:00")) if "issued_at" in data else None,
            nonce=data.get("nonce")
        )


@dataclass
class EvaluationResult:
    """Represents an evaluation result"""
    status: str  # 'compliant' or 'violation'
    certificate_id: Optional[str] = None
    reason: Optional[str] = None
    missing_proofs: List[ProofType] = field(default_factory=list)
    invalid_proofs: List[ProofType] = field(default_factory=list)
    
    @property
    def is_compliant(self) -> bool:
        """Check if the evaluation resulted in compliance"""
        return self.status == "compliant"
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> EvaluationResult:
        """Create EvaluationResult from API response dict"""
        return cls(
            status=data["status"],
            certificate_id=data.get("certificate_id"),
            reason=data.get("reason"),
            missing_proofs=[ProofType(p) for p in data.get("missing_proofs", [])],
            invalid_proofs=[ProofType(p) for p in data.get("invalid_proofs", [])]
        )


@dataclass
class Certificate:
    """Represents an HSK certificate"""
    certificate_id: str
    system_id: str
    evaluation_time: datetime
    hs_compliant: bool
    violations: List[str] = field(default_factory=list)
    missing_proofs: List[str] = field(default_factory=list)
    invalid_proofs: List[str] = field(default_factory=list)
    issuer_public_key: Optional[str] = None
    issuer_signature: Optional[str] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Certificate:
        """Create Certificate from API response dict"""
        return cls(
            certificate_id=data["certificate_id"],
            system_id=data["system_id"],
            evaluation_time=datetime.fromisoformat(data["evaluation_time"].replace("Z", "+00:00")),
            hs_compliant=data["hs_compliant"],
            violations=data.get("violations", []),
            missing_proofs=data.get("missing_proofs", []),
            invalid_proofs=data.get("invalid_proofs", []),
            issuer_public_key=data.get("issuer_public_key"),
            issuer_signature=data.get("issuer_signature")
        )


@dataclass
class TransparencyEntry:
    """Represents a transparency log entry"""
    certificate_id: str
    system_id: str
    timestamp: datetime
    compliant: bool
    merkle_root: Optional[str] = None
    position: Optional[int] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> TransparencyEntry:
        """Create TransparencyEntry from API response dict"""
        return cls(
            certificate_id=data["certificate_id"],
            system_id=data["system_id"],
            timestamp=datetime.fromisoformat(data["timestamp"].replace("Z", "+00:00")),
            compliant=data["compliant"],
            merkle_root=data.get("merkle_root"),
            position=data.get("position")
        )


class HSKClient:
    """
    Client for the HSK Falsification Machine API.
    
    This client provides methods to challenge AI systems, evaluate proofs,
    and manage violation certificates.
    
    Args:
        base_url: Base URL of the HSK verifier server (e.g., "https://verifier.hskernel.dev")
        api_key: Optional API key for authentication
        timeout: Request timeout in seconds (default: 30)
        verify_ssl: Whether to verify SSL certificates (default: True)
    
    Example:
        >>> client = HSKClient("https://verifier.hskernel.dev", api_key="secret-key")
        >>> health = client.health_check()
        >>> print(health["status"])
        'healthy'
    """
    
    def __init__(
        self,
        base_url: str,
        api_key: Optional[str] = None,
        timeout: int = 30,
        verify_ssl: bool = True
    ):
        self.base_url = base_url.rstrip('/')
        self.api_key = api_key
        self.timeout = timeout
        self.session = requests.Session()
        self.session.verify = verify_ssl
        
        if api_key:
            self.session.headers['Authorization'] = f'Bearer {api_key}'
        
        self.session.headers['Content-Type'] = 'application/json'
        self.session.headers['Accept'] = 'application/json'
        self.session.headers['User-Agent'] = 'hsk-python-client/0.1.0'
    
    def _request(
        self,
        method: str,
        path: str,
        **kwargs
    ) -> Dict[str, Any]:
        """Make an HTTP request to the API"""
        url = f"{self.base_url}{path}"
        
        try:
            response = self.session.request(
                method,
                url,
                timeout=self.timeout,
                **kwargs
            )
            
            if response.status_code == 401:
                raise AuthenticationError("Invalid or missing API key", 401)
            elif response.status_code == 404:
                raise NotFoundError(f"Resource not found: {path}", 404)
            elif response.status_code == 422:
                error_data = response.json() if response.content else {}
                raise ValidationError(
                    error_data.get('error', 'Validation failed'),
                    422,
                    error_data
                )
            
            response.raise_for_status()
            return response.json() if response.content else {}
            
        except requests.exceptions.HTTPError as e:
            error_data = e.response.json() if e.response.content else {}
            raise HSKError(
                error_data.get('error', f'HTTP {e.response.status_code}'),
                e.response.status_code,
                error_data
            )
        except requests.exceptions.Timeout:
            raise HSKError(f"Request timed out after {self.timeout}s")
        except requests.exceptions.RequestException as e:
            raise HSKError(f"Request failed: {str(e)}")
    
    def health_check(self) -> Dict[str, Any]:
        """
        Check the health of the HSK verifier server.
        
        Returns:
            Dict containing status, key_id, and timestamp
        
        Example:
            >>> health = client.health_check()
            >>> print(health["status"])
            'healthy'
        """
        return self._request('GET', '/health')
    
    def challenge(
        self,
        system_id: str,
        timeout_hours: int = 72
    ) -> Challenge:
        """
        Create a new challenge for a system.
        
        Args:
            system_id: Unique identifier for the system being challenged
            timeout_hours: Challenge deadline in hours (default: 72)
        
        Returns:
            Challenge object with request details
        
        Example:
            >>> challenge = client.challenge("my-ai-system", timeout_hours=48)
            >>> print(challenge.request_id)
            '550e8400-e29b-41d4-a716-446655440000'
        """
        data = {
            'system_id': system_id,
            'timeout_hours': timeout_hours
        }
        
        response = self._request('POST', '/challenge', json=data)
        return Challenge.from_dict(response)
    
    def get_challenge(self, request_id: str) -> Challenge:
        """
        Get challenge details by request ID.
        
        Args:
            request_id: The challenge request ID
        
        Returns:
            Challenge object
        
        Raises:
            NotFoundError: If challenge is not found
        """
        response = self._request('GET', f'/challenge/{request_id}')
        return Challenge.from_dict(response)
    
    def submit_response(
        self,
        request_id: str,
        system_id: str,
        provided_proofs: List[Dict[str, Any]],
        submitted_at: Optional[datetime] = None
    ) -> EvaluationResult:
        """
        Submit a system's response to a challenge.
        
        Args:
            request_id: The challenge request ID
            system_id: The system identifier
            provided_proofs: List of proof objects with 'proof_type' and 'data' keys
            submitted_at: Optional submission timestamp (defaults to now)
        
        Returns:
            EvaluationResult with status and details
        
        Example:
            >>> proofs = [{"proof_type": "ConsentLedger", "data": "base64encoded..."}]
            >>> result = client.submit_response(req_id, sys_id, proofs)
            >>> if result.is_compliant:
            ...     print("System is compliant!")
            ... else:
            ...     print(f"Violation: {result.reason}")
        """
        data = {
            'request_id': request_id,
            'system_id': system_id,
            'provided_proofs': provided_proofs,
            'submitted_at': (submitted_at or datetime.utcnow()).isoformat() + 'Z'
        }
        
        response = self._request('POST', '/response', json=data)
        return EvaluationResult.from_dict(response)
    
    def list_certificates(
        self,
        system_id: Optional[str] = None,
        compliant: Optional[bool] = None,
        limit: int = 100
    ) -> List[Certificate]:
        """
        List issued certificates.
        
        Args:
            system_id: Filter by system ID
            compliant: Filter by compliance status
            limit: Maximum number of results (default: 100, max: 1000)
        
        Returns:
            List of Certificate objects
        """
        params = {'limit': min(limit, 1000)}
        if system_id:
            params['system_id'] = system_id
        if compliant is not None:
            params['compliant'] = compliant
            
        response = self._request('GET', '/certificates', params=params)
        return [Certificate.from_dict(c) for c in response]
    
    def get_certificate(self, cert_id: str) -> Certificate:
        """
        Get certificate details by ID.
        
        Args:
            cert_id: The certificate ID (short or full)
        
        Returns:
            Certificate object
        
        Raises:
            NotFoundError: If certificate is not found
        """
        response = self._request('GET', f'/certificates/{cert_id}')
        return Certificate.from_dict(response)
    
    def verify_certificate(self, cert_id: str) -> Dict[str, Any]:
        """
        Verify a certificate's signature.
        
        Args:
            cert_id: The certificate ID
        
        Returns:
            Dict with verification result including 'valid' boolean
        
        Example:
            >>> result = client.verify_certificate("abc123")
            >>> print(result["valid"])
            True
        """
        return self._request('GET', f'/verify/{cert_id}')
    
    def submit_to_transparency_log(
        self,
        certificate: Dict[str, Any],
        log_url: str
    ) -> Dict[str, Any]:
        """
        Submit a certificate to a transparency log.
        
        Args:
            certificate: The certificate to submit
            log_url: URL of the transparency log
        
        Returns:
            Submission response with position and merkle root
        """
        data = {
            'certificate': certificate,
            'log_url': log_url
        }
        return self._request('POST', '/transparency/submit', json=data)
    
    def query_transparency_log(
        self,
        certificate_id: Optional[str] = None,
        system_id: Optional[str] = None,
        start_time: Optional[datetime] = None,
        end_time: Optional[datetime] = None
    ) -> List[TransparencyEntry]:
        """
        Query the transparency log.
        
        Args:
            certificate_id: Filter by certificate ID
            system_id: Filter by system ID
            start_time: Start of time range
            end_time: End of time range
        
        Returns:
            List of TransparencyEntry objects
        """
        params: Dict[str, Any] = {}
        if certificate_id:
            params['certificate_id'] = certificate_id
        if system_id:
            params['system_id'] = system_id
        if start_time:
            params['start_time'] = start_time.isoformat()
        if end_time:
            params['end_time'] = end_time.isoformat()
            
        response = self._request('GET', '/transparency/query', params=params)
        return [TransparencyEntry.from_dict(e) for e in response]


@dataclass
class Citizen:
    """Represents a registered citizen"""
    id: str
    did: str
    public_key: str
    created_at: datetime
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> Citizen:
        return cls(
            id=data["id"],
            did=data["did"],
            public_key=data["public_key"],
            created_at=datetime.fromisoformat(data["created_at"].replace("Z", "+00:00"))
        )


@dataclass
class ConsentEntry:
    """Represents a consent ledger entry"""
    entry_id: str
    action: str
    scope: List[str]
    purpose: str
    duration_seconds: int
    granted_at: datetime
    expires_at: datetime
    previous_entry_id: str
    public_key: str
    signature: str
    system_signature: Optional[str] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> ConsentEntry:
        return cls(
            entry_id=data["entry_id"],
            action=data["action"],
            scope=data["scope"],
            purpose=data["purpose"],
            duration_seconds=data["duration_seconds"],
            granted_at=datetime.fromisoformat(data["granted_at"].replace("Z", "+00:00")),
            expires_at=datetime.fromisoformat(data["expires_at"].replace("Z", "+00:00")),
            previous_entry_id=data["previous_entry_id"],
            public_key=data["public_key"],
            signature=data["signature"],
            system_signature=data.get("system_signature")
        )


@dataclass
class HSKProofs:
    """Represents HSK proof package for a citizen"""
    citizen_did: str
    public_key: str
    entry_count: int
    entries: List[ConsentEntry]
    proof_type: str
    latest_entry_id: Optional[str] = None
    
    @classmethod
    def from_dict(cls, data: Dict[str, Any]) -> HSKProofs:
        return cls(
            citizen_did=data["citizen_did"],
            public_key=data["public_key"],
            entry_count=data["entry_count"],
            entries=[ConsentEntry.from_dict(e) for e in data.get("entries", [])],
            proof_type=data["proof_type"],
            latest_entry_id=data.get("latest_entry_id")
        )


class ConsentLedgerClient:
    """
    Client for the Digital Identity + Consent Ledger API.
    
    This client provides methods to manage citizen identities and consent entries.
    
    Args:
        base_url: Base URL of the consent ledger server
        timeout: Request timeout in seconds (default: 30)
        verify_ssl: Whether to verify SSL certificates (default: True)
    
    Example:
        >>> client = ConsentLedgerClient("https://identity.hskernel.dev")
        >>> citizen = client.register_citizen("did:hsk:user:123", public_key)
        >>> consents = client.get_citizen_consents("did:hsk:user:123")
    """
    
    def __init__(self, base_url: str, timeout: int = 30, verify_ssl: bool = True):
        self.base_url = base_url.rstrip('/')
        self.timeout = timeout
        self.session = requests.Session()
        self.session.verify = verify_ssl
        self.session.headers['Content-Type'] = 'application/json'
        self.session.headers['Accept'] = 'application/json'
        self.session.headers['User-Agent'] = 'hsk-python-client/0.1.0'
    
    def _request(self, method: str, path: str, **kwargs) -> Dict[str, Any]:
        """Make an HTTP request to the API"""
        url = f"{self.base_url}{path}"
        
        try:
            response = self.session.request(method, url, timeout=self.timeout, **kwargs)
            
            if response.status_code == 404:
                raise NotFoundError(f"Resource not found: {path}", 404)
            
            response.raise_for_status()
            return response.json() if response.content else {}
            
        except requests.exceptions.HTTPError as e:
            error_data = e.response.json() if e.response.content else {}
            raise HSKError(
                error_data.get('error', f'HTTP {e.response.status_code}'),
                e.response.status_code,
                error_data
            )
        except requests.exceptions.RequestException as e:
            raise HSKError(f"Request failed: {str(e)}")
    
    def health_check(self) -> Dict[str, Any]:
        """Check the health of the consent ledger server"""
        return self._request('GET', '/health')
    
    def register_citizen(self, did: str, public_key: str) -> Citizen:
        """
        Register a new citizen.
        
        Args:
            did: Decentralized Identifier (e.g., "did:hsk:user:123")
            public_key: Base64-encoded Ed25519 public key (32 bytes)
        
        Returns:
            Citizen object
        """
        data = {'did': did, 'public_key': public_key}
        response = self._request('POST', '/citizens', json=data)
        return Citizen.from_dict(response)
    
    def get_citizen(self, did: str) -> Citizen:
        """Get citizen information by DID"""
        response = self._request('GET', f'/citizens/{did}')
        return Citizen.from_dict(response)
    
    def get_citizen_consents(self, did: str) -> List[ConsentEntry]:
        """Get all consent entries for a citizen"""
        response = self._request('GET', f'/citizens/{did}/consents')
        return [ConsentEntry.from_dict(e) for e in response]
    
    def grant_consent(
        self,
        citizen_did: str,
        scope: List[str],
        purpose: str,
        duration_seconds: int,
        citizen_signature: str,
        constraints: Optional[Dict[str, Any]] = None
    ) -> ConsentEntry:
        """
        Grant consent for data access.
        
        Args:
            citizen_did: Citizen's DID
            scope: List of resources being consented (e.g., ["health_data", "location"])
            purpose: Purpose of the consent (e.g., "medical_research")
            duration_seconds: How long the consent is valid (in seconds)
            citizen_signature: Ed25519 signature over the entry hash (base64)
            constraints: Optional constraints (e.g., {"no_derivatives": True})
        
        Returns:
            ConsentEntry object
        """
        data = {
            'citizen_did': citizen_did,
            'scope': scope,
            'purpose': purpose,
            'duration_seconds': duration_seconds,
            'citizen_signature': citizen_signature
        }
        if constraints:
            data['constraints'] = constraints
            
        response = self._request('POST', '/consent/grant', json=data)
        return ConsentEntry.from_dict(response)
    
    def revoke_consent(
        self,
        citizen_did: str,
        entry_id_to_revoke: str,
        citizen_signature: str
    ) -> ConsentEntry:
        """
        Revoke a previously granted consent.
        
        Args:
            citizen_did: Citizen's DID
            entry_id_to_revoke: The entry ID to revoke
            citizen_signature: Ed25519 signature over the revocation
        
        Returns:
            ConsentEntry object for the revocation
        """
        data = {
            'citizen_did': citizen_did,
            'entry_id_to_revoke': entry_id_to_revoke,
            'citizen_signature': citizen_signature
        }
        response = self._request('POST', '/consent/revoke', json=data)
        return ConsentEntry.from_dict(response)
    
    def verify_consent_entry(self, entry_id: str) -> Dict[str, Any]:
        """Verify a consent entry's signature"""
        return self._request('GET', f'/consent/verify/{entry_id}')
    
    def verify_citizen_chain(self, did: str) -> Dict[str, Any]:
        """
        Verify the hash chain for a citizen.
        
        Returns:
            Dict with 'valid', 'entry_count', and 'invalid_entries' keys
        """
        return self._request('GET', f'/verify/chain/{did}')
    
    def check_access(
        self,
        citizen_did: str,
        resource: str,
        purpose: str
    ) -> Dict[str, Any]:
        """
        Check if a specific access is consented.
        
        Args:
            citizen_did: Citizen's DID
            resource: Resource to access (e.g., "health_data")
            purpose: Purpose of access (e.g., "medical_research")
        
        Returns:
            Dict with 'consented' boolean
        """
        data = {
            'citizen_did': citizen_did,
            'resource': resource,
            'purpose': purpose
        }
        return self._request('POST', '/verify/access', json=data)
    
    def get_hsk_proofs(self, did: str) -> HSKProofs:
        """
        Get HSK proof package for a citizen.
        
        This package can be submitted to the HSK verifier as ConsentLedger proof.
        
        Args:
            did: Citizen's DID
        
        Returns:
            HSKProofs object containing all consent entries
        """
        response = self._request('GET', f'/hsk/proofs/{did}')
        return HSKProofs.from_dict(response)


# Example usage
if __name__ == '__main__':
    import sys
    
    print("HSK Python Client Example")
    print("=" * 50)
    
    # HSK Verifier client example
    try:
        hsk = HSKClient('http://localhost:8081')
        
        print("\n1. HSK Verifier Health Check:")
        health = hsk.health_check()
        print(f"   Status: {health.get('status', 'unknown')}")
        print(f"   Key ID: {health.get('key_id', 'unknown')}")
        
        print("\n2. Create Challenge:")
        challenge = hsk.challenge('test-system', timeout_hours=24)
        print(f"   Request ID: {challenge.request_id}")
        print(f"   Deadline: {challenge.deadline}")
        print(f"   Requested Proofs: {[p.value for p in challenge.requested_proofs]}")
        
    except HSKError as e:
        print(f"   Error: {e}")
    
    # Consent Ledger client example
    try:
        consent = ConsentLedgerClient('http://localhost:8080')
        
        print("\n3. Consent Ledger Health Check:")
        health = consent.health_check()
        print(f"   Status: {health.get('status', 'unknown')}")
        
    except HSKError as e:
        print(f"   Error: {e}")
    
    print("\n" + "=" * 50)
    print("Example complete!")
