# HSK Platform Vault Policies

# Admin policy - full access
path "*" {
  capabilities = ["create", "read", "update", "delete", "list", "sudo"]
}

# Database secrets engine policy
path "database/*" {
  capabilities = ["create", "read", "update", "delete", "list"]
}

path "database/creds/hsk-app" {
  capabilities = ["read"]
}

# Transit encryption policy
path "transit/encrypt/hsk-*" {
  capabilities = ["update"]
}

path "transit/decrypt/hsk-*" {
  capabilities = ["update"]
}

path "transit/sign/hsk-*" {
  capabilities = ["update"]
}

path "transit/verify/hsk-*" {
  capabilities = ["update"]
}

# PKI certificate policy
path "pki/issue/hsk-*" {
  capabilities = ["create", "update"]
}

path "pki/roles/hsk-*" {
  capabilities = ["read"]
}

# KV secrets policy
path "secret/data/hsk/*" {
  capabilities = ["create", "read", "update", "delete"]
}

path "secret/metadata/hsk/*" {
  capabilities = ["list"]
}

# Audit log access
path "sys/audit" {
  capabilities = ["read", "sudo"]
}

# Health check (no auth required in some configs)
path "sys/health" {
  capabilities = ["read"]
}

---
# App-specific policy (for HSK services)
path "database/creds/hsk-app" {
  capabilities = ["read"]
}

path "transit/encrypt/hsk-consent" {
  capabilities = ["update"]
}

path "transit/decrypt/hsk-consent" {
  capabilities = ["update"]
}

path "secret/data/hsk/app/*" {
  capabilities = ["read"]
}

path "pki/issue/hsk-service" {
  capabilities = ["create", "update"]
}

---
# Read-only policy (for monitoring)
path "sys/metrics" {
  capabilities = ["read"]
}

path "sys/health" {
  capabilities = ["read"]
}

path "sys/seal-status" {
  capabilities = ["read"]
}
