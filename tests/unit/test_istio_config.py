#!/usr/bin/env python3
"""
Unit tests for Istio Configuration
Tests service mesh, mTLS, circuit breakers, and tracing
"""

import unittest
import yaml
from pathlib import Path


class TestIstioInstallation(unittest.TestCase):
    """Test Istio installation configuration"""
    
    @classmethod
    def setUpClass(cls):
        cls.istio_dir = Path(__file__).parent.parent.parent / "istio-config"
    
    def test_istio_install_exists(self):
        """Verify Istio installation file exists"""
        install_file = self.istio_dir / "istio-install.yaml"
        self.assertTrue(install_file.exists(), "Istio install file should exist")
    
    def test_mtls_enabled(self):
        """Verify mTLS is enabled"""
        install_file = self.istio_dir / "istio-install.yaml"
        if install_file.exists():
            with open(install_file) as f:
                content = install_file.read_text()
            
            self.assertIn('enableAutoMtls', content, "Auto mTLS should be enabled")
            self.assertIn('mode: STRICT', content, "STRICT mTLS should be configured")
    
    def test_gateway_configured(self):
        """Verify Gateway is configured"""
        install_file = self.istio_dir / "istio-install.yaml"
        if install_file.exists():
            with open(install_file) as f:
                docs = list(yaml.safe_load_all(f))
            
            gateways = [d for d in docs if d and d.get('kind') == 'Gateway']
            self.assertTrue(len(gateways) > 0, "Should have Gateway configured")
            
            for gw in gateways:
                self.assertIn('spec', gw)
                self.assertIn('servers', gw['spec'])
    
    def test_tls_version(self):
        """Verify TLS 1.3 is used"""
        install_file = self.istio_dir / "istio-install.yaml"
        if install_file.exists():
            content = install_file.read_text()
            self.assertIn('TLSV1_3', content, "Should use TLS 1.3")
            self.assertIn('minProtocolVersion', content, "Should specify min TLS version")


class TestCircuitBreakers(unittest.TestCase):
    """Test circuit breaker configuration"""
    
    def test_circuit_breakers_exist(self):
        """Verify circuit breaker file exists"""
        cb_file = Path(__file__).parent.parent.parent / "istio-config" / "circuit-breakers.yaml"
        self.assertTrue(cb_file.exists(), "Circuit breaker file should exist")
    
    def test_circuit_breaker_settings(self):
        """Verify circuit breaker settings are configured"""
        cb_file = Path(__file__).parent.parent.parent / "istio-config" / "circuit-breakers.yaml"
        if cb_file.exists():
            with open(cb_file) as f:
                content = cb_file.read_text()
            
            self.assertIn('outlierDetection', content, "Should have outlier detection")
            self.assertIn('consecutive5xxErrors', content, "Should have 5xx error threshold")
            self.assertIn('baseEjectionTime', content, "Should have ejection time")
    
    def test_retry_policy(self):
        """Verify retry policy is configured"""
        cb_file = Path(__file__).parent.parent.parent / "istio-config" / "circuit-breakers.yaml"
        if cb_file.exists():
            content = cb_file.read_text()
            self.assertIn('retries', content, "Should have retry configuration")
            self.assertIn('perTryTimeout', content, "Should have per-try timeout")


class TestDistributedTracing(unittest.TestCase):
    """Test distributed tracing configuration"""
    
    def test_tracing_exists(self):
        """Verify tracing file exists"""
        tracing_file = Path(__file__).parent.parent.parent / "istio-config" / "distributed-tracing.yaml"
        self.assertTrue(tracing_file.exists(), "Tracing file should exist")
    
    def test_jaeger_configured(self):
        """Verify Jaeger is configured"""
        tracing_file = Path(__file__).parent.parent.parent / "istio-config" / "distributed-tracing.yaml"
        if tracing_file.exists():
            with open(tracing_file) as f:
                content = tracing_file.read_text()
            
            self.assertIn('jaeger', content, "Should have Jaeger configured")
            self.assertIn('otel-collector', content, "Should have OTel collector")
    
    def test_sampling_configured(self):
        """Verify trace sampling is configured"""
        install_file = Path(__file__).parent.parent.parent / "istio-config" / "istio-install.yaml"
        if install_file.exists():
            content = install_file.read_text()
            self.assertIn('sampling', content, "Should have sampling configured")


if __name__ == '__main__':
    unittest.main(verbosity=2)
