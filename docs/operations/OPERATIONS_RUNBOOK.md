# HSK Operations Runbook

Daily operational procedures for the HSK Falsification Machine platform.

---

## Table of Contents

1. [Daily Operations](#daily-operations)
2. [Weekly Operations](#weekly-operations)
3. [Monthly Operations](#monthly-operations)
4. [Troubleshooting](#troubleshooting)
5. [Maintenance Windows](#maintenance-windows)

---

## Daily Operations

### Morning Check (9:00 AM)

```bash
#!/bin/bash
# daily-check.sh

echo "=== HSK Daily Health Check ==="
echo "Date: $(date)"

# Check pod status
echo ""
echo "Pod Status:"
kubectl get pods -n hsk-verifier

# Check service status
echo ""
echo "Service Status:"
kubectl get svc -n hsk-verifier

# Check resource usage
echo ""
echo "Resource Usage:"
kubectl top pods -n hsk-verifier 2>/dev/null || echo "Metrics not available"

# Check recent events
echo ""
echo "Recent Events:"
kubectl get events -n hsk-verifier --field-selector type=Warning --sort-by='.lastTimestamp' | tail -10

# Health check endpoints
echo ""
echo "Health Checks:"
curl -s http://verifier.hskernel.dev/health | jq .
curl -s http://log1.hskernel.dev/health | jq .

# Check certificate counts
echo ""
echo "Certificate Stats:"
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -t -c "
    SELECT 
      COUNT(*) as total,
      COUNT(*) FILTER (WHERE hs_compliant = false) as violations,
      COUNT(*) FILTER (WHERE submitted_at > NOW() - INTERVAL '24 hours') as last_24h
    FROM transparency.certificates;
  "

echo ""
echo "=== Check Complete ==="
```

### Log Review

```bash
# Review verifier logs
kubectl logs -n hsk-verifier -l app=hs-verifier --since=24h | grep -E "(ERROR|WARN|violation)"

# Review transparency log
kubectl logs -n hsk-verifier -l app=transparency-log --since=24h | grep -E "(ERROR|submit|sync)"

# Database slow queries
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT query, calls, mean_time
    FROM pg_stat_statements
    ORDER BY mean_time DESC
    LIMIT 10;
  "
```

### Backup Verification

```bash
# Check latest backup
ls -la /var/backups/hsk/ | head -5

# Verify backup integrity
cd /var/backups/hsk/$(ls -t /var/backups/hsk/ | head -1)
sha256sum -c manifest_*.json

# Test restore (weekly)
# ./scripts/restore.sh <timestamp> --dry-run
```

---

## Weekly Operations

### Monday: Key Health Check

```bash
# Check key expiration
kubectl exec -n hsk-verifier deployment/hs-verifier -- \
  hs-verifier keys

# Check key usage
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      issuer_public_key,
      COUNT(*) as cert_count,
      MAX(evaluation_time) as last_used
    FROM transparency.certificates
    GROUP BY issuer_public_key
    ORDER BY last_used DESC;
  "
```

### Wednesday: Capacity Planning

```bash
# Database size
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      schemaname,
      tablename,
      pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) as size
    FROM pg_tables
    WHERE schemaname IN ('transparency', 'consent_ledger')
    ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
  "

# Growth rate
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      DATE(submitted_at) as date,
      COUNT(*) as entries
    FROM transparency.certificates
    WHERE submitted_at > NOW() - INTERVAL '7 days'
    GROUP BY DATE(submitted_at)
    ORDER BY date;
  "

# Project storage needs
CURRENT_SIZE=$(kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -t -c "
    SELECT pg_database_size('transparency');
  " | tr -d ' \n')

WEEKLY_GROWTH=500000000  # 500MB example
PROJECTED_90DAYS=$((CURRENT_SIZE + WEEKLY_GROWTH * 12))

echo "Current size: $((CURRENT_SIZE / 1024 / 1024)) MB"
echo "Projected 90 days: $((PROJECTED_90DAYS / 1024 / 1024)) MB"
```

### Friday: Security Review

```bash
# Failed authentication attempts
kubectl logs -n hsk-verifier -l app=hs-verifier --since=7d | \
  grep -c "authentication failed" || echo "0"

# Unusual patterns
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      system_id,
      COUNT(*) as violations,
      MIN(evaluation_time) as first_seen,
      MAX(evaluation_time) as last_seen
    FROM transparency.certificates
    WHERE hs_compliant = false
      AND evaluation_time > NOW() - INTERVAL '7 days'
    GROUP BY system_id
    HAVING COUNT(*) > 5
    ORDER BY violations DESC;
  "

# Audit log review
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      operation,
      COUNT(*),
      performed_by
    FROM transparency.audit_log
    WHERE performed_at > NOW() - INTERVAL '7 days'
    GROUP BY operation, performed_by
    ORDER BY COUNT(*) DESC;
  "
```

---

## Monthly Operations

### First Monday: Full System Review

```bash
# Generate monthly report
cat > monthly-report.sh <<'EOF'
#!/bin/bash
REPORT_DATE=$(date +%Y-%m)
REPORT_FILE="hsk-report-${REPORT_DATE}.md"

cat > $REPORT_FILE <<REPORT
# HSK Monthly Report - ${REPORT_DATE}

## Overview
- Report Period: $(date -d "-1 month" +%Y-%m-01) to $(date +%Y-%m-%d)
- Generated: $(date)

## Certificate Statistics
$(kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      COUNT(*) as total_certificates,
      COUNT(*) FILTER (WHERE hs_compliant = false) as violations,
      COUNT(DISTINCT system_id) as systems_evaluated
    FROM transparency.certificates
    WHERE submitted_at > NOW() - INTERVAL '1 month';
  ")

## System Compliance
$(kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      system_id,
      COUNT(*) FILTER (WHERE hs_compliant = true) as compliant,
      COUNT(*) FILTER (WHERE hs_compliant = false) as violations,
      ROUND(
        COUNT(*) FILTER (WHERE hs_compliant = true) * 100.0 / COUNT(*), 2
      ) as compliance_rate
    FROM transparency.certificates
    WHERE submitted_at > NOW() - INTERVAL '1 month'
    GROUP BY system_id
    ORDER BY violations DESC
    LIMIT 10;
  ")

## Infrastructure Health
- Uptime: $(kubectl get pods -n hsk-verifier -o json | jq -r '.items[].status.conditions[] | select(.type=="Ready") | .status' | grep -c "True")/$(kubectl get pods -n hsk-verifier --no-headers | wc -l) pods ready
- Average Response Time: [From Prometheus]
- Error Rate: [From Prometheus]

## Action Items
- [ ] Review systems with < 95% compliance
- [ ] Update documentation
- [ ] Schedule security audit if needed

REPORT

echo "Report generated: $REPORT_FILE"
EOF

chmod +x monthly-report.sh
./monthly-report.sh
```

### Second Monday: Key Rotation

```bash
# Rotate signing keys
# WARNING: Do this during maintenance window!

# 1. Generate new key (air-gapped)
hs-verifier generate-keys --output new-keyring.json --offline

# 2. Add to Kubernetes as new secret
kubectl create secret generic hsk-keyring-new \
  --from-file=keyring.json=new-keyring.json \
  -n hsk-verifier

# 3. Update deployment to use new key
kubectl set env deployment/hs-verifier \
  HSK_KEYRING_PATH=/etc/hsk-new/keyring.json \
  -n hsk-verifier

# 4. Mount new secret
kubectl patch deployment hs-verifier -n hsk-verifier --type='json' -p='[
  {
    "op": "add",
    "path": "/spec/template/spec/volumes/-",
    "value": {
      "name": "keyring-new",
      "secret": {"secretName": "hsk-keyring-new"}
    }
  },
  {
    "op": "add",
    "path": "/spec/template/spec/containers/0/volumeMounts/-",
    "value": {
      "name": "keyring-new",
      "mountPath": "/etc/hsk-new",
      "readOnly": true
    }
  }
]'

# 5. Verify new key is active
kubectl rollout status deployment/hs-verifier -n hsk-verifier
kubectl exec -n hsk-verifier deployment/hs-verifier -- hs-verifier keys

# 6. After 24h, remove old key
kubectl delete secret hsk-keyring -n hsk-verifier
kubectl patch deployment hs-verifier -n hsk-verifier --type='json' -p='[
  {"op": "remove", "path": "/spec/template/spec/volumes/0"}
]'
```

### Third Monday: Performance Optimization

```bash
# Analyze slow queries
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT query, calls, mean_time, rows
    FROM pg_stat_statements
    WHERE mean_time > 100
    ORDER BY mean_time DESC
    LIMIT 20;
  "

# Check index usage
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT 
      schemaname,
      tablename,
      indexname,
      idx_scan,
      idx_tup_read
    FROM pg_stat_user_indexes
    WHERE schemaname = 'transparency'
    ORDER BY idx_scan ASC
    LIMIT 20;
  "

# Add missing indexes if needed
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    CREATE INDEX CONCURRENTLY IF NOT EXISTS 
    idx_certificates_evaluation_time 
    ON transparency.certificates(evaluation_time);
  "

# Vacuum and analyze
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "VACUUM ANALYZE;"
```

### Last Monday: Disaster Recovery Test

```bash
# Test backup restoration
LATEST_BACKUP=$(ls -t /var/backups/hsk/ | head -1)

# Create test namespace
kubectl create namespace hsk-verifier-test

# Restore to test namespace
# ./scripts/restore.sh $LATEST_BACKUP --namespace hsk-verifier-test

# Verify restoration
kubectl get pods -n hsk-verifier-test
kubectl exec -n hsk-verifier-test deployment/hs-verifier -- hs-verifier keys

# Cleanup
kubectl delete namespace hsk-verifier-test
```

---

## Troubleshooting

### Pod Not Starting

```bash
# Check events
kubectl describe pod <pod-name> -n hsk-verifier

# Check logs
kubectl logs <pod-name> -n hsk-verifier --previous

# Common issues:
# 1. Image pull error -> Check registry credentials
# 2. CrashLoopBackOff -> Check application logs
# 3. Pending -> Check resource limits, node capacity
```

### Database Connection Issues

```bash
# Test connection from verifier pod
kubectl exec -n hsk-verifier deployment/hs-verifier -- \
  nc -zv transparency-db 5432

# Check database logs
kubectl logs -n hsk-verifier -l app=transparency-db

# Check connection pool
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT count(*), state 
    FROM pg_stat_activity 
    GROUP BY state;
  "
```

### High Memory Usage

```bash
# Identify memory hogs
kubectl top pods -n hsk-verifier --sort-by=memory

# Check for memory leaks
kubectl logs -n hsk-verifier deployment/hs-verifier | grep -i "memory\|oom"

# Restart if needed
kubectl rollout restart deployment/hs-verifier -n hsk-verifier
```

### Certificate Verification Failing

```bash
# Check certificate
kubectl exec -n hsk-verifier deployment/hs-verifier -- \
  hs-verifier verify-cert <certificate-file>

# Check key validity
kubectl exec -n hsk-verifier deployment/hs-verifier -- \
  hs-verifier keys

# Check transparency log
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT * FROM transparency.certificates
    WHERE certificate_id = '<cert-id>';
  "
```

---

## Maintenance Windows

### Scheduled Maintenance

1. **Announce maintenance** (48 hours in advance)
2. **Enable maintenance mode**
```bash
# Add maintenance annotation
kubectl annotate ingress hs-verifier \
  nginx.ingress.kubernetes.io/configuration-snippet="
    return 503 'Service temporarily unavailable for maintenance';
  " -n hsk-verifier
```
3. **Perform maintenance**
4. **Verify services**
5. **Remove maintenance mode**

### Emergency Maintenance

```bash
# Immediate maintenance mode
kubectl scale deployment hs-verifier --replicas=0 -n hsk-verifier

# Notify stakeholders
# ./scripts/notify-stakeholders.sh "Emergency maintenance in progress"

# Perform fixes

# Restore service
kubectl scale deployment hs-verifier --replicas=3 -n hsk-verifier
kubectl rollout status deployment/hs-verifier -n hsk-verifier
```

---

## Useful Commands Reference

```bash
# Quick health check
curl -s https://verifier.hskernel.dev/health | jq .

# Get all resources
kubectl get all -n hsk-verifier

# Follow logs
kubectl logs -f -n hsk-verifier -l app=hs-verifier

# Execute command in pod
kubectl exec -it -n hsk-verifier deployment/hs-verifier -- /bin/sh

# Port forward for debugging
kubectl port-forward -n hsk-verifier svc/hs-verifier 8080:8080

# Check certificate
openssl x509 -in cert.pem -text -noout
```
