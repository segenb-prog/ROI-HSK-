-- RI-0 Transparency Log Database Schema
-- PostgreSQL 15+
-- Purpose: Cryptographically verifiable append-only log of HSK certificates

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE SCHEMA IF NOT EXISTS transparency;

-- =====================================================
-- CORE TABLES
-- =====================================================

-- Certificate entries (the actual log)
CREATE TABLE transparency.certificates (
    id BIGSERIAL PRIMARY KEY,
    certificate_id VARCHAR(64) UNIQUE NOT NULL,  -- SHA256 of certificate
    system_id VARCHAR(255) NOT NULL,
    
    -- Certificate content
    hs_compliant BOOLEAN NOT NULL,
    violations JSONB DEFAULT '[]',
    missing_proofs JSONB DEFAULT '[]',
    invalid_proofs JSONB DEFAULT '[]',
    evaluation_time TIMESTAMPTZ NOT NULL,
    
    -- Full certificate (for retrieval)
    certificate_json JSONB NOT NULL,
    
    -- Issuer information
    issuer_public_key BYTEA NOT NULL,
    issuer_signature BYTEA NOT NULL,
    
    -- Merkle tree integration
    merkle_root VARCHAR(64) NOT NULL,
    merkle_proof JSONB,  -- Path from leaf to root
    leaf_index BIGINT NOT NULL,
    
    -- Hash chain (links to previous entry)
    previous_hash VARCHAR(64) NOT NULL,
    
    -- Timestamps
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified_at TIMESTAMPTZ,
    
    -- Server that accepted this entry
    server_id VARCHAR(255) NOT NULL
);

-- Indexes for efficient queries
CREATE INDEX idx_certificates_system ON transparency.certificates(system_id);
CREATE INDEX idx_certificates_compliant ON transparency.certificates(hs_compliant) WHERE hs_compliant = FALSE;
CREATE INDEX idx_certificates_time ON transparency.certificates(evaluation_time);
CREATE INDEX idx_certificates_root ON transparency.certificates(merkle_root);
CREATE INDEX idx_certificates_submitted ON transparency.certificates(submitted_at);

-- Composite index for violation queries
CREATE INDEX idx_certificates_violations ON transparency.certificates(system_id, evaluation_time) 
    WHERE hs_compliant = FALSE;

-- =====================================================
-- MERKLE TREE TABLES
-- =====================================================

-- Merkle tree nodes (for efficient proof generation)
CREATE TABLE transparency.merkle_nodes (
    id BIGSERIAL PRIMARY KEY,
    level INTEGER NOT NULL,      -- Tree level (0 = leaves)
    position BIGINT NOT NULL,    -- Position at this level
    node_hash VARCHAR(64) NOT NULL,
    left_child VARCHAR(64),      -- Hash of left child (NULL for leaves)
    right_child VARCHAR(64),     -- Hash of right child (NULL for leaves)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    UNIQUE(level, position)
);

CREATE INDEX idx_merkle_level ON transparency.merkle_nodes(level);
CREATE INDEX idx_merkle_position ON transparency.merkle_nodes(position);
CREATE INDEX idx_merkle_hash ON transparency.merkle_nodes(node_hash);

-- Current tree state
CREATE TABLE transparency.tree_state (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),  -- Singleton
    current_root VARCHAR(64) NOT NULL,
    tree_size BIGINT NOT NULL DEFAULT 0,
    last_leaf_index BIGINT NOT NULL DEFAULT -1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO transparency.tree_state (current_root, tree_size) 
VALUES ('0000000000000000000000000000000000000000000000000000000000000000', 0)
ON CONFLICT DO NOTHING;

-- =====================================================
-- LOG HEADS (for gossip protocol)
-- =====================================================

-- Signed tree heads from all servers
CREATE TABLE transparency.signed_tree_heads (
    id BIGSERIAL PRIMARY KEY,
    server_id VARCHAR(255) NOT NULL,
    tree_size BIGINT NOT NULL,
    merkle_root VARCHAR(64) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    signature BYTEA NOT NULL,
    
    UNIQUE(server_id, tree_size)
);

CREATE INDEX idx_sth_server ON transparency.signed_tree_heads(server_id);
CREATE INDEX idx_sth_time ON transparency.signed_tree_heads(timestamp);

-- Gossip messages (cross-server verification)
CREATE TABLE transparency.gossip_messages (
    id BIGSERIAL PRIMARY KEY,
    from_server VARCHAR(255) NOT NULL,
    to_server VARCHAR(255) NOT NULL,
    merkle_root VARCHAR(64) NOT NULL,
    timestamp TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    signature BYTEA NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    verified BOOLEAN DEFAULT FALSE
);

CREATE INDEX idx_gossip_from ON transparency.gossip_messages(from_server);
CREATE INDEX idx_gossip_time ON transparency.gossip_messages(timestamp);

-- =====================================================
-- MONITORING & AUDIT
-- =====================================================

