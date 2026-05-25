#!/bin/bash
# HSK Canary Deployment Script
# Performs gradual rollout with automatic rollback on failure

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-hsk-verifier}"
DEPLOYMENT="${DEPLOYMENT:-hs-verifier}"
NEW_IMAGE="${1:-}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_step() {
    echo -e "${BLUE}[STEP]${NC} $1"
}

usage() {
    echo "Usage: $0 <new-image-tag>"
    echo ""
    echo "Example: $0 hskernel/hs-verifier:v0.2.0"
    exit 1
}

# Validate input
if [ -z "$NEW_IMAGE" ]; then
    log_error "No image tag specified"
    usage
fi

# Check prerequisites
check_prerequisites() {
    log_step "Checking prerequisites..."
    
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl not found"
        exit 1
    fi
    
    if ! kubectl get namespace "$NAMESPACE" &> /dev/null; then
        log_error "Namespace $NAMESPACE not found"
        exit 1
    fi
    
    if ! kubectl get deployment "$DEPLOYMENT" -n "$NAMESPACE" &> /dev/null; then
        log_error "Deployment $DEPLOYMENT not found in namespace $NAMESPACE"
        exit 1
    fi
    
    # Check if Flagger is installed
    if ! kubectl get canary "$DEPLOYMENT" -n "$NAMESPACE" &> /dev/null; then
        log_warn "Flagger Canary resource not found. Creating..."
        kubectl apply -f k8s-deployments/canary/flagger-canary.yaml -n "$NAMESPACE"
    fi
    
    log_info "Prerequisites OK"
}

# Pre-deployment checks
pre_deployment_checks() {
    log_step "Running pre-deployment checks..."
    
    # Check current deployment health
    READY_REPLICAS=$(kubectl get deployment "$DEPLOYMENT" -n "$NAMESPACE" -o jsonpath='{.status.readyReplicas}')
    DESIRED_REPLICAS=$(kubectl get deployment "$DEPLOYMENT" -n "$NAMESPACE" -o jsonpath='{.spec.replicas}')
    
    if [ "$READY_REPLICAS" != "$DESIRED_REPLICAS" ]; then
        log_error "Current deployment is not healthy ($READY_REPLICAS/$DESIRED_REPLICAS ready)"
        exit 1
    fi
    
    # Run smoke tests
    log_info "Running smoke tests..."
    VERIFIER_POD=$(kubectl get pod -n "$NAMESPACE" -l app=hs-verifier -o jsonpath='{.items[0].metadata.name}')
    
    if ! kubectl exec -n "$NAMESPACE" "$VERIFIER_POD" -- curl -sf http://localhost:8080/health &> /dev/null; then
        log_error "Health check failed"
        exit 1
    fi
    
    log_info "Pre-deployment checks passed"
}

# Deploy new version
deploy_new_version() {
    log_step "Deploying new version: $NEW_IMAGE"
    
    # Update deployment image
    kubectl set image deployment/"$DEPLOYMENT" \
        verifier="$NEW_IMAGE" \
        -n "$NAMESPACE"
    
    # Wait for rollout to start
    sleep 5
    
    log_info "Deployment started. Monitoring canary progress..."
}

