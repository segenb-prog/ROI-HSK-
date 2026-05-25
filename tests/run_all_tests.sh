#!/bin/bash
# Comprehensive Test Runner for HSK Platform
# Runs all unit, integration, and validation tests

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

echo "=========================================="
echo "HSK Platform Test Suite"
echo "=========================================="
echo ""

TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

run_test_suite() {
    local name="$1"
    local command="$2"
    
    echo -e "${BLUE}Running: $name${NC}"
    if eval "$command" > /tmp/test_output_$$.log 2>&1; then
        echo -e "${GREEN}✓ $name passed${NC}"
        ((PASSED_TESTS++))
    else
        echo -e "${RED}✗ $name failed${NC}"
        cat /tmp/test_output_$$.log
        ((FAILED_TESTS++))
    fi
    ((TOTAL_TESTS++))
    echo ""
}

# 1. YAML Validation
run_test_suite "YAML Syntax Validation" "python3 tests/validation/validate_yaml.py"

# 2. Kubernetes Manifest Validation
if command -v kubeval &> /dev/null; then
    run_test_suite "Kubernetes Manifest Validation" "tests/validation/validate_k8s.sh"
else
    echo -e "${YELLOW}Skipping Kubernetes validation (kubeval not installed)${NC}"
fi

# 3. Unit Tests - Backup System
if command -v python3 &> /dev/null; then
    run_test_suite "Unit Tests: Backup System" "python3 -m pytest tests/unit/test_backup_system.py -v"
fi

# 4. Unit Tests - Vault Integration
if command -v python3 &> /dev/null; then
    run_test_suite "Unit Tests: Vault Integration" "python3 -m pytest tests/unit/test_vault_integration.py -v"
fi

# 5. Unit Tests - Istio Config
if command -v python3 &> /dev/null; then
    run_test_suite "Unit Tests: Istio Configuration" "python3 -m pytest tests/unit/test_istio_config.py -v"
fi

# 6. Unit Tests - Compliance
if command -v python3 &> /dev/null; then
    run_test_suite "Unit Tests: Compliance" "python3 -m pytest tests/unit/test_compliance.py -v"
fi

# 7. Unit Tests - Multi-Region
if command -v python3 &> /dev/null; then
    run_test_suite "Unit Tests: Multi-Region" "python3 -m pytest tests/unit/test_multi_region.py -v"
fi

# 8. Rust Tests
if command -v cargo &> /dev/null; then
    run_test_suite "Rust Unit Tests" "cargo test --workspace"
else
    echo -e "${YELLOW}Skipping Rust tests (cargo not installed)${NC}"
fi

# 9. Shell Script Validation
run_test_suite "Shell Script Validation" "bash -n scripts/*.sh"

# 10. Security Scan
run_test_suite "Security Scan (secrets check)" "tests/validation/check_secrets.sh"

# 11. Integration Tests (if platform is running)
if curl -s http://localhost:8080/health > /dev/null 2>&1; then
    run_test_suite "Integration Tests" "python3 -m pytest tests/integration/ -v"
else
    echo -e "${YELLOW}Skipping integration tests (platform not running)${NC}"
fi

# 12. Full Validation Suite
run_test_suite "Full Validation Suite" "bash tests/validation/validate-all.sh"

# Summary
echo "=========================================="
echo "Test Summary"
echo "=========================================="
echo -e "Total:  $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"
echo ""

# Cleanup
rm -f /tmp/test_output_$$.log

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed.${NC}"
    exit 1
fi
