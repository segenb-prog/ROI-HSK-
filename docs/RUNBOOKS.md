# HSK Platform Operational Runbooks

## Table of Contents

1. [Deployment Runbook](#deployment-runbook)
2. [Incident Response Runbook](#incident-response-runbook)
3. [Backup and Recovery Runbook](#backup-and-recovery-runbook)
4. [Scaling Runbook](#scaling-runbook)
5. [Security Incident Runbook](#security-incident-runbook)
6. [Compliance Audit Runbook](#compliance-audit-runbook)

---

## Deployment Runbook

### Pre-Deployment Checklist

```bash
# 1. Validate all configurations
make validate

# 2. Run all tests
make test

# 3. Security scan
make security-scan

# 4. Check secrets
make check-secrets

# 5. Build images
make docker-build
```

### Staging Deployment

```bash
# 1. Deploy to staging
make k8s-deploy-staging

# 2. Verify deployment
kubectl get pods -n hsk-staging

# 3. Run smoke tests
make smoke-test-staging

# 4. Run integration tests
make k8s-test

# 5. Verify health
make health-check-staging
```

### Production Deployment

```bash
# 1. Canary deployment
make canary-deploy

# 2. Monitor canary
make canary-status

# 3. Check metrics
# - Error rate < 1%
# - P95 latency < 500ms
# - Success rate > 99%

# 4. Promote or rollback
make canary-promote    # If metrics look good
make canary-rollback   # If issues detected
```

### Rollback Procedure

```bash
# Emergency rollback
kubectl rollout undo deployment/hs-verifier -n hsk-verifier

# Verify rollback
kubectl rollout status deployment/hs-verifier -n hsk-verifier

# Check logs
kubectl logs -l app=hs-verifier -n hsk-verifier --tail=100
```

---

## Incident Response Runbook

### Severity Levels

| Level | Description | Response Time | Examples |
|-------|-------------|---------------|----------|
| P1 | Critical | 15 min | Complete outage, data breach |
| P2 | High | 1 hour | Partial outage, performance degraded |
| P3 | Medium | 4 hours | Non-critical feature failure |
| P4 | Low | 24 hours | Minor issues, cosmetic bugs |

### Incident Response Process

```
1. DETECT
   - Alert fires
   - On-call engineer acknowledges

2. ASSESS
   - Determine severity
   - Identify affected components
   - Check recent deployments

3. RESPOND
   - P1/P2: Page additional engineers
   - Create incident channel
   - Start incident log

4. RESOLVE
   - Implement fix
   - Verify resolution
   - Monitor for stability

5. POST-INCIDENT
   - Write post-mortem
   - Identify action items
   - Update runbooks
```

### Common Incidents

#### Database Connection Issues

```bash
# Check database pods
kubectl get pods -n hsk-verifier -l app=database

# Check logs
kubectl logs -n hsk-verifier -l app=database --tail=100

# Check connection pool
kubectl exec -n hsk-verifier deployment/hs-verifier -- curl localhost:8080/metrics | grep db_connections

# Restart if needed
kubectl rollout restart deployment/transparency-db -n hsk-verifier
```

#### High Error Rate

```bash
# Check error logs
kubectl logs -n hsk-verifier -l app=hs-verifier | grep ERROR

# Check recent deployments
kubectl rollout history deployment/hs-verifier -n hsk-verifier

# Rollback if needed
kubectl rollout undo deployment/hs-verifier -n hsk-verifier

# Scale up if under load
kubectl scale deployment/hs-verifier -n hsk-verifier --replicas=10
```

#### Memory Issues

```bash
# Check memory usage
kubectl top pods -n hsk-verifier

# Check OOM kills
kubectl get events -n hsk-verifier | grep OOM

# Increase memory limits
kubectl patch deployment hs-verifier -n hsk-verifier -p '{"spec":{"template":{"spec":{"containers":[{"name":"hs-verifier","resources":{"limits":{"memory":"4Gi"}}}]}}}}'

# Restart pods
kubectl rollout restart deployment/hs-verifier -n hsk-verifier
```

---

## Backup and Recovery Runbook

### Backup Verification

```bash
# Check backup jobs
kubectl get cronjobs -n hsk-verifier

# Check last backup
kubectl get jobs -n hsk-verifier | grep backup

# Verify backup exists in S3
aws s3 ls s3://hsk-backups-primary/hourly/ | tail -5

# Test backup integrity
aws s3 cp s3://hsk-backups-primary/hourly/latest.sql.gz - | gunzip | head -c 1000
```

### Point-in-Time Recovery

```bash
# 1. Stop application
kubectl scale deployment/hs-verifier -n hsk-verifier --replicas=0

# 2. Create recovery job
kubectl apply -f backup-system/pitr-recovery.yaml

# 3. Monitor recovery
kubectl logs -f job/point-in-time-recovery -n hsk-verifier

# 4. Verify data
kubectl exec -n hsk-verifier deployment/transparency-db -- psql -c "SELECT COUNT(*) FROM consent_entries;"

# 5. Restart application
kubectl scale deployment/hs-verifier -n hsk-verifier --replicas=3
```

### Cross-Region Failover

```bash
# 1. Verify secondary region is healthy
kubectl --context=eu-west get pods -n hsk-verifier

# 2. Update DNS to point to secondary
# Update Route53/Cloudflare records

# 3. Promote secondary to primary
kubectl --context=eu-west exec deployment/transparency-db -- pg_ctl promote

# 4. Verify traffic routing
curl https://api.hskernel.io/health

# 5. Monitor for issues
# Watch dashboards, check error rates
```

---

## Scaling Runbook

### Horizontal Scaling

```bash
# Scale deployments
kubectl scale deployment/hs-verifier -n hsk-verifier --replicas=10

# Verify HPA is working
kubectl get hpa -n hsk-verifier

# Check metrics
kubectl top pods -n hsk-verifier
```

### Vertical Scaling

```bash
# Increase CPU/memory
kubectl patch deployment hs-verifier -n hsk-verifier -p '{
  "spec": {
    "template": {
      "spec": {
        "containers": [{
          "name": "hs-verifier",
          "resources": {
            "requests": {"cpu": "1", "memory": "2Gi"},
            "limits": {"cpu": "4", "memory": "8Gi"}
          }
        }]
      }
    }
  }
}'

# Monitor resource usage
watch kubectl top pods -n hsk-verifier
```

### Database Scaling

```bash
# Add read replica
kubectl apply -f k8s-deployments/database-replica.yaml

# Update connection pool
kubectl patch configmap database-config -n hsk-verifier --patch '{
  "data": {
    "read_replicas": "transparency-db-replica-1,transparency-db-replica-2"
  }
}'

# Restart application
kubectl rollout restart deployment/hs-verifier -n hsk-verifier
```

---

## Security Incident Runbook

### Data Breach Response

```
1. IMMEDIATE (0-15 min)
   - Isolate affected systems
   - Preserve evidence
   - Notify security team

2. CONTAINMENT (15-60 min)
   - Revoke compromised credentials
   - Block malicious IPs
   - Enable additional logging

3. INVESTIGATION (1-24 hours)
   - Determine scope of breach
   - Identify affected data
   - Document timeline

4. NOTIFICATION (24-72 hours)
   - Legal review
   - Regulatory notifications
   - User notifications (if required)

5. RECOVERY (1-7 days)
   - Fix vulnerabilities
   - Restore from clean backups
   - Enhanced monitoring

6. POST-INCIDENT
   - Post-mortem
   - Process improvements
   - Compliance reporting
```

### Revoking Compromised Credentials

```bash
# Revoke Vault tokens
vault token revoke -mode path auth/token

# Rotate database credentials
vault write -f database/rotate-role/hsk-app

# Rotate API keys
kubectl create secret generic hsk-api-keys \
  --from-literal=api-key=$(openssl rand -hex 32) \
  --dry-run=client -o yaml | kubectl apply -f -

# Restart services
kubectl rollout restart deployment/hs-verifier -n hsk-verifier
```

### Blocking Malicious IPs

```bash
# Add network policy
kubectl apply -f - <<EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: block-malicious
  namespace: hsk-verifier
spec:
  podSelector: {}
  policyTypes:
  - Ingress
  ingress:
  - from:
    - ipBlock:
        cidr: 0.0.0.0/0
        except:
        - 192.0.2.0/24  # Block this range
EOF

# Update WAF rules
# Add IPs to Cloudflare/AWS WAF block list
```

---

## Compliance Audit Runbook

### SOC 2 Audit Preparation

```bash
# 1. Gather evidence
echo "Collecting evidence..."

# Access logs
kubectl logs -n hsk-verifier --all-containers | gzip > /tmp/access-logs.gz

# Configuration files
tar czf /tmp/configs.tar.gz k8s-deployments/

# Policy documents
cp compliance-docs/SOC2_CONTROLS.md /tmp/

# 2. Run compliance checks
make compliance-check

# 3. Generate reports
python3 scripts/generate_compliance_report.py

# 4. Prepare for auditor
# - Grant read-only access
# - Schedule walkthrough
# - Prepare demo
```

### GDPR Audit Preparation

```bash
# 1. Verify data retention
kubectl exec -n hsk-verifier deployment/transparency-db -- psql -c "
  SELECT 
    COUNT(*) as total_records,
    COUNT(CASE WHEN created_at < NOW() - INTERVAL '7 years' THEN 1 END) as expired_records
  FROM consent_entries;
"

# 2. Check deletion requests
kubectl exec -n hsk-verifier deployment/transparency-db -- psql -c "
  SELECT 
    status,
    COUNT(*)
  FROM gdpr_deletion_requests
  GROUP BY status;
"

# 3. Verify data exports
ls -la /exports/

# 4. Generate GDPR report
python3 scripts/generate_gdpr_report.py
```

### Evidence Collection

```bash
#!/bin/bash
# collect_evidence.sh

DATE=$(date +%Y%m%d)
EVIDENCE_DIR="/evidence/$DATE"
mkdir -p $EVIDENCE_DIR

# System configuration
kubectl get all -n hsk-verifier -o yaml > $EVIDENCE_DIR/k8s-resources.yaml
kubectl get configmaps -n hsk-verifier -o yaml > $EVIDENCE_DIR/configmaps.yaml
kubectl get secrets -n hsk-verifier -o yaml > $EVIDENCE_DIR/secrets.yaml

# Network policies
kubectl get networkpolicies -n hsk-verifier -o yaml > $EVIDENCE_DIR/network-policies.yaml

# RBAC
kubectl get roles,rolebindings -n hsk-verifier -o yaml > $EVIDENCE_DIR/rbac.yaml

# Audit logs
kubectl logs -n hsk-verifier -l app=audit --since=720h > $EVIDENCE_DIR/audit-logs.txt

# Backup verification
aws s3 ls s3://hsk-backups-primary/ > $EVIDENCE_DIR/backup-list.txt

# Create tarball
tar czf /evidence/evidence-$DATE.tar.gz $EVIDENCE_DIR
echo "Evidence collected: /evidence/evidence-$DATE.tar.gz"
```

---

## Quick Reference

### Common Commands

```bash
# View logs
kubectl logs -f deployment/hs-verifier -n hsk-verifier

# Exec into pod
kubectl exec -it deployment/hs-verifier -n hsk-verifier -- /bin/sh

# Port forward
kubectl port-forward service/hs-verifier 8080:8080 -n hsk-verifier

# Check metrics
curl http://localhost:8080/metrics

# Database query
kubectl exec -it deployment/transparency-db -n hsk-verifier -- psql -d consent_ledger

# Redis query
kubectl exec -it deployment/redis -n hsk-verifier -- redis-cli

# Vault operations
kubectl exec -it vault-0 -n vault -- vault status
```

### Emergency Contacts

| Role | Contact | Escalation |
|------|---------|------------|
| On-Call Engineer | PagerDuty | 15 min |
| Platform Lead | +1-555-HSK-LEAD | 30 min |
| Security Team | security@hskernel.io | Immediate |
| Compliance Officer | compliance@hskernel.io | 1 hour |

### Useful Links

- Grafana: https://grafana.hskernel.io
- Prometheus: https://prometheus.hskernel.io
- Jaeger: https://jaeger.hskernel.io
- Status Page: https://status.hskernel.io
- Documentation: https://docs.hskernel.io
