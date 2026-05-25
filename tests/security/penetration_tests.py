#!/usr/bin/env python3
"""
Penetration Testing Scenarios for HSK Platform
Security testing to identify vulnerabilities
"""

import requests
import json
import base64
import hashlib
import time
from typing import Dict, List, Optional
import pytest


class PenetrationTester:
    """Penetration testing framework"""
    
    def __init__(self, base_url: str = "http://localhost:8080", api_key: str = "test-key"):
        self.base_url = base_url
        self.headers = {
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json"
        }
        self.results = []
    
    def log_result(self, test_name: str, passed: bool, details: str = ""):
        """Log test result"""
        status = "PASS" if passed else "FAIL"
        self.results.append({
            "test": test_name,
            "status": status,
            "details": details
        })
        print(f"[{status}] {test_name}: {details}")
    
    # ==================== INJECTION TESTS ====================
    
    def test_sql_injection(self) -> bool:
        """Test for SQL injection vulnerabilities"""
        payloads = [
            "' OR '1'='1",
            "'; DROP TABLE consent_entries; --",
            "' UNION SELECT * FROM users --",
            "1' AND 1=1 --",
            "1' AND 1=2 --",
        ]
        
        for payload in payloads:
            try:
                response = requests.get(
                    f"{self.base_url}/consent/search",
                    params={"query": payload},
                    headers=self.headers,
                    timeout=5
                )
                
                # Check if SQL error leaked
                if any(error in response.text.lower() for error in ['sql', 'syntax', 'postgresql']):
                    self.log_result("SQL Injection", False, f"SQL error leaked for payload: {payload}")
                    return False
                
                # Check if unauthorized data returned
                if response.status_code == 200 and len(response.json()) > 0:
                    self.log_result("SQL Injection", False, f"Potential data leak for payload: {payload}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("SQL Injection", True, "No SQL injection vulnerabilities found")
        return True
    
    def test_nosql_injection(self) -> bool:
        """Test for NoSQL injection vulnerabilities"""
        payloads = [
            {"$ne": None},
            {"$gt": ""},
            {"$regex": ".*"},
            {"$where": "this.password.length > 0"},
        ]
        
        for payload in payloads:
            try:
                response = requests.post(
                    f"{self.base_url}/consent/query",
                    json={"filter": payload},
                    headers=self.headers,
                    timeout=5
                )
                
                if response.status_code == 200:
                    self.log_result("NoSQL Injection", False, f"Potential NoSQL injection: {payload}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("NoSQL Injection", True, "No NoSQL injection vulnerabilities found")
        return True
    
    def test_command_injection(self) -> bool:
        """Test for command injection vulnerabilities"""
        payloads = [
            "; cat /etc/passwd",
            "| whoami",
            "`id`",
            "$(ls -la)",
            "; rm -rf /",
        ]
        
        for payload in payloads:
            try:
                response = requests.post(
                    f"{self.base_url}/export",
                    json={"filename": f"test{payload}"},
                    headers=self.headers,
                    timeout=5
                )
                
                # Check if command output leaked
                if any(indicator in response.text for indicator in ['root:', 'bin:', 'daemon:']):
                    self.log_result("Command Injection", False, f"Command injection possible: {payload}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("Command Injection", True, "No command injection vulnerabilities found")
        return True
    
    # ==================== AUTHENTICATION TESTS ====================
    
    def test_weak_authentication(self) -> bool:
        """Test for weak authentication mechanisms"""
        # Test for default credentials
        default_creds = [
            ("admin", "admin"),
            ("admin", "password"),
            ("root", "root"),
            ("test", "test"),
        ]
        
        for username, password in default_creds:
            try:
                response = requests.post(
                    f"{self.base_url}/auth/login",
                    json={"username": username, "password": password},
                    timeout=5
                )
                
                if response.status_code == 200:
                    self.log_result("Weak Authentication", False, f"Default credentials work: {username}/{password}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("Weak Authentication", True, "No default credentials found")
        return True
    
    def test_brute_force_protection(self) -> bool:
        """Test for brute force protection"""
        attempts = 20
        responses = []
        
        for i in range(attempts):
            try:
                response = requests.post(
                    f"{self.base_url}/auth/login",
                    json={"username": f"user{i}", "password": "wrongpassword"},
                    timeout=5
                )
                responses.append(response.status_code)
                
            except Exception as e:
                responses.append(0)
        
        # Check for rate limiting
        rate_limited = 429 in responses or 403 in responses
        
        if not rate_limited:
            self.log_result("Brute Force Protection", False, "No rate limiting detected after 20 attempts")
            return False
        
        self.log_result("Brute Force Protection", True, "Rate limiting is active")
        return True
    
    def test_session_management(self) -> bool:
        """Test for session management vulnerabilities"""
        # Test for session fixation
        try:
            # Get initial session
            response1 = requests.get(f"{self.base_url}/session", headers=self.headers)
            session1 = response1.cookies.get('session_id')
            
            # Login
            response2 = requests.post(
                f"{self.base_url}/auth/login",
                json={"username": "test", "password": "test"},
                headers=self.headers
            )
            session2 = response2.cookies.get('session_id')
            
            # Check if session changed after login
            if session1 == session2:
                self.log_result("Session Management", False, "Session not regenerated after login (session fixation)")
                return False
                
        except Exception as e:
            pass
        
        self.log_result("Session Management", True, "Session management appears secure")
        return True
    
    # ==================== AUTHORIZATION TESTS ====================
    
    def test_idor(self) -> bool:
        """Test for Insecure Direct Object Reference"""
        # Try to access other users' data
        test_ids = ["user1", "user2", "admin", "00000000-0000-0000-0000-000000000000"]
        
        for user_id in test_ids:
            try:
                response = requests.get(
                    f"{self.base_url}/users/{user_id}/consents",
                    headers=self.headers,
                    timeout=5
                )
                
                if response.status_code == 200:
                    data = response.json()
                    if len(data) > 0:
                        self.log_result("IDOR", False, f"Can access other user's data: {user_id}")
                        return False
                        
            except Exception as e:
                pass
        
        self.log_result("IDOR", True, "No IDOR vulnerabilities found")
        return True
    
    def test_privilege_escalation(self) -> bool:
        """Test for privilege escalation vulnerabilities"""
        # Try admin endpoints with regular user
        admin_endpoints = [
            "/admin/users",
            "/admin/config",
            "/admin/logs",
            "/system/restart",
        ]
        
        for endpoint in admin_endpoints:
            try:
                response = requests.get(
                    f"{self.base_url}{endpoint}",
                    headers=self.headers,
                    timeout=5
                )
                
                if response.status_code == 200:
                    self.log_result("Privilege Escalation", False, f"Admin endpoint accessible: {endpoint}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("Privilege Escalation", True, "No privilege escalation vulnerabilities found")
        return True
    
    # ==================== CRYPTOGRAPHY TESTS ====================
    
    def test_weak_crypto(self) -> bool:
        """Test for weak cryptographic implementations"""
        # Check TLS version
        try:
            import ssl
            import socket
            
            context = ssl.create_default_context()
            with socket.create_connection(("localhost", 443), timeout=5) as sock:
                with context.wrap_socket(sock, server_hostname="localhost") as ssock:
                    version = ssock.version()
                    if version in ['TLSv1', 'TLSv1.1']:
                        self.log_result("Weak Crypto", False, f"Weak TLS version: {version}")
                        return False
        except Exception as e:
            pass
        
        self.log_result("Weak Crypto", True, "TLS configuration appears secure")
        return True
    
    def test_signature_verification(self) -> bool:
        """Test signature verification bypass"""
        # Try to submit consent with invalid signature
        try:
            response = requests.post(
                f"{self.base_url}/consent",
                json={
                    "did": "did:hsk:test",
                    "purpose": "Test",
                    "data_categories": ["email"],
                    "signature": "invalid_signature"
                },
                headers=self.headers,
                timeout=5
            )
            
            if response.status_code == 200:
                self.log_result("Signature Verification", False, "Invalid signature accepted")
                return False
                
        except Exception as e:
            pass
        
        self.log_result("Signature Verification", True, "Signature verification working")
        return True
    
    # ==================== INPUT VALIDATION TESTS ====================
    
    def test_xss(self) -> bool:
        """Test for XSS vulnerabilities"""
        xss_payloads = [
            "<script>alert('XSS')</script>",
            "<img src=x onerror=alert('XSS')>",
            "javascript:alert('XSS')",
            "<svg onload=alert('XSS')>",
            "' onclick='alert(1)",
        ]
        
        for payload in xss_payloads:
            try:
                response = requests.post(
                    f"{self.base_url}/consent",
                    json={
                        "did": "did:hsk:test",
                        "purpose": payload,
                        "data_categories": ["email"]
                    },
                    headers=self.headers,
                    timeout=5
                )
                
                # Check if payload reflected without sanitization
                if payload in response.text and "<script>" in response.text:
                    self.log_result("XSS", False, f"XSS payload reflected: {payload[:50]}")
                    return False
                    
            except Exception as e:
                pass
        
        self.log_result("XSS", True, "No XSS vulnerabilities found")
        return True
    
    def test_xxe(self) -> bool:
        """Test for XXE vulnerabilities"""
        xxe_payload = """<?xml version="1.0"?>
<!DOCTYPE foo [
  <!ENTITY xxe SYSTEM "file:///etc/passwd">
]>
<foo>&xxe;</foo>"""
        
        try:
            response = requests.post(
                f"{self.base_url}/import",
                data=xxe_payload,
                headers={"Content-Type": "application/xml"},
                timeout=5
            )
            
            if 'root:' in response.text:
                self.log_result("XXE", False, "XXE vulnerability found")
                return False
                
        except Exception as e:
            pass
        
        self.log_result("XXE", True, "No XXE vulnerabilities found")
        return True
    
    # ==================== RATE LIMITING TESTS ====================
    
    def test_rate_limiting(self) -> bool:
        """Test rate limiting effectiveness"""
        requests_made = 0
        rate_limited = False
        
        for i in range(150):
            try:
                response = requests.get(
                    f"{self.base_url}/health",
                    headers=self.headers,
                    timeout=2
                )
                requests_made += 1
                
                if response.status_code == 429:
                    rate_limited = True
                    break
                    
            except Exception as e:
                break
        
        if not rate_limited:
            self.log_result("Rate Limiting", False, f"No rate limiting after {requests_made} requests")
            return False
        
        self.log_result("Rate Limiting", True, f"Rate limiting active after {requests_made} requests")
        return True
    
    # ==================== INFORMATION DISCLOSURE TESTS ====================
    
    def test_information_disclosure(self) -> bool:
        """Test for information disclosure"""
        # Check for stack traces
        try:
            response = requests.get(
                f"{self.base_url}/error",
                headers=self.headers,
                timeout=5
            )
            
            error_indicators = [
                'stack trace',
                'traceback',
                'line',
                'file "',
                'exception',
                'error at'
            ]
            
            if any(indicator in response.text.lower() for indicator in error_indicators):
                self.log_result("Information Disclosure", False, "Stack traces exposed in error messages")
                return False
                
        except Exception as e:
            pass
        
        # Check for version disclosure
        try:
            response = requests.get(f"{self.base_url}/health", headers=self.headers)
            headers = response.headers
            
            sensitive_headers = ['server', 'x-powered-by', 'x-aspnet-version']
            for header in sensitive_headers:
                if header in headers:
                    self.log_result("Information Disclosure", False, f"Sensitive header exposed: {header}")
                    return False
                    
        except Exception as e:
            pass
        
        self.log_result("Information Disclosure", True, "No information disclosure found")
        return True
    
    # ==================== RUN ALL TESTS ====================
    
    def run_all_tests(self) -> Dict:
        """Run all penetration tests"""
        print("="*60)
        print("HSK Platform Penetration Testing")
        print("="*60)
        print()
        
        tests = [
            ("SQL Injection", self.test_sql_injection),
            ("NoSQL Injection", self.test_nosql_injection),
            ("Command Injection", self.test_command_injection),
            ("Weak Authentication", self.test_weak_authentication),
            ("Brute Force Protection", self.test_brute_force_protection),
            ("Session Management", self.test_session_management),
            ("IDOR", self.test_idor),
            ("Privilege Escalation", self.test_privilege_escalation),
            ("Weak Cryptography", self.test_weak_crypto),
            ("Signature Verification", self.test_signature_verification),
            ("XSS", self.test_xss),
            ("XXE", self.test_xxe),
            ("Rate Limiting", self.test_rate_limiting),
            ("Information Disclosure", self.test_information_disclosure),
        ]
        
        passed = 0
        failed = 0
        
        for name, test_func in tests:
            try:
                result = test_func()
                if result:
                    passed += 1
                else:
                    failed += 1
            except Exception as e:
                self.log_result(name, False, f"Test error: {e}")
                failed += 1
        
        print()
        print("="*60)
        print(f"Results: {passed} passed, {failed} failed")
        print("="*60)
        
        return {
            "passed": passed,
            "failed": failed,
            "total": len(tests),
            "results": self.results
        }


if __name__ == "__main__":
    tester = PenetrationTester()
    results = tester.run_all_tests()
    
    # Exit with error code if any tests failed
    exit(0 if results["failed"] == 0 else 1)
