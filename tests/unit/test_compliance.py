#!/usr/bin/env python3
"""
Unit tests for Compliance Documentation
Tests SOC2 and GDPR compliance configurations
"""

import unittest
from pathlib import Path


class TestSOC2Compliance(unittest.TestCase):
    """Test SOC 2 Type II controls"""
    
    @classmethod
    def setUpClass(cls):
        cls.compliance_dir = Path(__file__).parent.parent.parent / "compliance-docs"
    
    def test_soc2_document_exists(self):
        """Verify SOC 2 document exists"""
        soc2_file = self.compliance_dir / "SOC2_CONTROLS.md"
        self.assertTrue(soc2_file.exists(), "SOC 2 document should exist")
    
    def test_soc2_cc6_controls(self):
        """Verify CC6 (Logical Access) controls are documented"""
        soc2_file = self.compliance_dir / "SOC2_CONTROLS.md"
        if soc2_file.exists():
            content = soc2_file.read_text()
            self.assertIn('CC6.1', content, "Should have CC6.1 controls")
            self.assertIn('CC6.6', content, "Should have encryption controls")
            self.assertIn('CC6.7', content, "Should have infrastructure controls")
    
    def test_soc2_cc7_controls(self):
        """Verify CC7 (Monitoring) controls are documented"""
        soc2_file = self.compliance_dir / "SOC2_CONTROLS.md"
        if soc2_file.exists():
            content = soc2_file.read_text()
            self.assertIn('CC7.1', content, "Should have security monitoring")
            self.assertIn('CC7.2', content, "Should have system monitoring")
    
    def test_soc2_cc8_controls(self):
        """Verify CC8 (Change Management) controls are documented"""
        soc2_file = self.compliance_dir / "SOC2_CONTROLS.md"
        if soc2_file.exists():
            content = soc2_file.read_text()
            self.assertIn('CC8.1', content, "Should have change management")
    
    def test_evidence_collection(self):
        """Verify evidence collection is documented"""
        soc2_file = self.compliance_dir / "SOC2_CONTROLS.md"
        if soc2_file.exists():
            content = soc2_file.read_text()
            self.assertIn('Evidence', content, "Should document evidence collection")
            self.assertIn('Audit Trail', content, "Should have audit trail")


class TestGDPRCompliance(unittest.TestCase):
    """Test GDPR compliance documentation"""
    
    def test_gdpr_document_exists(self):
        """Verify GDPR document exists"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        self.assertTrue(gdpr_file.exists(), "GDPR document should exist")
    
    def test_data_processing_records(self):
        """Verify Article 30 data processing records"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        if gdpr_file.exists():
            content = gdpr_file.read_text()
            self.assertIn('Article 30', content, "Should reference Article 30")
            self.assertIn('Processing Activities', content, "Should document processing activities")
    
    def test_data_subject_rights(self):
        """Verify data subject rights are documented"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        if gdpr_file.exists():
            content = gdpr_file.read_text()
            self.assertIn('Right to Access', content, "Should document Article 15")
            self.assertIn('Right to Erasure', content, "Should document Article 17")
            self.assertIn('Right to Data Portability', content, "Should document Article 20")
    
    def test_data_residency(self):
        """Verify data residency controls are documented"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        if gdpr_file.exists():
            content = gdpr_file.read_text()
            self.assertIn('Data Residency', content, "Should document data residency")
            self.assertIn('eu-west', content, "Should specify EU region")
    
    def test_breach_notification(self):
        """Verify breach notification procedures"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        if gdpr_file.exists():
            content = gdpr_file.read_text()
            self.assertIn('Breach Notification', content, "Should document breach procedures")
            self.assertIn('72 hours', content, "Should mention 72-hour notification")
    
    def test_retention_policy(self):
        """Verify data retention is documented"""
        gdpr_file = Path(__file__).parent.parent.parent / "compliance-docs" / "GDPR_COMPLIANCE.md"
        if gdpr_file.exists():
            content = gdpr_file.read_text()
            self.assertIn('Retention', content, "Should document retention")
            self.assertIn('7 years', content, "Should specify retention periods")


if __name__ == '__main__':
    unittest.main(verbosity=2)
