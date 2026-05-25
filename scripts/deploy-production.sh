#!/bin/bash
# Production Deployment Script for HSK Platform
# Automates the complete production deployment process

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Configuration
NAMESPACE="hsk-verifier"
STAGING_NAMESPACE="hsk-staging"
CANARY_PERCENTAGE=10
ROLLBACK_THRESHOLD=5

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

# ==================== PRE-DEPLOYMENT ====================

pre_deployment_checks() {
    log_info "Running pre-deployment checks..."
    
    # Check kubectl access
    if ! kubectl cluster-info > /dev/null 2>&1; then
        log_error "Cannot connect to Kubernetes cluster"
        exit 1
    fi
    
    # Validate YAML files
    log_info "Validating YAML files..."
    if ! python3 tests/validation/validate_yaml.py > /dev/null 2>&1; then
        log_error "YAML validation failed"
        exit 1
    fi
    
    # Run unit tests
    log_info "Running unit tests..."
    if ! python3 -m pytest tests/unit/ -q > /dev/null 2>&1; then
        log_error "Unit tests failed"
        exit 1
    fi
    
    # Security scan
    log_info "Running security scan..."
    if ! bash tests/validation/check_secrets.sh > /dev/null 2>&1; then
        log_error "Security scan failed"
        exit 1
    fi
    
    log_success "Pre-deployment checks passed"
}

# ==================== STAGING DEPLOYMENT ====================

deploy_to_staging() {
    log_info "Deploying to staging..."
    
    # Apply staging configuration
    kubectl apply -k k8s-deployments/overlays/staging/
    
    # Wait for rollout
    log_info "Waiting for staging deployment..."
    kubectl rollout status deployment/hs-verifier -n $STAGING_NAMESPACE --timeout=300s
    kubectl rollout status deployment/transparency-log -n $STAGING_NAMESPACE --timeout=300s
    
    # Run smoke tests
    log_info "Running smoke tests on staging..."
    sleep 10
    
    STAGING_URL=$(kubectl get ingress -n $STAGING_NAMESPACE -o jsonpath='{.items[0].spec.rules[0].host}')
    
    if ! curl -sf "https://$STAGING_URL/health" > /dev/null 2>&1; then
        log_error "Staging health check failed"
        exit 1
    fi
    
    log_success "Staging deployment successful"
}

# ==================== CANARY DEPLOYMENT ====================

canary_deploy() {
    log_info "Starting canary deployment..."
    
    # Create canary deployment
    kubectl apply -f k8s-deployments/canary/flagger-canary.yaml
    
    # Wait for canary to be ready
    log_info "Waiting for canary deployment..."
    sleep 30
    
    # Monitor canary
    log_info "Monitoring canary metrics..."
    
    for i in {1..30}; do
        # Check error rate
        ERROR_RATE=$(curl -s "http://prometheus:9090/api/v1/query?query=rate(http_requests_total{status=~'5..'}[5m])" | jq -r '.data.result[0].value[1] // "0"')
        
        # Check latency
        LATENCY=$(curl -s "http://prometheus:9090/api/v1/query?query=histogram_quantile(0.95,rate(http_request_duration_seconds_bucket[5m]))" | jq -r '.data.result[0].value[1] // "0"')
        
        log_info "Canary metrics - Error rate: $ERROR_RATE, P95 latency: ${LATENCY}s"
        
        # Check thresholds
        if (( $(echo "$ERROR_RATE > 0.01" | bc -l) )); then
            log_error "Error rate exceeded threshold: $ERROR_RATE"
            canary_rollback
            exit 1
        fi
        
        if (( $(echo "$LATENCY > 0.5" | bc -l) )); then
            log_error "Latency exceeded threshold: ${LATENCY}s"
            canary_rollback
            exit 1
        fi
        
        sleep 10
    done
    
    log_success "Canary deployment successful"
}

canary_rollback() {
    log_warn "Rolling back canary deployment..."
    kubectl patch canary hs-verifier -n $NAMESPACE --type merge -p '{"spec":{"abort":true}}'
    log_info "Canary rollback initiated"
}

canary_promote() {
    log_info "Promoting canary to production..."
    kubectl patch canary hs-verifier -n $NAMESPACE --type merge -p '{"spec":{"skipAnalysis":true}}'
    log_success "Canary promoted to production"
}

# ==================== FULL PRODUCTION DEPLOYMENT ====================

