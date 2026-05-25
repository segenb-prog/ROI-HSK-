#!/usr/bin/env python3
"""
Unit tests for Vault Integration
Tests Vault deployment, policies, and secret rotation
"""

import unittest
import yaml
from pathlib import Path


class TestVaultDeployment(unittest.TestCase):
    """Test Vault deployment configuration"""
    
    @classmethod
    def setUpClass(cls):
        cls.vault_dir = Path(__file__).parent.parent.parent / "vault-integration"
    
    def test_vault_deployment_exists(self):
        """Verify Vault deployment file exists"""
        deployment_file = self.vault_dir / "vault-deployment.yaml"
        self.assertTrue(deployment_file.exists(), "Vault deployment file should exist")
    
    def test_vault_ha_configuration(self):
        """Verify Vault is configured for high availability"""
        deployment_file = self.vault_dir / "vault-deployment.yaml"
        if deployment_file.exists():
            with open(deployment_file) as f:
                docs = list(yaml.safe_load_all(f))
            
            # Find StatefulSet
            statefulsets = [d for d in docs if d and d.get('kind') == 'StatefulSet']
            self.assertTrue(len(statefulsets) > 0, "Should have Vault StatefulSet")
            
            for sts in statefulsets:
                self.assertEqual(sts['spec'].get('replicas'), 3, "Vault should have 3 replicas")
    
    def test_vault_auto_unseal(self):
        """Verify Vault uses auto-unseal"""
        deployment_file = self.vault_dir / "vault-deployment.yaml"
        if deployment_file.exists():
            content = deployment_file.read_text()
            self.assertIn('awskms', content, "Vault should use AWS KMS auto-unseal")
            self.assertIn('VAULT_SEAL_TYPE', content, "Should specify seal type")
    
    def test_vault_tls(self):
        """Verify Vault uses TLS"""
        deployment_file = self.vault_dir / "vault-deployment.yaml"
        if deployment_file.exists():
            content = deployment_file.read_text()
            self.assertIn('tls_cert_file', content, "Vault should use TLS")
            self.assertIn('tls_min_version', content, "Should specify TLS version")
    
    def test_vault_policies_exist(self):
        """Verify Vault policies file exists"""
        policies_file = self.vault_dir / "vault-policies.hcl"
        self.assertTrue(policies_file.exists(), "Vault policies file should exist")
    
    def test_vault_policies_structure(self):
        """Verify Vault policies have correct structure"""
        policies_file = self.vault_dir / "vault-policies.hcl"
        if policies_file.exists():
            content = policies_file.read_text()
            # Check for policy paths
            self.assertIn('path', content, "Policies should define paths")
            self.assertIn('capabilities', content, "Policies should define capabilities")
    
    def test_secret_rotation(self):
        """Verify secret rotation is configured"""
        rotation_file = self.vault_dir / "secret-rotation.yaml"
        self.assertTrue(rotation_file.exists(), "Secret rotation file should exist")
        
        if rotation_file.exists():
            with open(rotation_file) as f:
                content = rotation_file.read_text()
            
            # Check for rotation
            self.assertIn('secret-rotation', content, "Should have rotation CronJob")
            self.assertIn('rotate', content, "Should rotate keys")


class TestVaultDatabaseConfig(unittest.TestCase):
    """Test Vault database secrets engine configuration"""
    
    def test_database_config_exists(self):
        """Verify database config file exists"""
        config_file = Path(__file__).parent.parent.parent / "vault-integration" / "vault-database-config.yaml"
        self.assertTrue(config_file.exists(), "Database config file should exist")
    
    def test_dynamic_credentials(self):
        """Verify dynamic database credentials are configured"""
        config_file = Path(__file__).parent.parent.parent / "vault-integration" / "vault-database-config.yaml"
        if config_file.exists():
            content = config_file.read_text()
            self.assertIn('database/roles', content, "Should configure database roles")
            self.assertIn('creation_statements', content, "Should have creation statements")
            self.assertIn('default_ttl', content, "Should have TTL for credentials")


if __name__ == '__main__':
    unittest.main(verbosity=2)
