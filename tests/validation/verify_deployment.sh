#!/bin/bash
# Deployment Verification Script
# Verifies that all components are properly deployed

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

FAILED=0
PASSED=0

echo "=========================================="
echo "HSK Platform Deployment Verification"
echo "=========================================="
echo ""

if ! command -v kubectl &> /dev/null; then
    echo -e "${RED}kubectl not found. Cannot verify deployment.${NC}"
    exit 1
fi

# Check if we have cluster access
if ! kubectl cluster-info > /dev/null 2>&1; then
    echo -e "${RED}Cannot connect to Kubernetes cluster.${NC}"
    exit 1
fi

echo -e "${BLUE}Connected to cluster:${NC}"
kubectl cluster-info | head -2
echo ""

# Function to check resource exists
check_resource() {
    local resource="$1"
    local namespace="$2"
    local name="$3"
    
    echo -n "Checking $resource/$name in $namespace ... "
    if kubectl get "$resource" -n "$namespace" "$name" > /dev/null 2>&1; then
        echo -e "${GREEN}✓${NC}"
        ((PASSED++))
        return 0
    else
        echo -e "${RED}✗${NC}"
        ((FAILED++))
        return 1
    fi
}

# Function to check deployment is ready
check_deployment() {
    local namespace="$1"
    local name="$2"
    
    echo -n "Checking deployment/$name in $namespace ... "
    if kubectl get deployment -n "$namespace" "$name" > /dev/null 2>&1; then
        ready=$(kubectl get deployment -n "$namespace" "$name" -o jsonpath='{.status.readyReplicas}' 2>/dev/null || echo "0")
        desired=$(kubectl get deployment -n "$namespace" "$name" -o jsonpath='{.spec.replicas}' 2>/dev/null || echo "0")
        
        if [ "$ready" = "$desired" ] && [ "$ready" != "0" ]; then
            echo -e "${GREEN}✓ ($ready/$desired ready)${NC}"
            ((PASSED++))
            return 0
        else
            echo -e "${YELLOW}⚠ ($ready/$desired ready)${NC}"
            return 1
        fi
    else
        echo -e "${RED}✗ (not found)${NC}"
        ((FAILED++))
        return 1
    fi
}

# 1. Core Namespace
echo -e "${BLUE}Core Namespace${NC}"
check_resource "namespace" "" "hsk-verifier" || true
echo ""

# 2. Core Deployments
echo -e "${BLUE}Core Deployments${NC}"
check_deployment "hsk-verifier" "hs-verifier" || true
check_deployment "hsk-verifier" "transparency-log" || true
check_deployment "hsk-verifier" "did-server" || true
echo ""

# 3. Database
echo -e "${BLUE}Database${NC}"
check_deployment "hsk-verifier" "transparency-db" || true
check_resource "service" "hsk-verifier" "transparency-db" || true
echo ""

# 4. Vault
echo -e "${BLUE}Vault${NC}"
check_resource "namespace" "" "vault" || true
check_resource "statefulset" "vault" "vault" || true
check_resource "service" "vault" "vault" || true
echo ""

# 5. Istio
echo -e "${BLUE}Istio${NC}"
check_resource "namespace" "" "istio-system" || true
check_deployment "istio-system" "istiod" || true
check_deployment "istio-system" "istio-ingressgateway" || true
echo ""

# 6. Monitoring
echo -e "${BLUE}Monitoring${NC}"
check_resource "namespace" "" "monitoring" || true
check_deployment "monitoring" "prometheus" || true
check_deployment "monitoring" "grafana" || true
check_deployment "monitoring" "alertmanager" || true
echo ""

# 7. ConfigMaps and Secrets
echo -e "${BLUE}Configuration${NC}"
check_resource "configmap" "hsk-verifier" "verifier-config" || true
check_resource "configmap" "hsk-verifier" "database-config" || true
check_resource "secret" "hsk-verifier" "database-credentials" || true
echo ""

# 8. Services
echo -e "${BLUE}Services${NC}"
check_resource "service" "hsk-verifier" "hs-verifier" || true
check_resource "service" "hsk-verifier" "transparency-log" || true
check_resource "service" "hsk-verifier" "did-server" || true
echo ""

# 9. Ingress
echo -e "${BLUE}Ingress${NC}"
check_resource "ingress" "hsk-verifier" "hs-verifier-ingress" || true
echo ""

# 10. Network Policies
echo -e "${BLUE}Network Policies${NC}"
check_resource "networkpolicy" "hsk-verifier" "default-deny" || true
check_resource "networkpolicy" "hsk-verifier" "allow-verifier" || true
echo ""

# 11. RBAC
echo -e "${BLUE}RBAC${NC}"
check_resource "serviceaccount" "hsk-verifier" "hs-verifier" || true
check_resource "role" "hsk-verifier" "hs-verifier-role" || true
check_resource "rolebinding" "hsk-verifier" "hs-verifier-binding" || true
echo ""

# 12. CronJobs
echo -e "${BLUE}CronJobs${NC}"
check_resource "cronjob" "hsk-verifier" "database-backup-hourly" || true
check_resource "cronjob" "hsk-verifier" "database-backup-daily" || true
check_resource "cronjob" "vault" "secret-rotation" || true
echo ""

# 13. HPA
echo -e "${BLUE}Horizontal Pod Autoscaler${NC}"
check_resource "hpa" "hsk-verifier" "hs-verifier" || true
echo ""

# Summary
echo "=========================================="
echo "Deployment Verification Summary"
echo "=========================================="
echo -e "${GREEN}Passed: $PASSED${NC}"
echo -e "${RED}Failed: $FAILED${NC}"
echo ""

# Show pod status
echo -e "${BLUE}Pod Status${NC}"
kubectl get pods -n hsk-verifier 2>/dev/null || echo "No pods in hsk-verifier namespace"
echo ""

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All components verified!${NC}"
    exit 0
else
    echo -e "${RED}Some components are missing or not ready.${NC}"
    exit 1
fi
