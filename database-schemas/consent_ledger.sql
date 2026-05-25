-- RI-0 Consent Ledger Database Schema
-- PostgreSQL 15+
-- Purpose: Tamper-evident storage of consent grants, revocations, and amendments

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- Schema for consent ledger
CREATE SCHEMA IF NOT EXISTS consent_ledger;

-- =====================================================
-- CORE TABLES
-- =====================================================

-- Citizens table (linked to Digital Identity System)
CREATE TABLE consent_ledger.citizens (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    did VARCHAR(255) UNIQUE NOT NULL,  -- Decentralized Identifier
    public_key BYTEA NOT NULL,         -- Ed25519 public key (32 bytes)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB DEFAULT '{}'
);

CREATE INDEX idx_citizens_did ON consent_ledger.citizens(did);

-- Consent entries (the actual ledger)
CREATE TABLE consent_ledger.entries (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    entry_id VARCHAR(64) UNIQUE NOT NULL,  -- SHA256 hash of entry content (hex)
    citizen_id UUID NOT NULL REFERENCES consent_ledger.citizens(id),
    
    -- Consent action
    action VARCHAR(20) NOT NULL CHECK (action IN ('grant', 'revoke', 'amend')),
    
    -- Scope of consent
    scope JSONB NOT NULL,  -- Array of resource identifiers
    purpose TEXT NOT NULL, -- Why the data is being accessed
    
    -- Temporal constraints
    duration_seconds BIGINT NOT NULL CHECK (duration_seconds >= 0),
    granted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    
    -- Additional constraints
    constraints JSONB DEFAULT '{}',  -- e.g., {"no_derivatives": true}
    
    -- Cryptographic verification
    previous_entry_id VARCHAR(64) NOT NULL,  -- Hash chain link
    public_key BYTEA NOT NULL,               -- Citizen's public key at time of signing
    signature BYTEA NOT NULL,                -- Ed25519 signature (64 bytes)
    
    -- System metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    system_signature BYTEA,  -- Optional: system attestation
    
    -- Ensure chronological ordering
    CONSTRAINT valid_temporal CHECK (action = 'revoke' OR expires_at > granted_at)
);

-- Indexes for efficient queries
CREATE INDEX idx_entries_citizen ON consent_ledger.entries(citizen_id);
CREATE INDEX idx_entries_action ON consent_ledger.entries(action);
CREATE INDEX idx_entries_expires ON consent_ledger.entries(expires_at);
CREATE INDEX idx_entries_previous ON consent_ledger.entries(previous_entry_id);
CREATE INDEX idx_entries_created ON consent_ledger.entries(created_at);

-- Composite index for active consents
-- Volatile functions such as NOW() are not allowed in PostgreSQL partial-index predicates.
-- Use a stable partial predicate and keep the time filter in queries/functions.
CREATE INDEX idx_entries_active ON consent_ledger.entries(citizen_id, action, expires_at)
    WHERE action = 'grant';

-- =====================================================
-- AUDIT & VERIFICATION TABLES
-- =====================================================

-- Hash chain verification state
CREATE TABLE consent_ledger.hash_chain (
    citizen_id UUID PRIMARY KEY REFERENCES consent_ledger.citizens(id),
    latest_entry_id VARCHAR(64) NOT NULL,
    latest_hash BYTEA NOT NULL,  -- 32 bytes
    entry_count BIGINT NOT NULL DEFAULT 0,
    last_verified_at TIMESTAMPTZ,
    chain_valid BOOLEAN DEFAULT TRUE
);

-- Verification audit log
CREATE TABLE consent_ledger.verification_log (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    citizen_id UUID REFERENCES consent_ledger.citizens(id),
    entry_id UUID REFERENCES consent_ledger.entries(id),
    verified_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verification_type VARCHAR(50) NOT NULL,
    result BOOLEAN NOT NULL,
    error_message TEXT,
    verifier_id VARCHAR(255)  -- Which system performed verification
);

