#!/bin/bash
# HSK Platform Restore Script
# Restores PostgreSQL databases and Kubernetes resources from backup

set -euo pipefail

# Configuration
NAMESPACE="${NAMESPACE:-hsk-verifier}"
BACKUP_DIR="${BACKUP_DIR:-/var/backups/hsk}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
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

usage() {
    echo "Usage: $0 <backup_timestamp>"
    echo ""
    echo "Example: $0 20250115_120000"
    exit 1
}

# Verify backup
verify_backup() {
    local backup_path="$1"
    
    log_info "Verifying backup at ${backup_path}..."
    
    if [ ! -d "$backup_path" ]; then
        log_error "Backup directory not found: $backup_path"
        exit 1
    fi
    
    # Check manifest exists
    local manifest=$(find "$backup_path" -name "manifest_*.json" | head -1)
    if [ -z "$manifest" ]; then
        log_error "Backup manifest not found"
        exit 1
    fi
    
    # Verify checksums
    log_info "Verifying file checksums..."
    local files=$(jq -r '.files[]' "$manifest")
    
    for file in $files; do
        local expected=$(jq -r ".checksums[\"$file\"]" "$manifest")
        local actual=$(sha256sum "${backup_path}/${file}" | awk '{print $1}')
        
        if [ "$expected" != "$actual" ]; then
            log_error "Checksum mismatch for $file"
            exit 1
        fi
    done
    
    log_info "Backup verification complete"
}

# Restore PostgreSQL
restore_postgres() {
    local backup_path="$1"
    
    log_info "Restoring PostgreSQL databases..."
    
    # Get database pod
    local db_pod=$(kubectl get pod -n "${NAMESPACE}" -l app=transparency-db -o jsonpath='{.items[0].metadata.name}')
    
    if [ -z "$db_pod" ]; then
        log_error "Database pod not found"
        return 1
    fii
    
    # Restore transparency database
    local transparency_backup=$(find "$backup_path" -name "transparency_*.sql.gz" | head -1)
    if [ -n "$transparency_backup" ]; then
        log_info "Restoring transparency database..."
        
        # Drop and recreate database
        kubectl exec -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U transparency -c "DROP DATABASE IF EXISTS transparency;"
        kubectl exec -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U transparency -c "CREATE DATABASE transparency;"
        
        # Restore
        gunzip -c "$transparency_backup" | kubectl exec -i -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U transparency
        
        log_info "Transparency database restored"
    fi
    
    # Restore consent_ledger database
    local consent_backup=$(find "$backup_path" -name "consent_ledger_*.sql.gz" | head -1)
    if [ -n "$consent_backup" ]; then
        log_info "Restoring consent_ledger database..."
        
        kubectl exec -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U consent -c "DROP DATABASE IF EXISTS consent_ledger;"
        kubectl exec -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U consent -c "CREATE DATABASE consent_ledger;"
        
        gunzip -c "$consent_backup" | kubectl exec -i -n "${NAMESPACE}" "${db_pod}" -- \
            psql -U consent -d consent_ledger
        
        log_info "Consent_ledger database restored"
    fi
}

# Restore secrets
restore_secrets() {
    local backup_path="$1"
    
    log_info "Restoring Kubernetes secrets..."
    
    local secrets_file=$(find "$backup_path" -name "secrets_*.yaml" | head -1)
    if [ -n "$secrets_file" ]; then
        kubectl apply -f "$secrets_file" -n "${NAMESPACE}"
        log_info "Secrets restored"
    fi
}

# Restore ConfigMaps
restore_configmaps() {
    local backup_path="$1"
    
    log_info "Restoring ConfigMaps..."
    
    local cm_file=$(find "$backup_path" -name "configmaps_*.yaml" | head -1)
    if [ -n "$cm_file" ]; then
        kubectl apply -f "$cm_file" -n "${NAMESPACE}"
        log_info "ConfigMaps restored"
    fi
}

# Download from S3 (if needed)
download_from_s3() {
    local timestamp="$1"
    
    if [ -n "${S3_BUCKET:-}" ]; then
        log_info "Downloading backup from S3..."
        
        local local_path="${BACKUP_DIR}/${timestamp}"
        mkdir -p "$local_path"
        
        aws s3 sync "s3://${S3_BUCKET}/backups/${timestamp}/" "$local_path/"
        
        echo "$local_path"
    fi
}

# Main execution
main() {
    if [ $# -lt 1 ]; then
        usage
    fi
    
    local timestamp="$1"
    local backup_path="${BACKUP_DIR}/${timestamp}"
    
    log_info "HSK Platform Restore Starting..."
    log_info "Restoring from backup: ${timestamp}"
    
    # Check prerequisites
    if ! command -v kubectl &> /dev/null; then
        log_error "kubectl not found"
        exit 1
    fi
    
    # Download from S3 if not local
    if [ ! -d "$backup_path" ] && [ -n "${S3_BUCKET:-}" ]; then
        backup_path=$(download_from_s3 "$timestamp")
    fi
    
    # Confirm restore
    echo ""
    log_warn "This will restore the HSK platform to backup: ${timestamp}"
    log_warn "Current data will be overwritten!"
    echo ""
    read -p "Are you sure you want to continue? (yes/no): " confirm
    
    if [ "$confirm" != "yes" ]; then
        log_info "Restore cancelled"
        exit 0
    fi
    
    # Verify and restore
    verify_backup "$backup_path"
    restore_postgres "$backup_path"
    restore_secrets "$backup_path"
    restore_configmaps "$backup_path"
    
    # Restart deployments
    log_info "Restarting deployments..."
    kubectl rollout restart deployment/hs-verifier -n "${NAMESPACE}"
    kubectl rollout restart statefulset/transparency-log -n "${NAMESPACE}"
    
    # Wait for rollout
    kubectl rollout status deployment/hs-verifier -n "${NAMESPACE}"
    kubectl rollout status statefulset/transparency-log -n "${NAMESPACE}"
    
    log_info "Restore complete!"
}

main "$@"
