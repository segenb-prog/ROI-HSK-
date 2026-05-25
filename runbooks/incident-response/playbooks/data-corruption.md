# Incident Response Playbook: Data Corruption

## Overview

| Field | Value |
|-------|-------|
| **Severity** | P1 - Critical |
| **Response Time** | 10 minutes |
| **Escalation Time** | 30 minutes |
| **Owner** | Database Team |

## Detection

### Symptoms

- Application errors referencing invalid data
- Database constraint violations
- Checksum mismatches
- User reports of incorrect data
- Monitoring alerts for data integrity

### Verification

```bash
# Check database health
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "SELECT pg_is_in_recovery(), pg_last_xact_replay_timestamp();"

# Run integrity checks
kubectl exec -n hsk-production deployment/hsk-verifier -- ./verify-data-integrity.sh

# Check for recent errors
kubectl logs -n hsk-production -l app=hsk-verifier | grep -i "integrity\|corrupt\|constraint"
```

## Assessment

### Determine Scope

```sql
-- Check for corrupted tables
SELECT 
    schemaname,
    tablename,
    n_tup_ins,
    n_tup_upd,
    n_tup_del
FROM pg_stat_user_tables
WHERE tablename LIKE 'hsk_%'
ORDER BY n_tup_upd DESC;

-- Check for recent errors in logs
SELECT 
    message,
    COUNT(*),
    MAX(log_time) as last_seen
FROM pg_log
WHERE log_time > NOW() - INTERVAL '1 hour'
    AND (message LIKE '%constraint%' OR message LIKE '%integrity%')
GROUP BY message;
```

### Identify Corruption Type

```sql
-- Foreign key violations
SELECT 
    tc.table_name,
    kcu.column_name,
    ccu.table_name AS foreign_table,
    ccu.column_name AS foreign_column
FROM information_schema.table_constraints tc
JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name
JOIN information_schema.constraint_column_usage ccu ON ccu.constraint_name = tc.constraint_name
WHERE tc.constraint_type = 'FOREIGN KEY';

-- Check for orphaned records
SELECT COUNT(*) FROM consent_entries ce
LEFT JOIN identities i ON ce.identity_id = i.id
WHERE i.id IS NULL;

-- Check for invalid signatures
SELECT COUNT(*) FROM consent_entries
WHERE signature IS NULL OR LENGTH(signature) != 128;
```

## Response Procedures

### Option 1: Point-in-Time Recovery (Recommended)

```bash
# 1. Identify clean restore point
# Check backup timestamps
aws s3 ls s3://hsk-backups/database/ | tail -20

# 2. Stop writes
kubectl patch configmap hsk-config -n hsk-production --patch '{"data":{"READ_ONLY_MODE":"true"}}'

# 3. Create current state backup (for forensic analysis)
pg_dump $DATABASE_URL | gzip > /tmp/corrupted-backup-$(date +%s).sql.gz

# 4. Restore to clean point
./scripts/restore-from-backup.sh --timestamp "2024-01-15T10:00:00Z"

# 5. Verify restoration
kubectl exec -n hsk-production deployment/hsk-verifier -- ./verify-data-integrity.sh

# 6. Re-enable writes
kubectl patch configmap hsk-config -n hsk-production --patch '{"data":{"READ_ONLY_MODE":"false"}}'
```

### Option 2: Manual Repair (for isolated corruption)

```sql
-- 1. Begin transaction
BEGIN;

-- 2. Fix orphaned records
DELETE FROM consent_entries
WHERE identity_id NOT IN (SELECT id FROM identities);

-- 3. Rebuild invalid signatures
UPDATE consent_entries
SET signature = hsk_sign_consent(identity_id, purpose, data_categories, valid_from, valid_until)
WHERE signature IS NULL OR hsk_verify_consent(id) = false;

-- 4. Verify fixes
SELECT COUNT(*) FROM consent_entries WHERE hsk_verify_consent(id) = false;

-- 5. Commit if verification passes
COMMIT;
-- Or ROLLBACK if issues remain
```

