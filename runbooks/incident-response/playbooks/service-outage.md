# Incident Response Playbook: Service Outage

## Overview

| Field | Value |
|-------|-------|
| **Severity** | P1 - Critical |
| **Response Time** | 5 minutes |
| **Escalation Time** | 15 minutes |
| **Owner** | SRE Team |

## Symptoms

- Service returning 5xx errors
- High error rate in monitoring
- Health check failures
- Increased latency
- Complete service unavailability

## Initial Assessment (First 5 minutes)

### 1. Verify the Incident

```bash
# Check service health endpoint
kubectl get pods -n hsk-production -l app=hsk-verifier
kubectl logs -n hsk-production -l app=hsk-verifier --tail=100

# Check error rate
curl -s "http://prometheus:9090/api/v1/query?query=sum(rate(http_requests_total{status=~\"5..\"}[5m]))"
```

### 2. Assess Scope

```bash
# Check which services are affected
kubectl get pods -n hsk-production --field-selector=status.phase!=Running

# Check recent deployments
kubectl rollout history deployment/hsk-verifier -n hsk-production
```

## Response Procedures

### Scenario 1: Recent Deployment Caused Issue

```bash
# Immediate rollback
kubectl rollout undo deployment/hsk-verifier -n hsk-production

# Verify rollback
kubectl rollout status deployment/hsk-verifier -n hsk-production

# Monitor for recovery
kubectl logs -n hsk-production -l app=hsk-verifier -f
```

### Scenario 2: Database Connection Issues

```bash
# Check database connectivity
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "SELECT 1;"

# Check connection pool status
kubectl exec -n hsk-production deployment/hsk-verifier -- curl localhost:8080/metrics | grep hsk_database_connections

# Restart pods if needed
kubectl rollout restart deployment/hsk-verifier -n hsk-production
```

### Scenario 3: Resource Exhaustion

```bash
# Check resource usage
kubectl top pods -n hsk-production

# Check for OOM kills
kubectl get events -n hsk-production --field-selector=reason=OOMKilled

# Scale up immediately
kubectl scale deployment/hsk-verifier -n hsk-production --replicas=10

# Check HPA status
kubectl get hpa -n hsk-production
```

### Scenario 4: Network Issues

```bash
# Check Istio sidecar status
kubectl get pods -n hsk-production -o json | jq '.items[].status.containerStatuses[] | select(.name=="istio-proxy") | .ready'

# Check service mesh connectivity
istioctl proxy-status -n hsk-production

# Restart Istio proxy if needed
kubectl rollout restart deployment/hsk-verifier -n hsk-production
```

## Communication

### Internal (Slack)

```
🚨 INCIDENT: Service Outage - hsk-verifier
Severity: P1
Impact: [Describe user impact]
Started: [Timestamp]
Status: Investigating
Incident Commander: [Your name]
Channel: #incident-[timestamp]
```

### External (Status Page)

Update status page if user-facing impact:
- Identified → Investigating
- Investigating → Monitoring (after fix)
- Monitoring → Resolved (after 30 min stable)

## Escalation Path

1. **5 min**: Page on-call engineer
2. **15 min**: Escalate to SRE lead
3. **30 min**: Escalate to engineering manager
4. **1 hour**: Executive notification

## Post-Incident

Within 24 hours:
1. Create incident timeline
2. Document root cause
3. Identify action items
4. Schedule post-mortem

## Runbook Updates

After each incident, update this runbook with lessons learned.
