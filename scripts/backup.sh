#!/bin/bash
# HSK Platform Backup Script
# Backs up PostgreSQL databases and Kubernetes secrets

set -euo pipefail

# Configuration
BACKUP_DIR="${BACKUP_DIR:-/var/backups/hsk}"
RETENTION_DAYS="${RETENTION_DAYS:-30}"
NAMESPACE="${NAMESPACE:-hsk-verifier}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Create backup directory
mkdir -p "${BACKUP_DIR}/${TIMESTAMP}"

cd "${BACKUP_DIR}/${TIMESTAMP}"

log_info "Starting backup at ${TIMESTAMP}"

# Backup PostgreSQL databases
backup_postgres() {
    log_info "Backing up PostgreSQL databases..."
    
    # Get database pod
    DB_POD=$(kubectl get pod -n "${NAMESPACE}" -l app=transparency-db -o jsonpath='{.items[0].metadata.name}')
    
    if [ -z "$DB_POD" ]; then
        log_error "Database pod not found"
        return 1
    fi
    
    # Backup transparency database
    kubectl exec -n "${NAMESPACE}" "${DB_POD}" -- \
        pg_dump -U transparency transparency > "transparency_${TIMESTAMP}.sql"
    
    # Backup consent_ledger database
    kubectl exec -n "${NAMESPACE}" "${DB_POD}" -- \
        pg_dump -U consent consent_ledger > "consent_ledger_${TIMESTAMP}.sql"
    
    # Compress
    gzip "transparency_${TIMESTAMP}.sql"
    gzip "consent_ledger_${TIMESTAMP}.sql"
    
    log_info "Database backup complete"
}

# Backup Kubernetes secrets
backup_secrets() {
    log_info "Backing up Kubernetes secrets..."
    
    kubectl get secrets -n "${NAMESPACE}" -o yaml > "secrets_${TIMESTAMP}.yaml"
    
    log_info "Secrets backup complete"
}

# Backup ConfigMaps
backup_configmaps() {
    log_info "Backing up ConfigMaps..."
    
    kubectl get configmaps -n "${NAMESPACE}" -o yaml > "configmaps_${TIMESTAMP}.yaml"
    
    log_info "ConfigMaps backup complete"
}

# Backup certificates
backup_certificates() {
    log_info "Backing up certificates from transparency log..."
    
    # Query certificates and save as JSON
    kubectl exec -n "${NAMESPACE}" -c log-server \
        $(kubectl get pod -n "${NAMESPACE}" -l app=transparency-log -o jsonpath='{.items[0].metadata.name}') -- \
        curl -s http://localhost:8080/certificates > "certificates_${TIMESTAMP}.json"
    
    log_info "Certificates backup complete"
}

# Create backup manifest
create_manifest() {
    log_info "Creating backup manifest..."
    
    cat > "manifest_${TIMESTAMP}.json" <<EOF
{
    "backup_timestamp": "${TIMESTAMP}",
    "namespace": "${NAMESPACE}",
    "files": [
        "transparency_${TIMESTAMP}.sql.gz",
        "consent_ledger_${TIMESTAMP}.sql.gz",
        "secrets_${TIMESTAMP}.yaml",
        "configmaps_${TIMESTAMP}.yaml",
        "certificates_${TIMESTAMP}.json"
    ],
    "checksums": {}
}
EOF
    
    # Add checksums
    for file in *.gz *.yaml *.json; do
        if [ -f "$file" ]; then
            checksum=$(sha256sum "$file" | awk '{print $1}')
            jq --arg file "$file" --arg checksum "$checksum" \
                '.checksums[$file] = $checksum' "manifest_${TIMESTAMP}.json" > tmp.json
            mv tmp.json "manifest_${TIMESTAMP}.json"
        fi
    done
    
    log_info "Manifest created"
}

# Upload to remote storage (optional)
upload_backup() {
    if [ -n "${S3_BUCKET:-}" ]; then
        log_info "Uploading backup to S3..."
        
        aws s3 sync "${BACKUP_DIR}/${TIMESTAMP}" "s3://${S3_BUCKET}/backups/${TIMESTAMP}/"
        
        log_info "Upload complete"
    fi
}

# Clean up old backups
cleanup_old_backups() {
    log_info "Cleaning up backups older than ${RETENTION_DAYS} days..."
    
    find "${BACKUP_DIR}" -maxdepth 1 -type d -mtime +${RETENTION_DAYS} -exec rm -rf {} \; 2>/dev/null || true
    
    if [ -n "${S3_BUCKET:-}" ]; then
        aws s3 ls "s3://${S3_BUCKET}/backups/" | \
            awk '{print $2}' | \
            xargs -I {} aws s3 rm --recursive "s3://${S3_BUCKET}/backups/{}" 2>/dev/null || true
    fi
    
    log_info "Cleanup complete"
}

# Main execution
main() {
    log_info "HSK Platform Backup Starting..."
    
    # Check prerequisites
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl not found"
        exit 1
    fi
    
    if ! command -v jq &> /dev/null; then
        log_error "jq not found"
        exit 1
    fi
    
    # Run backups
    backup_postgres
    backup_secrets
    backup_configmaps
    backup_certificates
    create_manifest
    upload_backup
    cleanup_old_backups
    
    log_info "Backup complete: ${BACKUP_DIR}/${TIMESTAMP}"
    log_info "Backup size: $(du -sh "${BACKUP_DIR}/${TIMESTAMP}" | awk '{print $1}')"
}

# Handle signals
trap 'log_error "Backup interrupted"; exit 1' INT TERM

main "$@"