### Option 3: Replica Promotion (for primary corruption)

```bash
# 1. Stop replication
kubectl exec -n hsk-production deployment/postgres-replica -- pg_ctl stop -D /var/lib/postgresql/data

# 2. Promote replica
kubectl exec -n hsk-production deployment/postgres-replica -- touch /var/lib/postgresql/data/promote.signal

# 3. Update service endpoints
kubectl patch service postgres-primary -n hsk-production -p '{"spec":{"selector":{"role":"replica"}}}'

# 4. Update application connection strings
kubectl set env deployment/hsk-verifier DATABASE_URL="postgresql://replica:5432/hsk" -n hsk-production

# 5. Rebuild old primary as new replica
./scripts/rebuild-replica.sh --source replica --target primary
```

## Verification

### Data Integrity Checks

```bash
# Run full integrity verification
kubectl exec -n hsk-production deployment/hsk-verifier -- ./verify-data-integrity.sh --full

# Check Merkle tree consistency
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "
    SELECT 
        COUNT(*) as total_entries,
        COUNT(DISTINCT merkle_root) as unique_roots,
        MAX(created_at) as last_entry
    FROM consent_entries;
"

# Verify hash chain
kubectl exec -n hsk-production deployment/hsk-verifier -- psql $DATABASE_URL -c "
    SELECT 
        id,
        previous_hash,
        current_hash,
        hsk_verify_hash_chain(id) as valid
    FROM consent_entries
    ORDER BY created_at DESC
    LIMIT 100;
"
```

### Application Verification

```bash
# Run smoke tests
./scripts/smoke-tests.sh --environment production

# Verify API responses
curl -s https://api.hsk.platform/v1/health | jq .
curl -s https://api.hsk.platform/v1/consent/verify -H "Authorization: Bearer $TEST_TOKEN" | jq .

# Check user-facing functionality
./scripts/e2e-tests.sh --critical-only
```

## Prevention

### Enable Enhanced Checks

```sql
-- Enable data checksums
ALTER SYSTEM SET data_checksums = on;

-- Enable page verification
ALTER SYSTEM SET wal_log_hints = on;

-- Increase checkpoint frequency
ALTER SYSTEM SET checkpoint_timeout = '5min';
ALTER SYSTEM SET checkpoint_completion_target = 0.9;

-- Reload configuration
SELECT pg_reload_conf();
```

### Monitoring Enhancements

```yaml
# Add to prometheus rules
groups:
  - name: data-integrity
    rules:
      - alert: DataCorruptionDetected
        expr: hsk_data_integrity_check_failed > 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Data corruption detected"
          
      - alert: OrphanedRecords
        expr: hsk_orphaned_records_count > 0
        for: 5m
        labels:
          severity: warning
          
      - alert: InvalidSignatures
        expr: hsk_invalid_signature_count > 0
        for: 1m
        labels:
          severity: critical
```

## Post-Incident

### Analysis

1. **Root Cause Identification**
   - Review logs leading to corruption
   - Identify triggering event
   - Assess impact scope

2. **Data Loss Assessment**
   ```sql
   -- Calculate data loss window
   SELECT 
       MIN(created_at) as earliest_lost,
       MAX(created_at) as latest_lost,
       COUNT(*) as records_lost
   FROM consent_entries_backup
   WHERE created_at BETWEEN '[restore_point]' AND '[corruption_detected]';
   ```

3. **Recovery Effectiveness**
   - Time to detection
   - Time to recovery
   - Data completeness

### Documentation

1. Incident timeline
2. Root cause analysis
3. Recovery procedures used
4. Lessons learned
5. Preventive measures implemented

## Communication

```
📊 DATA CORRUPTION INCIDENT
Severity: P1
Impact: [X] records affected, [Y] users impacted
Detection: [Timestamp]
Resolution: [Timestamp]
Data Loss: [None/Minimal/Significant]
Status: Resolved
Next Steps: Post-mortem scheduled [Date/Time]
```
