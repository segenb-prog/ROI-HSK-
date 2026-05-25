#!/bin/bash
# HSK Platform Cost Report
# Analyzes resource usage and provides cost optimization recommendations

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-hsk-verifier}"
OUTPUT_FORMAT="${OUTPUT_FORMAT:-text}"  # text, json, csv

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

log_section() {
    echo -e "\n${BLUE}=== $1 ===${NC}"
}

# Get current resource usage
get_resource_usage() {
    log_section "Current Resource Usage"
    
    # Get pod metrics
    kubectl top pods -n "$NAMESPACE" 2>/dev/null || {
        log_warn "Metrics not available. Is metrics-server installed?"
        return 1
    }
    
    # Get node metrics
    log_section "Node Usage"
    kubectl top nodes 2>/dev/null || true
}

# Analyze resource requests vs usage
analyze_efficiency() {
    log_section "Resource Efficiency Analysis"
    
    echo "Pod | Container | CPU Request | CPU Usage | Memory Request | Memory Usage | Efficiency"
    echo "----|-----------|-------------|-----------|----------------|--------------|------------"
    
    kubectl get pods -n "$NAMESPACE" -o json | jq -r '
        .items[] |
        select(.status.phase == "Running") |
        .metadata.name as $pod |
        .spec.containers[] |
        "\($pod) | \(.name) | \(.resources.requests.cpu // "0") | - | \(.resources.requests.memory // "0") | - | -"
    '
}

# Calculate estimated costs
calculate_costs() {
    log_section "Estimated Monthly Costs"
    
    # Get all pods with their resource requests
    PODS=$(kubectl get pods -n "$NAMESPACE" -o json)
    
    # Count pods by type
    VERIFIER_PODS=$(echo "$PODS" | jq '[.items[] | select(.metadata.labels.app == "hs-verifier")] | length')
    LOG_PODS=$(echo "$PODS" | jq '[.items[] | select(.metadata.labels.app == "transparency-log")] | length')
    DB_PODS=$(echo "$PODS" | jq '[.items[] | select(.metadata.labels.app == "transparency-db")] | length')
    
    # Pricing (example AWS us-east-1 prices)
    CPU_PRICE_PER_CORE=0.031611  # per hour
    MEM_PRICE_PER_GB=0.003968    # per hour
    STORAGE_PRICE_PER_GB=0.10    # per GB-month
    
    # Calculate verifier costs (assuming 0.5 CPU, 512MB each)
    VERIFIER_CPU_COST=$(echo "$VERIFIER_PODS * 0.5 * $CPU_PRICE_PER_CORE * 730" | bc -l)
    VERIFIER_MEM_COST=$(echo "$VERIFIER_PODS * 0.5 * $MEM_PRICE_PER_GB * 730" | bc -l)
    
    # Calculate log costs (assuming 1 CPU, 1GB each)
    LOG_CPU_COST=$(echo "$LOG_PODS * 1.0 * $CPU_PRICE_PER_CORE * 730" | bc -l)
    LOG_MEM_COST=$(echo "$LOG_PODS * 1.0 * $MEM_PRICE_PER_GB * 730" | bc -l)
    
    # Calculate DB costs (assuming 2 CPU, 4GB, 500GB storage)
    DB_CPU_COST=$(echo "$DB_PODS * 2.0 * $CPU_PRICE_PER_CORE * 730" | bc -l)
    DB_MEM_COST=$(echo "$DB_PODS * 4.0 * $MEM_PRICE_PER_GB * 730" | bc -l)
    DB_STORAGE_COST=$(echo "$DB_PODS * 500 * $STORAGE_PRICE_PER_GB" | bc -l)
    
    TOTAL=$(echo "$VERIFIER_CPU_COST + $VERIFIER_MEM_COST + $LOG_CPU_COST + $LOG_MEM_COST + $DB_CPU_COST + $DB_MEM_COST + $DB_STORAGE_COST" | bc -l)
    
    echo ""
    echo "Component          | Pods | CPU Cost | Memory Cost | Storage Cost | Total"
    echo "-------------------|------|----------|-------------|--------------|-------"
    printf "%-18s | %4d | $%7.2f | $%9.2f | $%10.2f | $%7.2f\n" "HSK Verifier" "$VERIFIER_PODS" "$VERIFIER_CPU_COST" "$VERIFIER_MEM_COST" "0.00" "$(echo "$VERIFIER_CPU_COST + $VERIFIER_MEM_COST" | bc -l)"
    printf "%-18s | %4d | $%7.2f | $%9.2f | $%10.2f | $%7.2f\n" "Transparency Log" "$LOG_PODS" "$LOG_CPU_COST" "$LOG_MEM_COST" "0.00" "$(echo "$LOG_CPU_COST + $LOG_MEM_COST" | bc -l)"
    printf "%-18s | %4d | $%7.2f | $%9.2f | $%10.2f | $%7.2f\n" "PostgreSQL" "$DB_PODS" "$DB_CPU_COST" "$DB_MEM_COST" "$DB_STORAGE_COST" "$(echo "$DB_CPU_COST + $DB_MEM_COST + $DB_STORAGE_COST" | bc -l)"
    echo "-------------------|------|----------|-------------|--------------|-------"
    printf "%-18s | %4d |          |             |              | $%7.2f\n" "TOTAL" "$((VERIFIER_PODS + LOG_PODS + DB_PODS))" "$TOTAL"
    
    echo ""
    log_info "Estimated monthly cost: ~$$(printf "%.2f" "$TOTAL")"
}