-- Audit log for all operations
CREATE TABLE transparency.audit_log (
    id BIGSERIAL PRIMARY KEY,
    operation VARCHAR(50) NOT NULL,
    certificate_id VARCHAR(64),
    system_id VARCHAR(64),
    details JSONB,
    performed_by VARCHAR(255),
    performed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    ip_address INET
);

CREATE INDEX idx_audit_operation ON transparency.audit_log(operation);
CREATE INDEX idx_audit_time ON transparency.audit_log(performed_at);

-- Sync status between servers
CREATE TABLE transparency.sync_status (
    server_id VARCHAR(255) PRIMARY KEY,
    last_sync_at TIMESTAMPTZ,
    their_root VARCHAR(64),
    our_root VARCHAR(64),
    in_sync BOOLEAN DEFAULT FALSE,
    drift_count BIGINT DEFAULT 0
);

-- =====================================================
-- FUNCTIONS
-- =====================================================

-- Compute leaf hash for a certificate
CREATE OR REPLACE FUNCTION transparency.compute_leaf_hash(
    p_certificate_id VARCHAR,
    p_previous_hash VARCHAR
) RETURNS VARCHAR AS $$
BEGIN
    RETURN encode(
        digest(
            decode(p_certificate_id, 'hex') || 
            decode(p_previous_hash, 'hex'),
            'sha256'
        ),
        'hex'
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Compute parent hash from two children
CREATE OR REPLACE FUNCTION transparency.compute_parent_hash(
    p_left VARCHAR,
    p_right VARCHAR
) RETURNS VARCHAR AS $$
BEGIN
    RETURN encode(
        digest(
            decode(p_left, 'hex') || 
            decode(p_right, 'hex'),
            'sha256'
        ),
        'hex'
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Add a new certificate to the log
CREATE OR REPLACE FUNCTION transparency.add_certificate(
    p_certificate_id VARCHAR,
    p_system_id VARCHAR,
    p_hs_compliant BOOLEAN,
    p_violations JSONB,
    p_missing_proofs JSONB,
    p_invalid_proofs JSONB,
    p_evaluation_time TIMESTAMPTZ,
    p_certificate_json JSONB,
    p_issuer_public_key BYTEA,
    p_issuer_signature BYTEA,
    p_server_id VARCHAR
) RETURNS TABLE (
    position BIGINT,
    merkle_root VARCHAR,
    inclusion_proof JSONB
) AS $$
DECLARE
    v_state transparency.tree_state%ROWTYPE;
    v_leaf_hash VARCHAR;
    v_new_root VARCHAR;
    v_proof JSONB;
BEGIN
    -- Get current tree state
    SELECT * INTO v_state FROM transparency.tree_state WHERE id = 1;
    
    -- Compute leaf hash
    v_leaf_hash := transparency.compute_leaf_hash(
        p_certificate_id,
        v_state.current_root
    );
    
    -- Insert certificate
    INSERT INTO transparency.certificates (
        certificate_id, system_id, hs_compliant, violations,
        missing_proofs, invalid_proofs, evaluation_time,
        certificate_json, issuer_public_key, issuer_signature,
        merkle_root, leaf_index, previous_hash, server_id
    ) VALUES (
        p_certificate_id, p_system_id, p_hs_compliant, p_violations,
        p_missing_proofs, p_invalid_proofs, p_evaluation_time,
        p_certificate_json, p_issuer_public_key, p_issuer_signature,
        v_leaf_hash, v_state.tree_size, v_state.current_root, p_server_id
    )
    RETURNING leaf_index INTO position;
    
    -- Update tree state
    UPDATE transparency.tree_state SET
        current_root = v_leaf_hash,
        tree_size = v_state.tree_size + 1,
        last_leaf_index = position,
        updated_at = NOW()
    WHERE id = 1;
    
    -- Generate inclusion proof (simplified - full implementation would compute actual path)
    v_proof := jsonb_build_array(
        jsonb_build_object('hash', v_state.current_root, 'is_left', true)
    );
    
    -- Return results
    merkle_root := v_leaf_hash;
    inclusion_proof := v_proof;
    
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

-- Verify inclusion of a certificate in the log
CREATE OR REPLACE FUNCTION transparency.verify_inclusion(
    p_certificate_id VARCHAR,
    p_merkle_root VARCHAR
) RETURNS TABLE (
    found BOOLEAN,
    position BIGINT,
    verified BOOLEAN,
    error TEXT
) AS $$
DECLARE
    v_cert transparency.certificates%ROWTYPE;
    v_current_hash VARCHAR;
BEGIN
    -- Find certificate
    SELECT * INTO v_cert 
    FROM transparency.certificates 
    WHERE certificate_id = p_certificate_id;
    
    IF NOT FOUND THEN
        found := FALSE;
        position := NULL;
        verified := FALSE;
        error := 'Certificate not found in log';
        RETURN NEXT;
        RETURN;
    END IF;
    
    found := TRUE;
    position := v_cert.leaf_index;
    
    -- Verify merkle root matches
    IF v_cert.merkle_root = p_merkle_root THEN
        verified := TRUE;
        error := NULL;
    ELSE
        verified := FALSE;
        error := 'Merkle root mismatch';
    END IF;
    
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

-- Get consistency proof between two tree sizes
CREATE OR REPLACE FUNCTION transparency.get_consistency_proof(
    p_first_size BIGINT,
    p_second_size BIGINT
) RETURNS TABLE (
    proof_nodes JSONB,
    verified BOOLEAN
) AS $$
BEGIN
    -- Simplified implementation
    -- Full implementation would compute actual consistency proof
    proof_nodes := '[]'::JSONB;
    verified := (p_first_size <= p_second_size);
    
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

-- Gossip sync: compare and reconcile with another server
CREATE OR REPLACE FUNCTION transparency.gossip_sync(
    p_server_id VARCHAR,
    p_their_root VARCHAR,
    p_their_size BIGINT
) RETURNS TABLE (
    in_sync BOOLEAN,
    action VARCHAR,
    details JSONB
) AS $$
DECLARE
    v_our_root VARCHAR;
    v_our_size BIGINT;
BEGIN
    -- Get our current state
    SELECT current_root, tree_size 
    INTO v_our_root, v_our_size
    FROM transparency.tree_state WHERE id = 1;
    
    -- Update sync status
    INSERT INTO transparency.sync_status (server_id, their_root, our_root, in_sync)
    VALUES (p_server_id, p_their_root, v_our_root, v_our_root = p_their_root)
    ON CONFLICT (server_id) DO UPDATE SET
        last_sync_at = NOW(),
        their_root = p_their_root,
        our_root = v_our_root,
        in_sync = (v_our_root = p_their_root),
        drift_count = CASE 
            WHEN v_our_root = p_their_root THEN 0 
            ELSE transparency.sync_status.drift_count + 1 
        END;
    
    -- Determine action
    IF v_our_root = p_their_root THEN
        in_sync := TRUE;
        action := 'none';
        details := jsonb_build_object('message', 'Logs in sync');
    ELSIF p_their_size > v_our_size THEN
        in_sync := FALSE;
        action := 'fetch';
        details := jsonb_build_object(
            'our_size', v_our_size,
            'their_size', p_their_size,
            'fetch_count', p_their_size - v_our_size
        );
    ELSE
        in_sync := FALSE;
        action := 'investigate';
        details := jsonb_build_object(
            'our_root', v_our_root,
            'their_root', p_their_root,
            'warning', 'Possible fork detected'
        );
    END IF;
    
    RETURN NEXT;
END;
$$ LANGUAGE plpgsql;

-- =====================================================
-- VIEWS
-- =====================================================

-- Recent violations
CREATE VIEW transparency.recent_violations AS
SELECT 
    certificate_id,
    system_id,
    evaluation_time,
    violations,
    missing_proofs,
    invalid_proofs,
    merkle_root,
    submitted_at
FROM transparency.certificates
WHERE hs_compliant = FALSE
ORDER BY submitted_at DESC
LIMIT 100;

-- System compliance summary
CREATE VIEW transparency.system_compliance AS
SELECT 
    system_id,
    COUNT(*) FILTER (WHERE hs_compliant = TRUE) AS compliant_count,
    COUNT(*) FILTER (WHERE hs_compliant = FALSE) AS violation_count,
    COUNT(*) AS total_count,
    MAX(evaluation_time) AS last_evaluation
FROM transparency.certificates
GROUP BY system_id;

-- Daily statistics
CREATE VIEW transparency.daily_stats AS
SELECT 
    DATE(submitted_at) AS date,
    COUNT(*) AS total_entries,
    COUNT(*) FILTER (WHERE hs_compliant = FALSE) AS violations,
    COUNT(DISTINCT system_id) AS systems_evaluated
FROM transparency.certificates
GROUP BY DATE(submitted_at)
ORDER BY date DESC;

-- =====================================================
-- PERMISSIONS
-- =====================================================

CREATE ROLE transparency_log_app WITH LOGIN PASSWORD 'change_me_in_production';

GRANT USAGE ON SCHEMA transparency TO transparency_log_app;
GRANT SELECT, INSERT ON ALL TABLES IN SCHEMA transparency TO transparency_log_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA transparency TO transparency_log_app;

-- Read-only role for auditors
CREATE ROLE transparency_auditor WITH LOGIN PASSWORD 'change_me_in_production';
GRANT USAGE ON SCHEMA transparency TO transparency_auditor;
GRANT SELECT ON ALL TABLES IN SCHEMA transparency TO transparency_auditor;

COMMENT ON SCHEMA transparency IS 'Cryptographically verifiable transparency log for HSK certificates';
COMMENT ON TABLE transparency.certificates IS 'Append-only log of HSK compliance certificates';