# Monitor canary progress
monitor_canary() {
    log_step "Monitoring canary deployment..."
    
    local max_attempts=60
    local attempt=0
    
    while [ $attempt -lt $max_attempts ]; do
        attempt=$((attempt + 1))
        
        # Get canary status
        CANARY_STATUS=$(kubectl get canary "$DEPLOYMENT" -n "$NAMESPACE" -o json 2>/dev/null || echo '{}')
        
        PHASE=$(echo "$CANARY_STATUS" | jq -r '.status.phase // "Unknown"')
        CANARY_WEIGHT=$(echo "$CANARY_STATUS" | jq -r '.status.canaryWeight // 0')
        FAILED_CHECKS=$(echo "$CANARY_STATUS" | jq -r '.status.failedChecks // 0')
        
        log_info "Phase: $PHASE | Canary Weight: ${CANARY_WEIGHT}% | Failed Checks: $FAILED_CHECKS"
        
        case "$PHASE" in
            "Succeeded")
                log_info "✓ Canary deployment succeeded!"
                return 0
                ;;
            "Failed")
                log_error "✗ Canary deployment failed!"
                log_error "Check logs: kubectl logs -n $NAMESPACE deployment/$DEPLOYMENT"
                return 1
                ;;
            "Progressing")
                # Continue monitoring
                ;;
            *)
                log_warn "Unknown phase: $PHASE"
                ;;
        esac
        
        sleep 10
    done
    
    log_error "Timeout waiting for canary deployment"
    return 1
}

# Rollback on failure
rollback() {
    log_error "Initiating rollback..."
    
    # Trigger Flagger rollback
    kubectl exec -n flagger deploy/flagger-loadtester -- \
        curl -X POST http://localhost:8080/rollback/hsk-verifier/hs-verifier
    
    # Wait for rollback to complete
    sleep 30
    
    # Verify rollback
    CURRENT_IMAGE=$(kubectl get deployment "$DEPLOYMENT" -n "$NAMESPACE" -o jsonpath='{.spec.template.spec.containers[0].image}')
    log_info "Rolled back to: $CURRENT_IMAGE"
}

# Post-deployment verification
post_deployment_verification() {
    log_step "Running post-deployment verification..."
    
    # Wait for deployment to be ready
    kubectl rollout status deployment/"$DEPLOYMENT" -n "$NAMESPACE" --timeout=300s
    
    # Run integration tests
    log_info "Running integration tests..."
    VERIFIER_POD=$(kubectl get pod -n "$NAMESPACE" -l app=hs-verifier -o jsonpath='{.items[0].metadata.name}')
    
    # Test challenge endpoint
    RESPONSE=$(kubectl exec -n "$NAMESPACE" "$VERIFIER_POD" -- \
        curl -sf -X POST http://localhost:8080/challenge \
        -H "Content-Type: application/json" \
        -d '{"system_id":"post-deploy-test","timeout_hours":1}')
    
    if [ -z "$RESPONSE" ]; then
        log_error "Post-deployment test failed"
        return 1
    fi
    
    log_info "✓ Post-deployment verification passed"
}

# Notify stakeholders
notify() {
    local status="$1"
    local message="$2"
    
    # Send to Slack if webhook configured
    if [ -n "${SLACK_WEBHOOK:-}" ]; then
        curl -sf -X POST "$SLACK_WEBHOOK" \
            -H "Content-Type: application/json" \
            -d "{\"text\":\"HSK Deployment: $status - $message\"}" > /dev/null || true
    fi
    
    # Send to PagerDuty if key configured
    if [ -n "${PAGERDUTY_KEY:-}" ]; then
        # Implementation depends on PagerDuty integration
        : # Placeholder
    fi
}

# Main execution
main() {
    log_info "Starting canary deployment of $DEPLOYMENT to $NEW_IMAGE"
    
    check_prerequisites
    pre_deployment_checks
    
    if deploy_new_version; then
        if monitor_canary; then
            post_deployment_verification
            log_info "✓ Deployment completed successfully!"
            notify "SUCCESS" "Deployed $DEPLOYMENT:$NEW_IMAGE"
        else
            log_error "✗ Deployment failed, rolling back..."
            rollback
            notify "FAILED" "Deployment of $DEPLOYMENT:$NEW_IMAGE failed and was rolled back"
            exit 1
        fi
    else
        log_error "✗ Deployment failed"
        notify "FAILED" "Deployment of $DEPLOYMENT:$NEW_IMAGE failed"
        exit 1
    fi
}

# Handle signals
trap 'log_error "Deployment interrupted"; exit 1' INT TERM

main "$@"