# Generate optimization recommendations
generate_recommendations() {
    log_section "Optimization Recommendations"
    
    # Check for over-provisioned pods
    echo "1. Right-sizing Recommendations:"
    
    # Check if HPA is configured
    if kubectl get hpa -n "$NAMESPACE" &> /dev/null; then
        log_info "✓ HPA is configured"
        kubectl get hpa -n "$NAMESPACE"
    else
        log_warn "✗ HPA not configured - consider adding for automatic scaling"
    fi
    
    # Check for spot instances
    echo ""
    echo "2. Spot Instance Opportunities:"
    SPOT_PODS=$(kubectl get pods -n "$NAMESPACE" -l workload-type=spot --no-headers 2>/dev/null | wc -l)
    if [ "$SPOT_PODS" -gt 0 ]; then
        log_info "✓ Spot instances in use: $SPOT_PODS pods"
    else
        log_warn "✗ No spot instances - could save ~70% on compute costs"
        echo "   Recommendation: Deploy hs-verifier-spot for non-critical workloads"
    fi
    
    # Check for VPA
    echo ""
    echo "3. Vertical Scaling Recommendations:"
    if kubectl get vpa -n "$NAMESPACE" &> /dev/null; then
        log_info "✓ VPA is configured"
        kubectl get vpa -n "$NAMESPACE" -o json | jq -r '.items[] | "   \(.metadata.name): \(.status.recommendation.containerRecommendations[0].target)"' 2>/dev/null || true
    else
        log_warn "✗ VPA not configured - consider adding for right-sizing"
    fi
    
    # Check for resource quotas
    echo ""
    echo "4. Resource Quotas:"
    if kubectl get resourcequota -n "$NAMESPACE" &> /dev/null; then
        log_info "✓ Resource quotas configured"
        kubectl get resourcequota -n "$NAMESPACE"
    else
        log_warn "✗ No resource quotas - consider adding limits"
    fi
}

# Export to different formats
export_report() {
    local format="$1"
    local output_file="${2:-hsk-cost-report.$(date +%Y%m%d)}"
    
    case "$format" in
        json)
            kubectl get pods -n "$NAMESPACE" -o json | jq '{
                timestamp: now,
                namespace: .items[0].metadata.namespace,
                pods: [.items[] | {
                    name: .metadata.name,
                    resources: .spec.containers[].resources
                }]
            }' > "${output_file}.json"
            log_info "Report exported to ${output_file}.json"
            ;;
        csv)
            echo "pod,container,cpu_request,mem_request" > "${output_file}.csv"
            kubectl get pods -n "$NAMESPACE" -o json | jq -r '
                .items[] |
                .metadata.name as $pod |
                .spec.containers[] |
                "\($pod),\(.name),\(.resources.requests.cpu // 0),\(.resources.requests.memory // 0)"
            ' >> "${output_file}.csv"
            log_info "Report exported to ${output_file}.csv"
            ;;
        *)
            # Text format - already printed
            ;;
    esac
}

# Main execution
main() {
    log_info "HSK Platform Cost Report"
    log_info "Namespace: $NAMESPACE"
    log_info "Date: $(date)"
    
    get_resource_usage
    analyze_efficiency
    calculate_costs
    generate_recommendations
    
    if [ "$OUTPUT_FORMAT" != "text" ]; then
        export_report "$OUTPUT_FORMAT"
    fi
    
    log_info "Report complete!"
}

# Handle arguments
while getopts "n:f:o:h" opt; do
    case $opt in
        n) NAMESPACE="$OPTARG" ;;
        f) OUTPUT_FORMAT="$OPTARG" ;;
        o) OUTPUT_FILE="$OPTARG" ;;
        h)
            echo "Usage: $0 [-n namespace] [-f format] [-o output_file]"
            echo ""
            echo "Options:"
            echo "  -n    Namespace (default: hsk-verifier)"
            echo "  -f    Output format: text, json, csv (default: text)"
            echo "  -o    Output file (default: hsk-cost-report.YYYYMMDD)"
            echo "  -h    Show this help"
            exit 0
            ;;
        *)
            echo "Unknown option: -$opt"
            exit 1
            ;;
    esac
done

main