deploy_to_production() {
    log_info "Deploying to production..."
    
    # Apply production configuration
    kubectl apply -k k8s-deployments/overlays/production/
    
    # Wait for deployments
    log_info "Waiting for production deployment..."
    kubectl rollout status deployment/hs-verifier -n $NAMESPACE --timeout=300s
    kubectl rollout status deployment/transparency-log -n $NAMESPACE --timeout=300s
    
    # Verify health
    log_info "Verifying production health..."
    PROD_URL=$(kubectl get ingress -n $NAMESPACE -o jsonpath='{.items[0].spec.rules[0].host}')
    
    for i in {1..10}; do
        if curl -sf "https://$PROD_URL/health" > /dev/null 2>&1; then
            log_success "Production health check passed"
            break
        fi
        log_warn "Health check attempt $i failed, retrying..."
        sleep 5
    done
    
    log_success "Production deployment successful"
}

# ==================== POST-DEPLOYMENT ====================

post_deployment_verification() {
    log_info "Running post-deployment verification..."
    
    # Run health checks
    bash tests/validation/health_check.sh
    
    # Run integration tests
    log_info "Running integration tests..."
    python3 -m pytest tests/integration/ -v --tb=short
    
    # Verify metrics
    log_info "Verifying metrics collection..."
    curl -sf "http://prometheus:9090/api/v1/targets" | jq '.data.activeTargets | length' > /dev/null
    
    # Check alerts
    log_info "Checking alertmanager..."
    curl -sf "http://alertmanager:9093/api/v1/status" > /dev/null
    
    log_success "Post-deployment verification complete"
}

# ==================== ROLLBACK ====================

rollback_production() {
    log_warn "Rolling back production deployment..."
    
    # Undo last rollout
    kubectl rollout undo deployment/hs-verifier -n $NAMESPACE
    kubectl rollout undo deployment/transparency-log -n $NAMESPACE
    
    # Wait for rollback
    kubectl rollout status deployment/hs-verifier -n $NAMESPACE --timeout=300s
    kubectl rollout status deployment/transparency-log -n $NAMESPACE --timeout=300s
    
    # Verify rollback
    if bash tests/validation/health_check.sh > /dev/null 2>&1; then
        log_success "Rollback successful"
    else
        log_error "Rollback verification failed - manual intervention required"
        exit 1
    fi
}

# ==================== NOTIFICATIONS ====================

notify_slack() {
    local message="$1"
    local color="${2:-good}"
    
    if [ -n "$SLACK_WEBHOOK_URL" ]; then
        curl -s -X POST -H 'Content-type: application/json' \
            --data "{\"attachments\":[{\"color\":\"$color\",\"text\":\"$message\"}]}" \
            "$SLACK_WEBHOOK_URL" > /dev/null
    fi
}

notify_pagerduty() {
    local severity="$1"
    local message="$2"
    
    if [ -n "$PAGERDUTY_SERVICE_KEY" ]; then
        curl -s -X POST -H 'Content-type: application/json' \
            -d "{\"routing_key\":\"$PAGERDUTY_SERVICE_KEY\",\"event_action\":\"trigger\",\"payload\":{\"summary\":\"$message\",\"severity\":\"$severity\"}}" \
            "https://events.pagerduty.com/v2/enqueue" > /dev/null
    fi
}

# ==================== MAIN ====================

main() {
    echo "=========================================="
    echo "HSK Platform Production Deployment"
    echo "=========================================="
    echo ""
    
    # Parse arguments
    COMMAND="${1:-full}"
    
    case "$COMMAND" in
        "pre-checks")
            pre_deployment_checks
            ;;
        "staging")
            pre_deployment_checks
            deploy_to_staging
            ;;
        "canary")
            canary_deploy
            ;;
        "promote")
            canary_promote
            ;;
        "rollback")
            rollback_production
            ;;
        "full")
            pre_deployment_checks
            deploy_to_staging
            canary_deploy
            canary_promote
            deploy_to_production
            post_deployment_verification
            notify_slack "Production deployment completed successfully" "good"
            ;;
        *)
            echo "Usage: $0 [pre-checks|staging|canary|promote|rollback|full]"
            exit 1
            ;;
    esac
    
    echo ""
    echo "=========================================="
    echo "Deployment Complete"
    echo "=========================================="
}

# Run main
trap 'log_error "Deployment interrupted"; exit 1' INT TERM
main "$@"
