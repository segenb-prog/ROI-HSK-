#!/bin/bash
# Validate Kubernetes manifests using kubeval

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

if ! command -v kubeval &> /dev/null; then
    echo -e "${YELLOW}kubeval not installed. Install with:${NC}"
    echo "  brew install kubeval  # macOS"
    echo "  or download from https://github.com/instrumenta/kubeval"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../.."

FAILED=0
PASSED=0

# Directories containing K8s manifests
K8S_DIRS=(
    "k8s-deployments"
    "k8s-tests"
    "k8s-tests/kuttl"
    "backup-system"
    "vault-integration"
    "istio-config"
    "multi-region"
    "auth-system"
    "rate-limiter"
    "data-lifecycle"
    "monitoring-advanced"
    "federation"
    "crypto-advanced"
    "ml-analytics"
    "admin-ui"
    "webhook-system"
    "performance"
    "dev-tools"
    "gitops"
)

echo "Validating Kubernetes manifests..."
echo ""

for dir in "${K8S_DIRS[@]}"; do
    if [ -d "$dir" ]; then
        while IFS= read -r -d '' file; do
            echo -n "Validating: $file ... "
            if kubeval "$file" --strict --ignore-missing-schemas 2>/dev/null; then
                echo -e "${GREEN}✓${NC}"
                ((PASSED++))
            else
                echo -e "${RED}✗${NC}"
                ((FAILED++))
            fi
        done < <(find "$dir" -name "*.yaml" -print0 2>/dev/null)
    fi
done

echo ""
echo "=========================================="
echo "Kubernetes Validation Summary"
echo "=========================================="
echo -e "Passed: $PASSED"
echo -e "Failed: $FAILED"

if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}All manifests are valid!${NC}"
    exit 0
else
    echo -e "${RED}Some manifests failed validation.${NC}"
    exit 1
fi
