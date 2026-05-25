# HSK Security Runbook

This document provides procedures for handling security incidents in the HSK Falsification Machine platform.

---

## Table of Contents

1. [Incident Response](#incident-response)
2. [Key Compromise](#key-compromise)
3. [Database Breach](#database-breach)
4. [Transparency Log Attack](#transparency-log-attack)
5. [DDoS Response](#ddos-response)
6. [Post-Incident](#post-incident)

---

## Incident Response

### Severity Levels

| Level | Description | Response Time | Examples |
|-------|-------------|---------------|----------|
| P0 - Critical | Complete system compromise | 15 minutes | Key theft, database breach |
| P1 - High | Significant security impact | 1 hour | Log inconsistency, unauthorized access |
| P2 - Medium | Limited security impact | 4 hours | Failed verification attempts |
| P3 - Low | Minimal security impact | 24 hours | Minor configuration issues |

### Incident Response Team

- **On-Call Engineer**: First responder
- **Security Lead**: Technical decisions
- **Compliance Officer**: Regulatory requirements
- **Legal**: If data breach involves PII

### Initial Response (First 15 minutes)

1. **Acknowledge**: Page the on-call engineer
2. **Assess**: Determine severity and scope
3. **Contain**: Isolate affected systems
4. **Communicate**: Notify stakeholders

```bash
# Isolate a compromised pod
kubectl delete pod <pod-name> -n hsk-verifier --force

# Scale down affected deployment
kubectl scale deployment hs-verifier --replicas=0 -n hsk-verifier

# Enable emergency mode (deny all traffic)
kubectl apply -f emergency-network-policy.yaml
```

---

## Key Compromise

### Detection

- Alert: `HSKKeyCompromiseDetected`
- Symptoms: Invalid signatures, unexpected key usage

### Response

1. **Immediately revoke compromised key**
```bash
# Generate new keypair (air-gapped!)
hs-verifier generate-keys --output new-keyring.json --offline

# Update Kubernetes secret
kubectl create secret generic hsk-keyring-new \
  --from-file=keyring.json=new-keyring.json \
  -n hsk-verifier --dry-run=client -o yaml | kubectl apply -f -

# Rollout new keys
kubectl rollout restart deployment/hs-verifier -n hsk-verifier
```

2. **Re-sign all certificates since compromise**
```bash
# Query affected certificates
kubectl exec -n hsk-verifier deployment/hs-verifier -- \
  hs-verifier query --start-time "2025-01-01T00:00:00Z"

# Re-issue certificates
for cert in $(cat affected_certs.txt); do
  hs-verifier reissue --certificate $cert --keyring new-keyring.json
done
```

3. **Publish incident report to transparency logs**
```bash
# Create incident certificate
cat > incident.json <<EOF
{
  "type": "key_compromise",
  "compromised_key_id": "key-abc123",
  "new_key_id": "key-def456",
  "affected_certificates": 150,
  "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

hs-verifier submit-incident --file incident.json
```

---

## Database Breach

### Detection

- Alert: `DatabaseUnauthorizedAccess`
- Symptoms: Unexpected queries, large data exports

### Response

1. **Isolate database**
```bash
# Remove network access
kubectl apply -f - <<EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: database-isolate
  namespace: hsk-verifier
spec:
  podSelector:
    matchLabels:
      app: transparency-db
  policyTypes:
    - Ingress
    - Egress
EOF
```

2. **Create forensic snapshot**
```bash
# Create volume snapshot
kubectl apply -f - <<EOF
apiVersion: snapshot.storage.k8s.io/v1
kind: VolumeSnapshot
metadata:
  name: forensic-snapshot-$(date +%Y%m%d-%H%M%S)
  namespace: hsk-verifier
spec:
  volumeSnapshotClassName: csi-snapclass
  source:
    persistentVolumeClaimName: data-transparency-db-0
EOF
```

3. **Analyze access logs**
```bash
# Query audit log
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT * FROM transparency.audit_log 
    WHERE performed_at > NOW() - INTERVAL '24 hours'
    ORDER BY performed_at DESC;
  "
```

4. **Rotate all credentials**
```bash
# Generate new database password
NEW_PASSWORD=$(openssl rand -base64 32)

# Update secret
kubectl create secret generic transparency-db-credentials \
  --from-literal=username=transparency \
  --from-literal=password="$NEW_PASSWORD" \
  -n hsk-verifier --dry-run=client -o yaml | kubectl apply -f -

# Restart database to apply
kubectl rollout restart statefulset/transparency-db -n hsk-verifier
```

---

## Transparency Log Attack

### Detection

- Alert: `TransparencyLogOutOfSync`
- Symptoms: Different merkle roots across servers

### Response

1. **Stop accepting new entries**
```bash
# Enable read-only mode
kubectl set env deployment/transparency-log READ_ONLY=true -n hsk-verifier
```

2. **Compare log states**
```bash
# Get merkle roots from all servers
for i in 0 1 2; do
  kubectl exec -n hsk-verifier transparency-log-$i -- \
    curl -s http://localhost:8080/head | jq '.merkle_root'
done
```

3. **Identify compromised server**
```bash
# Check gossip messages
kubectl exec -n hsk-verifier deployment/transparency-db -- \
  psql -U transparency -c "
    SELECT from_server, to_server, verified, COUNT(*)
    FROM transparency.gossip_messages
    WHERE timestamp > NOW() - INTERVAL '1 hour'
    GROUP BY from_server, to_server, verified;
  "
```

4. **Rebuild from honest servers**
```bash
# Identify honest majority
HONEST_SERVER="transparency-log-0"

# Copy data from honest server
kubectl exec -n hsk-verifier $HONEST_SERVER -- \
  pg_dump -U transparency transparency | \
  kubectl exec -i -n hsk-verifier transparency-log-1 -- \
  psql -U transparency -c "DROP DATABASE transparency; CREATE DATABASE transparency;"

# Restart compromised servers
kubectl delete pod transparency-log-1 transparency-log-2 -n hsk-verifier
```

---

## DDoS Response

### Detection

- Alert: `HighRequestRate`
- Symptoms: Elevated latency, connection errors

### Response

1. **Enable rate limiting**
```bash
# Update ingress with stricter limits
kubectl annotate ingress hs-verifier \
  nginx.ingress.kubernetes.io/limit-rps=10 \
  nginx.ingress.kubernetes.io/limit-connections=5 \
  -n hsk-verifier
```

2. **Scale up**
```bash
# Increase replicas
kubectl scale deployment hs-verifier --replicas=20 -n hsk-verifier

# Enable cluster autoscaling
kubectl annotate deployment hs-verifier \
  cluster-autoscaler.kubernetes.io/safe-to-evict="false" \
  -n hsk-verifier
```

3. **Block malicious IPs**
```bash
# Add IP blacklist
kubectl apply -f - <<EOF
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: ddos-protection
  namespace: hsk-verifier
spec:
  podSelector:
    matchLabels:
      app: hs-verifier
  policyTypes:
    - Ingress
  ingress:
    - from:
        - ipBlock:
            cidr: 0.0.0.0/0
            except:
              - 192.0.2.0/24  # Blocked range
EOF
```

4. **Enable CloudFlare/WAF**
```bash
# Update DNS to use CloudFlare
# Configure WAF rules in CloudFlare dashboard
```

---

## Post-Incident

### Required Actions

1. **Document timeline**
   - When was the incident detected?
   - What actions were taken?
   - What was the root cause?

2. **Preserve evidence**
   - Save logs
   - Create snapshots
   - Document configuration changes

3. **Notify stakeholders**
   - Internal teams
   - Affected users (if required)
   - Regulators (if required by law)

4. **Post-mortem meeting**
   - Within 48 hours for P0/P1
   - Within 1 week for P2/P3

### Incident Report Template

```markdown
# Incident Report: [TITLE]

## Summary
- Date/Time: 
- Severity: 
- Duration: 
- Impact: 

## Timeline
- T+0:00 - Incident detected
- T+0:15 - On-call paged
- T+0:30 - Containment started
- T+1:00 - Incident resolved

## Root Cause
[Description]

## Actions Taken
1. [Action 1]
2. [Action 2]

## Lessons Learned
- [Lesson 1]
- [Lesson 2]

## Preventive Measures
- [Measure 1]
- [Measure 2]
```

---

## Contact Information

| Role | Contact | Pager |
|------|---------|-------|
| On-Call Engineer | oncall@hskernel.dev | +1-555-0100 |
| Security Lead | security@hskernel.dev | +1-555-0101 |
| Compliance Officer | compliance@hskernel.dev | +1-555-0102 |

---

## Appendix

### Useful Commands

```bash
# Get all logs
kubectl logs -n hsk-verifier -l app=hs-verifier --all-containers

# Get events
kubectl get events -n hsk-verifier --sort-by='.lastTimestamp'

# Check resource usage
kubectl top pods -n hsk-verifier

# Debug network
kubectl run debug --rm -it --image=nicolaka/netshoot --restart=Never -- bash
```

### Emergency Contacts

- **Cloud Provider**: [Support URL]
- **Kubernetes Support**: [Support URL]
- **Security Vendor**: [Support URL]
