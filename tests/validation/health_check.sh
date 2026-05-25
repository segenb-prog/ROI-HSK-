#!/bin/bash
# Comprehensive Health Check for HSK Platform
# Checks all services and components

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Configuration
API_URL="${API_URL:-http://localhost:8080}"
AUTH_URL="${AUTH_URL:-http://localhost:8081}"
PROMETHEUS_URL="${PROMETHEUS_URL:-http://localhost:9090}"
GRAFANA_URL="${GRAFANA_URL:-http://localhost:3000}"

FAILED=0
PASSED=0

check_http() {
    local name="$1"
    local url="$2"
    local expected_code="${3:-200}"
    
    echo -n "Checking $name ... "
    if response=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 "$url" 2>/dev/null); then
        if [ "$response" = "$expected_code" ]; then
            echo -e "${GREEN}✓ ($response)${NC}"
            ((PASSED++))
            return 0
        else
            echo -e "${RED}✗ (expected $expected_code, got $response)${NC}"
            ((FAILED++))
            return 1
        fi
    else
        echo -e "${RED}✗ (connection failed)${NC}"
        ((FAILED++))
        return 1
    fi
}

check_k8s_pod() {
    local namespace="$1"
    local label="$2"
    
    echo -n "Checking pods in $namespace with label $label ... "
    if kubectl get pods -n "$namespace" -l "$label" --field-selector=status.phase=Running 2>/dev/null | grep -q Running; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((FAILED++))
    fi
}

echo "=========================================="
echo "HSK Platform Health Check"
echo "=========================================="
echo ""

# 1. Core API Health
echo -e "${BLUE}Core Services${NC}"
check_http "API Health" "$API_URL/health"
check_http "API Ready" "$API_URL/ready"
check_http "Metrics" "$API_URL/metrics"
echo ""

# 2. Authentication Service
echo -e "${BLUE}Authentication Service${NC}"
check_http "Auth Health" "$AUTH_URL/health" || true
echo ""

# 3. Kubernetes Pods (if kubectl is available)
if command -v kubectl &> /dev/null; then
    echo -e "${BLUE}Kubernetes Pods${NC}"
    check_k8s_pod "hsk-verifier" "app=hs-verifier" || true
    check_k8s_pod "hsk-verifier" "app=transparency-log" || true
    check_k8s_pod "hsk-verifier" "app=database" || true
    check_k8s_pod "vault" "app=vault" || true
    check_k8s_pod "istio-system" "app=istio-ingressgateway" || true
    echo ""
fi

# 4. Monitoring
echo -e "${BLUE}Monitoring${NC}"
check_http "Prometheus" "$PROMETHEUS_URL/-/healthy" || true
check_http "Grafana" "$GRAFANA_URL/api/health" || true
echo ""

# 5. Database Connectivity (if psql is available)
if command -v psql &> /dev/null; then
    echo -e "${BLUE}Database${NC}"
    echo -n "Checking PostgreSQL ... "
    if PGPASSWORD="${DB_PASSWORD:-password}" psql -h "${DB_HOST:-localhost}" -U "${DB_USER:-postgres}" -d "${DB_NAME:-consent_ledger}" -c "SELECT 1" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((FAILED++))
    fi
    echo ""
fi

# 6. Redis (if redis-cli is available)
if command -v redis-cli &> /dev/null; then
    echo -e "${BLUE}Redis Cache${NC}"
    echo -n "Checking Redis ... "
    if redis-cli -h "${REDIS_HOST:-localhost}" -p "${REDIS_PORT:-6379}" ping > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED++))
    else
        echo -e "${RED}✗${NC}"
        ((FAILED++))
    fi
    echo ""
fi

# 7. Vault (if available)
echo -e "${BLUE}Vault${NC}"
check_http "Vault Health" "https://vault.hskernel.io/v1/sys/health" "200" || true
echo ""

# 8. Critical Endpoints
echo -e "${BLUE}Critical Endpoints${NC}"
check_http "Challenge Endpoint" "$API_URL/challenge" "405" || true  # Expecting 405 for GET
check_http "Certificates Endpoint" "$API_URL/certificates" "200" || true
check_http "Transparency Log" "$API_URL/transparency/health" "200" || true
echo ""

# Summary
echo "=========================================="
echo "Health Check Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All health checks passed!${NC}"
    exit 0
else
    echo -e "${RED}Some health checks failed.${NC}"
    exit 1
fi
