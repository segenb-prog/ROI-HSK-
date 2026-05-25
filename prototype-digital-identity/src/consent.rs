// Consent management utilities

use crate::models::*;
use chrono::{DateTime, Utc, Duration};

/// Check if a consent entry is currently active
pub fn is_consent_active(entry: &ConsentEntry) -> bool {
    if entry.action != "grant" {
        return false;
    }
    
    let now = Utc::now();
    entry.granted_at <= now && entry.expires_at > now
}

/// Check if consent has been revoked
pub fn is_consent_revoked(entry: &ConsentEntry, all_entries: &[ConsentEntry]) -> bool {
    // Find if there's a revocation entry that references this entry
    all_entries.iter().any(|e| {
        e.action == "revoke" && 
        e.purpose.contains(&entry.entry_id)
    })
}

/// Get effective consents (granted but not revoked and not expired)
pub fn get_effective_consents(entries: &[ConsentEntry]) -> Vec<&ConsentEntry> {
    entries
        .iter()
        .filter(|e| {
            e.action == "grant" &&
            is_consent_active(e) &&
            !is_consent_revoked(e, entries)
        })
        .collect()
}

/// Check if a resource is in scope
pub fn resource_in_scope(resource: &str, scope: &serde_json::Value) -> bool {
    if let Some(arr) = scope.as_array() {
        arr.iter().any(|v| {
            v.as_str().map(|s| s == resource).unwrap_or(false)
        })
    } else {
        false
    }
}

/// Check if purpose matches
pub fn purpose_matches(requested: &str, allowed: &str) -> bool {
    allowed == "any" || allowed == requested
}

/// Check constraints
pub fn check_constraints(
    constraints: &Option<serde_json::Value>,
    check_no_derivatives: bool,
) -> bool {
    if let Some(constraints) = constraints {
        if check_no_derivatives {
            if let Some(no_deriv) = constraints.get("no_derivatives") {
                return no_deriv.as_bool().unwrap_or(false);
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_resource_in_scope() {
        let scope = json!(["resource1", "resource2", "resource3"]);
        
        assert!(resource_in_scope("resource1", &scope));
        assert!(resource_in_scope("resource2", &scope));
        assert!(!resource_in_scope("resource4", &scope));
    }

    #[test]
    fn test_purpose_matches() {
        assert!(purpose_matches("analytics", "analytics"));
        assert!(purpose_matches("analytics", "any"));
        assert!(!purpose_matches("analytics", "marketing"));
    }
}