CREATE INDEX idx_verification_log_citizen ON consent_ledger.verification_log(citizen_id);
CREATE INDEX idx_verification_log_entry ON consent_ledger.verification_log(entry_id);
CREATE INDEX idx_verification_log_time ON consent_ledger.verification_log(verified_at);

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Compute entry hash (matches Rust implementation)
CREATE OR REPLACE FUNCTION consent_ledger.compute_entry_hash(
    p_previous_hash BYTEA,
    p_timestamp TIMESTAMPTZ,
    p_action VARCHAR,
    p_scope JSONB,
    p_purpose TEXT,
    p_duration BIGINT,
    p_constraints JSONB,
    p_public_key BYTEA
) RETURNS BYTEA AS $$
BEGIN
    RETURN digest(
        p_previous_hash ||
        p_timestamp::TEXT::BYTEA ||
        p_action::BYTEA ||
        p_scope::TEXT::BYTEA ||
        p_purpose::BYTEA ||
        p_duration::TEXT::BYTEA ||
        p_constraints::TEXT::BYTEA ||
        p_public_key,
        'sha256'
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Verify hash chain integrity for a citizen
CREATE OR REPLACE FUNCTION consent_ledger.verify_chain(p_citizen_id UUID)
RETURNS TABLE (
    entry_uuid UUID,
    entry_hash VARCHAR,
    computed_hash VARCHAR,
    valid BOOLEAN,
    error TEXT
) AS $$
DECLARE
    v_entry RECORD;
    v_previous_hash BYTEA := '\x0000000000000000000000000000000000000000000000000000000000000000';
    v_computed_hash BYTEA;
BEGIN
    FOR v_entry IN 
        SELECT * FROM consent_ledger.entries 
        WHERE citizen_id = p_citizen_id 
        ORDER BY created_at ASC
    LOOP
        v_computed_hash := consent_ledger.compute_entry_hash(
            v_previous_hash,
            v_entry.granted_at,
            v_entry.action,
            v_entry.scope,
            v_entry.purpose,
            v_entry.duration_seconds,
            v_entry.constraints,
            v_entry.public_key
        );
        
        entry_uuid := v_entry.id;
        entry_hash := v_entry.entry_id;
        computed_hash := encode(v_computed_hash, 'hex');
        valid := (v_entry.entry_id = computed_hash);
        error := CASE 
            WHEN valid THEN NULL 
            ELSE 'Hash mismatch: expected ' || computed_hash || ', got ' || v_entry.entry_id 
        END;
        
        RETURN NEXT;
        
        IF NOT valid THEN
            EXIT;
        END IF;
        
        v_previous_hash := v_computed_hash;
    END LOOP;
END;
$$ LANGUAGE plpgsql;

-- Get active consents for a citizen
CREATE OR REPLACE FUNCTION consent_ledger.get_active_consents(p_citizen_did VARCHAR)
RETURNS TABLE (
    entry_id VARCHAR,
    scope JSONB,
    purpose TEXT,
    expires_at TIMESTAMPTZ,
    constraints JSONB
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        e.entry_id,
        e.scope,
        e.purpose,
        e.expires_at,
        e.constraints
    FROM consent_ledger.entries e
    JOIN consent_ledger.citizens c ON e.citizen_id = c.id
    WHERE c.did = p_citizen_did
        AND e.action = 'grant'
        AND e.expires_at > NOW()
        AND NOT EXISTS (
            -- Check if there's a revocation
            SELECT 1 FROM consent_ledger.entries e2
            WHERE e2.previous_entry_id = e.entry_id
                AND e2.action = 'revoke'
        )
    ORDER BY e.granted_at DESC;
END;
$$ LANGUAGE plpgsql;

-- Check if a specific data access is consented
CREATE OR REPLACE FUNCTION consent_ledger.is_access_consented(
    p_citizen_did VARCHAR,
    p_resource VARCHAR,
    p_purpose VARCHAR
) RETURNS BOOLEAN AS $$
DECLARE
    v_count INTEGER;
BEGIN
    SELECT COUNT(*) INTO v_count
    FROM consent_ledger.get_active_consents(p_citizen_did)
    WHERE 
        -- Check if resource is in scope
        scope @> jsonb_build_array(p_resource)
        -- Check if purpose matches
        AND (purpose = p_purpose OR purpose = 'any');
    
    RETURN v_count > 0;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- TRIGGERS
-- =====================================================

-- Update hash chain state on new entry
CREATE OR REPLACE FUNCTION consent_ledger.update_hash_chain()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO consent_ledger.hash_chain (
        citizen_id, latest_entry_id, latest_hash, entry_count
    )
    VALUES (
        NEW.citizen_id,
        NEW.entry_id,
        decode(NEW.entry_id, 'hex'),
        1
    )
    ON CONFLICT (citizen_id) DO UPDATE SET
        latest_entry_id = NEW.entry_id,
        latest_hash = decode(NEW.entry_id, 'hex'),
        entry_count = consent_ledger.hash_chain.entry_count + 1,
        last_verified_at = NULL;
    
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trigger_update_hash_chain
    AFTER INSERT ON consent_ledger.entries
    FOR EACH ROW
    EXECUTE FUNCTION consent_ledger.update_hash_chain();

-- =====================================================
-- INITIAL DATA
-- =====================================================

-- Insert genesis entry (system initialization)
INSERT INTO consent_ledger.citizens (did, public_key)
VALUES ('did:hsk:system', '\x0000000000000000000000000000000000000000000000000000000000000000')
ON CONFLICT (did) DO NOTHING;

-- =====================================================
-- PERMISSIONS
-- =====================================================

-- Create application role
CREATE ROLE consent_ledger_app WITH LOGIN PASSWORD 'change_me_in_production';

GRANT USAGE ON SCHEMA consent_ledger TO consent_ledger_app;
GRANT SELECT, INSERT ON ALL TABLES IN SCHEMA consent_ledger TO consent_ledger_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA consent_ledger TO consent_ledger_app;

-- Revoke dangerous permissions
REVOKE DELETE ON consent_ledger.entries FROM consent_ledger_app;
REVOKE UPDATE ON consent_ledger.entries FROM consent_ledger_app;
REVOKE TRUNCATE ON consent_ledger.entries FROM consent_ledger_app;

COMMENT ON SCHEMA consent_ledger IS 'Tamper-evident consent ledger for HSK compliance';
COMMENT ON TABLE consent_ledger.entries IS 'Cryptographically linked consent entries. DO NOT MODIFY DIRECTLY - use API.';
