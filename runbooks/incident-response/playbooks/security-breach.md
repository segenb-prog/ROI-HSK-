# Incident Response Playbook: Security Breach

## Overview

| Field | Value |
|-------|-------|
| **Severity** | P0 - Emergency |
| **Response Time** | Immediate |
| **Escalation Time** | 5 minutes |
| **Owner** | Security Team |

## Classification

### Types of Security Incidents

1. **Data Breach** - Unauthorized access to user data
2. **Credential Compromise** - API keys, certificates, passwords exposed
3. **Malicious Activity** - DDoS, intrusion attempts, malware
4. **Insider Threat** - Unauthorized actions by internal users
5. **Supply Chain Attack** - Compromised dependencies

## Immediate Response (First 5 minutes)

### 1. Declare Security Incident

```bash
# Create incident channel
/slack create-channel incident-security-[timestamp]

# Page security team
/pager trigger hsk-security-team "Security incident declared"
```

### 2. Preserve Evidence

```bash
# Capture current state
kubectl get all -n hsk-production -o yaml > /tmp/incident-$(date +%s)-state.yaml

# Save logs
kubectl logs -n hsk-production --all-containers --since=1h > /tmp/incident-$(date +%s)-logs.txt

# Capture network state
kubectl exec -n hsk-production deployment/hsk-verifier -- netstat -tuln > /tmp/incident-$(date +%s)-netstat.txt
```

### 3. Isolate Affected Systems

```bash
# If specific pod compromised
kubectl delete pod -n hsk-production [compromised-pod] --force

# If service needs isolation
kubectl patch service hsk-verifier -n hsk-production -p '{"spec":{"selector":null}}'

# Enable emergency WAF rules
kubectl apply -f emergency-waf-rules.yaml
```

## Containment Procedures

### Credential Compromise

```bash
# 1. Rotate all secrets immediately
./scripts/emergency-key-rotation.sh

# 2. Revoke all active sessions
kubectl exec -n hsk-production deployment/hsk-verifier -- redis-cli FLUSHDB

# 3. Force re-authentication for all users
kubectl patch configmap hsk-config -n hsk-production --patch '{"data":{"FORCE_REAUTH":"true"}}'

# 4. Audit all recent API key usage
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "SELECT * FROM api_key_usage WHERE created_at > NOW() - INTERVAL '24 hours';"
```

### Data Breach

```bash
# 1. Identify affected data
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "
  SELECT table_name, COUNT(*) 
  FROM information_schema.tables 
  WHERE table_schema = 'public';
"

# 2. Enable audit logging for all queries
kubectl apply -f enhanced-audit-logging.yaml

# 3. Export access logs for analysis
kubectl logs -n hsk-production -l app=hsk-verifier --since=24h | grep -E "(SELECT|INSERT|UPDATE|DELETE)" > /tmp/data-access-$(date +%s).log

# 4. Notify affected users (after legal review)
# See: /runbooks/compliance/data-breach-notification.md
```

### DDoS Attack

```bash
# 1. Enable emergency rate limiting
kubectl apply -f emergency-rate-limits.yaml

# 2. Scale up to absorb traffic
kubectl scale deployment/hsk-verifier -n hsk-production --replicas=50

# 3. Enable Cloudflare/CDN protection
curl -X POST "https://api.cloudflare.com/client/v4/zones/[zone-id]/settings/security_level" \
  -H "Authorization: Bearer $CF_TOKEN" \
  -d '{"value":"under_attack"}'

# 4. Block malicious IPs
kubectl apply -f - <<EOF
apiVersion: projectcalico.org/v3
kind: GlobalNetworkPolicy
metadata:
  name: block-malicious-ips
spec:
  selector: app == 'hsk-verifier'
  types:
    - Ingress
  ingress:
    - action: Deny
      source:
        nets: ["$(cat malicious-ips.txt | tr '\n' ',')"]
EOF
```

## Investigation

### Log Analysis

```bash
# Search for suspicious patterns
kubectl logs -n hsk-production --all-containers --since=24h | \
  grep -iE "(error|fail|unauthorized|forbidden|injection|sql|xss)" | \
  sort | uniq -c | sort -rn | head -50

# Check for unusual API patterns
kubectl logs -n hsk-production -l app=hsk-verifier | \
  jq -r 'select(.level == "warn" or .level == "error") | [.timestamp, .method, .path, .status, .source_ip] | @tsv'

# Analyze authentication failures
kubectl logs -n hsk-production -l app=hsk-verifier | \
  jq -r 'select(.event_type == "auth_failure") | [.timestamp, .identity_id, .reason, .source_ip] | @tsv'
```

### Database Forensics

```sql
-- Check for unauthorized access
SELECT 
    identity_id,
    action,
    resource_type,
    resource_id,
    created_at,
    source_ip
FROM audit_log
WHERE created_at > NOW() - INTERVAL '24 hours'
    AND (action LIKE '%admin%' OR action LIKE '%delete%' OR action LIKE '%export%')
ORDER BY created_at DESC;

-- Check for bulk data access
SELECT 
    identity_id,
    COUNT(*) as access_count,
    MIN(created_at) as first_access,
    MAX(created_at) as last_access
FROM audit_log
WHERE created_at > NOW() - INTERVAL '24 hours'
GROUP BY identity_id
HAVING COUNT(*) > 1000
ORDER BY access_count DESC;
```

## Recovery

### Restore from Backup (if data corrupted)

```bash
# 1. Stop affected services
kubectl scale deployment hsk-verifier -n hsk-production --replicas=0

# 2. Restore database
./scripts/restore-from-backup.sh --timestamp [clean-backup-time]

# 3. Verify data integrity
kubectl run db-verify -n hsk-production --rm -i --image=hsk-verifier -- ./verify-data-integrity.sh

# 4. Restart services
kubectl scale deployment hsk-verifier -n hsk-production --replicas=3
```

### Certificate Rotation

```bash
# Emergency certificate rotation
vault write -f pki/issue/hsk-verifier common_name=verifier.hsk.internal ttl=720h
kubectl create secret tls hsk-verifier-tls --cert=new-cert.pem --key=new-key.pem -n hsk-production --dry-run=client -o yaml | kubectl apply -f -
kubectl rollout restart deployment/hsk-verifier -n hsk-production
```

## Communication

### Internal Security Channel

```
🔒 SECURITY INCIDENT DECLARED
Type: [Data Breach/Credential Compromise/Malicious Activity]
Severity: P0
Time Detected: [Timestamp]
Incident Commander: [Name]
Status: Containment in progress
Next Update: [15 minutes]
Confidential - Do not discuss outside #incident-security
```

### Legal/Compliance Notification

- GDPR: Notify within 72 hours of discovery
- SOC2: Document in incident log
- Customer notification: After legal review

## Post-Incident

### Immediate (within 1 hour)
1. Secure all evidence
2. Document timeline
3. Identify root cause
4. Implement temporary fixes

### Within 24 hours
1. Complete forensic analysis
2. File security incident report
3. Update security controls
4. Schedule post-mortem

### Within 1 week
1. Implement permanent fixes
2. Update security policies
3. Conduct security training
4. Review and update playbooks

## Contact Information

| Role | Contact |
|------|---------|
| CISO | security@hsk.platform |
| Legal | legal@hsk.platform |
| Compliance | compliance@hsk.platform |
| On-call Security | +1-555-SECURITY |
