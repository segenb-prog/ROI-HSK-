#!/bin/bash
# Check for exposed secrets in code

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/../.."

echo "Scanning for exposed secrets..."
echo ""

SECRETS_FOUND=0

# Patterns to search for
PATTERNS=(
    'password\s*=\s*["\'][^${}]+["\']'
    'api_key\s*=\s*["\'][^${}]+["\']'
    'secret\s*=\s*["\'][^${}]+["\']'
    'token\s*=\s*["\'][^${}]+["\']'
    'AWS_ACCESS_KEY_ID\s*=\s*["\'][^${}]+["\']'
    'AWS_SECRET_ACCESS_KEY\s*=\s*["\'][^${}]+["\']'
    'PRIVATE_KEY\s*=\s*["\'][^${}]+["\']'
    '-----BEGIN PRIVATE KEY-----'
    '-----BEGIN RSA PRIVATE KEY-----'
    'sk-[a-zA-Z0-9]{48}'  # Stripe keys
    'ghp_[a-zA-Z0-9]{36}'  # GitHub tokens
)

EXCLUDE_PATTERNS=(
    'example'
    'placeholder'
    'CHANGEME'
    'YOUR_'
    'TODO'
    'password = "${'
    'secretKeyRef'
    'valueFrom'
)

for pattern in "${PATTERNS[@]}"; do
    echo "Checking pattern: $pattern"
    
    matches=$(grep -r -E "$pattern" \
        --include="*.yaml" \
        --include="*.yml" \
        --include="*.json" \
        --include="*.sh" \
        --include="*.py" \
        --include="*.rs" \
        --include="*.swift" \
        --include="*.kt" \
        --include="*.tsx" \
        --include="*.dart" \
        . 2>/dev/null | grep -v node_modules | grep -v target | grep -v ".git" || true)
    
    if [ -n "$matches" ]; then
        # Filter out excluded patterns
        filtered_matches=""
        while IFS= read -r line; do
            exclude=0
            for exclude_pattern in "${EXCLUDE_PATTERNS[@]}"; do
                if echo "$line" | grep -q "$exclude_pattern"; then
                    exclude=1
                    break
                fi
            done
            if [ $exclude -eq 0 ]; then
                filtered_matches="$filtered_matches$line"
            fi
        done <<< "$matches"
        
        if [ -n "$filtered_matches" ]; then
            echo -e "${RED}Potential secret found:${NC}"
            echo "$filtered_matches"
            SECRETS_FOUND=1
        fi
    fi
done

echo ""
if [ $SECRETS_FOUND -eq 0 ]; then
    echo -e "${GREEN}No exposed secrets detected!${NC}"
    exit 0
else
    echo -e "${RED}Potential secrets were found. Please review.${NC}"
    exit 1
fi
