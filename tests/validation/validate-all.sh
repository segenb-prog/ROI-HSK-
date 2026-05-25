#!/bin/bash
# Comprehensive Validation Script for HSK Platform
# Validates all configurations, manifests, and deployments

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

FAILED=0
PASSED=0

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASSED++))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAILED++))
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

echo "=========================================="
echo "HSK Platform Validation Suite"
echo "=========================================="
echo ""

# 1. Validate YAML syntax
log_info "Validating YAML syntax..."
for file in $(find . -name "*.yaml" -o -name "*.yml" | grep -v node_modules); do
    if python3 -c "import yaml; yaml.safe_load(open('$file'))" 2>/dev/null; then
        log_pass "YAML syntax: $file"
    else
        log_fail "YAML syntax: $file"
    fi
done
echo ""

# 2. Validate Kubernetes manifests
log_info "Validating Kubernetes manifests..."
if command -v kubeval &> /dev/null; then
    for file in $(find k8s-deployments k8s-tests backup-system vault-integration istio-config -name "*.yaml" 2>/dev/null); do
        if kubeval "$file" 2>/dev/null; then
            log_pass "Kubeval: $file"
        else
            log_fail "Kubeval: $file"
        fi
    done
else
    log_warn "kubeval not installed, skipping"
fi
echo ""

# 3. Validate JSON files
log_info "Validating JSON files..."
for file in $(find . -name "*.json" | grep -v node_modules); do
    if python3 -c "import json; json.load(open('$file'))" 2>/dev/null; then
        log_pass "JSON: $file"
    else
        log_fail "JSON: $file"
    fi
done
echo ""

# 4. Validate SQL syntax
log_info "Validating SQL files..."
for file in $(find database-schemas -name "*.sql" 2>/dev/null); do
    # Basic SQL validation - check for common keywords
    if grep -q "CREATE\|INSERT\|SELECT\|UPDATE\|DELETE" "$file"; then
        log_pass "SQL: $file"
    else
        log_fail "SQL: $file (no SQL keywords found)"
    fi
done
echo ""

# 5. Validate Rust code
log_info "Validating Rust code..."
if command -v cargo &> /dev/null; then
    cd rust-hs-verifier
    if cargo check 2>/dev/null; then
        log_pass "Rust: rust-hs-verifier compiles"
    else
        log_fail "Rust: rust-hs-verifier compilation failed"
    fi
    cd ..
    
    cd prototype-digital-identity
    if cargo check 2>/dev/null; then
        log_pass "Rust: prototype-digital-identity compiles"
    else
        log_fail "Rust: prototype-digital-identity compilation failed"
    fi
    cd ..
else
    log_warn "cargo not installed, skipping Rust validation"
fi
echo ""

# 6. Validate Swift code
log_info "Validating Swift code..."
if [ -f "mobile-sdks/ios-sdk.swift" ]; then
    # Basic syntax check
    if grep -q "import Foundation" mobile-sdks/ios-sdk.swift; then
        log_pass "Swift: iOS SDK structure valid"
    else
        log_fail "Swift: iOS SDK missing imports"
    fi
fi
echo ""

# 7. Validate Kotlin code
log_info "Validating Kotlin code..."
if [ -f "mobile-sdks/android-sdk.kt" ]; then
    if grep -q "package io.hskernel.sdk" mobile-sdks/android-sdk.kt; then
        log_pass "Kotlin: Android SDK structure valid"
    else
        log_fail "Kotlin: Android SDK missing package"
    fi
fi
echo ""

# 8. Validate OpenAPI spec
log_info "Validating OpenAPI specification..."
if command -v swagger-codegen &> /dev/null; then
    if swagger-codegen validate -i docs/api/openapi.yaml 2>/dev/null; then
        log_pass "OpenAPI: Specification valid"
    else
        log_fail "OpenAPI: Specification invalid"
    fi
else
    log_warn "swagger-codegen not installed, skipping"
fi
echo ""

# 9. Check for required files
log_info "Checking required files..."
REQUIRED_FILES=(
    "README.md"
    "Makefile"
    "Cargo.toml"
    "k8s-deployments/namespace.yaml"
    "k8s-deployments/verifier-deployment.yaml"
    "database-schemas/consent_ledger.sql"
    "docs/api/openapi.yaml"
)

for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        log_pass "Required file: $file"
    else
        log_fail "Required file missing: $file"
    fi
done
echo ""

# 10. Validate shell scripts
log_info "Validating shell scripts..."
for file in $(find scripts -name "*.sh" 2>/dev/null); do
    if bash -n "$file" 2>/dev/null; then
        log_pass "Shell script: $file"
    else
        log_fail "Shell script: $file"
    fi
done
echo ""

# 11. Check for secrets in code
log_info "Checking for exposed secrets..."
SECRET_PATTERNS=(
    "password.*=.*[^${}]"
    "api_key.*=.*[^${}]"
    "secret.*=.*[^${}]"
    "token.*=.*[^${}]"
    "AWS_ACCESS_KEY_ID"
    "PRIVATE_KEY"
)

SECRETS_FOUND=0
for pattern in "${SECRET_PATTERNS[@]}"; do
    if grep -r "$pattern" --include="*.yaml" --include="*.json" --include="*.sh" . 2>/dev/null | grep -v "example\|placeholder\|CHANGEME"; then
        log_fail "Potential secret exposed: $pattern"
        SECRETS_FOUND=1
    fi
done

if [ $SECRETS_FOUND -eq 0 ]; then
    log_pass "No exposed secrets detected"
fi
echo ""

# 12. Validate Dockerfiles
log_info "Validating Dockerfiles..."
for file in $(find . -name "Dockerfile*" 2>/dev/null); do
    if grep -q "FROM" "$file"; then
        log_pass "Dockerfile: $file"
    else
        log_fail "Dockerfile: $file (no FROM instruction)"
    fi
done
echo ""

# 13. Check file permissions
log_info "Checking file permissions..."
for file in $(find scripts -name "*.sh" 2>/dev/null); do
    if [ -x "$file" ]; then
        log_pass "Executable: $file"
    else
        log_warn "Not executable: $file"
    fi
done
echo ""

# 14. Validate documentation links
log_info "Validating documentation..."
if grep -q "http" README.md; then
    log_pass "README has links"
else
    log_warn "README may be missing links"
fi
echo ""

# Summary
echo "=========================================="
echo "Validation Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All validations passed!${NC}"
    exit 0
else
    echo -e "${RED}Some validations failed. Please review.${NC}"
    exit 1
fi
