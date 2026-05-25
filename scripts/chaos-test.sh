#!/bin/bash
# Chaos Engineering Test Runner
# Runs comprehensive chaos experiments and monitors system resilience

set -e

NAMESPACE="hsk-verifier"
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}============================================${NC}"
echo -e "${BLUE}   Chaos Engineering Test Suite${NC}"
echo -e "${BLUE}============================================${NC}"
echo ""

# Check prerequisites
check_prerequisites() {
    echo -e "${BLUE}Checking prerequisites...${NC}"
    
    if ! kubectl get namespace chaos-mesh &> /dev/null; then
        echo -e "${RED}Error: Chaos Mesh not installed${NC}"
        echo "Run: make install-chaos-mesh"
        exit 1
    fi
    
    if ! kubectl get pods -n $NAMESPACE -l app=hs-verifier | grep -q Running; then
        echo -e "${RED}Error: HSK Verifier not running in namespace $NAMESPACE${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Prerequisites check passed!${NC}"
    echo ""
}

# Get baseline metrics
get_baseline() {
    echo -e "${BLUE}Capturing baseline metrics...${NC}"
    
    BASELINE_PODS=$(kubectl get pods -n $NAMESPACE -l app=hs-verifier --no-headers | wc -l)
    BASELINE_HEALTH=$(curl -s http://localhost:8080/health 2>/dev/null | jq -r '.status' || echo "unknown")
    
    echo "  Baseline pods: $BASELINE_PODS"
    echo "  Baseline health: $BASELINE_HEALTH"
    echo ""
}

# Run pod failure experiment
run_pod_failure() {
    echo -e "${YELLOW}Experiment 1: Pod Failure${NC}"
    echo -e "${BLUE}Injecting pod failures...${NC}"
    
    kubectl apply -f - <<EOF
apiVersion: chaos-mesh.org/v1alpha1
kind: PodChaos
metadata:
  name: pod-failure-test
  namespace: $NAMESPACE
spec:
  action: pod-failure
  mode: fixed-percent
  value: "50"
  duration: "30s"
  selector:
    labelSelectors:
      app: hs-verifier
EOF
    
    echo -e "${BLUE}Waiting 30 seconds for experiment...${NC}"
    sleep 30
    
    # Check recovery
    RECOVERED_PODS=$(kubectl get pods -n $NAMESPACE -l app=hs-verifier --no-headers | grep Running | wc -l)
    
    if [ "$RECOVERED_PODS" -ge "$BASELINE_PODS" ]; then
        echo -e "${GREEN}✓ Pod failure recovery: PASSED${NC}"
    else
        echo -e "${RED}✗ Pod failure recovery: FAILED${NC}"
        echo "  Expected: $BASELINE_PODS pods, Got: $RECOVERED_PODS"
    fi
    
    kubectl delete podchaos pod-failure-test -n $NAMESPACE --ignore-not-found
    echo ""
}

# Run network delay experiment
run_network_delay() {
    echo -e "${YELLOW}Experiment 2: Network Delay${NC}"
    echo -e "${BLUE}Injecting network latency...${NC}"
    
    kubectl apply -f - <<EOF
apiVersion: chaos-mesh.org/v1alpha1
kind: NetworkChaos
metadata:
  name: network-delay-test
  namespace: $NAMESPACE
spec:
  action: delay
  mode: all
  selector:
    labelSelectors:
      app: hs-verifier
  delay:
    latency: "100ms"
    correlation: "100"
    jitter: "0ms"
  duration: "30s"
EOF
    
    echo -e "${BLUE}Testing API latency under network delay...${NC}"
    
    # Measure response time
    START_TIME=$(date +%s%N)
    RESPONSE=$(curl -s -o /dev/null -w "%{http_code}" http://localhost:8080/health 2>/dev/null || echo "000")
    END_TIME=$(date +%s%N)
    DURATION=$(( (END_TIME - START_TIME) / 1000000 ))
    
    if [ "$RESPONSE" = "200" ]; then
        echo -e "${GREEN}✓ API responded in ${DURATION}ms (with 100ms delay)${NC}"
    else
        echo -e "${RED}✗ API failed with code: $RESPONSE${NC}"
    fi
    
    kubectl delete networkchaos network-delay-test -n $NAMESPACE --ignore-not-found
    echo ""
}

# Run stress test
run_stress_test() {
    echo -e "${YELLOW}Experiment 3: CPU/Memory Stress${NC}"
    echo -e "${BLUE}Injecting resource stress...${NC}"
    
    kubectl apply -f - <<EOF
apiVersion: chaos-mesh.org/v1alpha1
kind: StressChaos
metadata:
  name: stress-test
  namespace: $NAMESPACE
spec:
  mode: all
  selector:
    labelSelectors:
      app: hs-verifier
  stressors:
    cpu:
      workers: 2
      load: 80
    memory:
      workers: 2
      size: "256Mi"
  duration: "30s"
EOF
    
    echo -e "${BLUE}Monitoring system under stress...${NC}"
    sleep 30
    
    # Check if pods are still healthy
    HEALTHY_PODS=$(kubectl get pods -n $NAMESPACE -l app=hs-verifier --no-headers | grep -c Running || echo "0")
    
    if [ "$HEALTHY_PODS" -gt 0 ]; then
        echo -e "${GREEN}✓ System survived stress test${NC}"
    else
        echo -e "${RED}✗ System failed under stress${NC}"
    fi
    
    kubectl delete stresschaos stress-test -n $NAMESPACE --ignore-not-found
    echo ""
}

# Run network partition experiment
run_network_partition() {
    echo -e "${YELLOW}Experiment 4: Network Partition${NC}"
    echo -e "${BLUE}Creating network partition...${NC}"
    
    kubectl apply -f - <<EOF
apiVersion: chaos-mesh.org/v1alpha1
kind: NetworkChaos
metadata:
  name: network-partition-test
  namespace: $NAMESPACE
spec:
  action: partition
  mode: all
  selector:
    labelSelectors:
      app: hs-verifier
  direction: both
  target:
    selector:
      labelSelectors:
        app: transparency-db
    mode: all
  duration: "20s"
EOF
    
    echo -e "${BLUE}Testing behavior during partition...${NC}"
    sleep 20
    
    # Check if verifier handled partition gracefully
    RESPONSE=$(curl -s http://localhost:8080/health 2>/dev/null | jq -r '.status' || echo "error")
    
    if [ "$RESPONSE" = "healthy" ] || [ "$RESPONSE" = "degraded" ]; then
        echo -e "${GREEN}✓ System handled partition gracefully${NC}"
    else
        echo -e "${YELLOW}! System may have issues during partition (status: $RESPONSE)${NC}"
    fi
    
    kubectl delete networkchaos network-partition-test -n $NAMESPACE --ignore-not-found
    echo ""
}

# Generate report
generate_report() {
    echo -e "${BLUE}============================================${NC}"
    echo -e "${BLUE}   Chaos Test Report${NC}"
    echo -e "${BLUE}============================================${NC}"
    echo ""
    echo "Timestamp: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "Namespace: $NAMESPACE"
    echo ""
    echo -e "${GREEN}All chaos experiments completed!${NC}"
    echo ""
    echo "Check Grafana dashboards for detailed metrics:"
    echo "  http://localhost:3000/d/hsk-verifier"
    echo ""
}

# Main execution
main() {
    check_prerequisites
    get_baseline
    
    echo -e "${YELLOW}Starting chaos experiments...${NC}"
    echo ""
    
    run_pod_failure
    run_network_delay
    run_stress_test
    run_network_partition
    
    generate_report
}

# Handle interruption
trap 'echo -e "\n${RED}Chaos test interrupted${NC}"; kubectl delete podchaos,networkchaos,stresschaos --all -n $NAMESPACE --ignore-not-found; exit 1' INT TERM

main "$@"
