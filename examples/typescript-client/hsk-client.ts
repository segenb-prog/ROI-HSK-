/**
 * HSK Falsification Machine TypeScript Client
 * 
 * Official TypeScript/JavaScript SDK for the HSK Falsification Machine API.
 * Compatible with OpenAPI 3.0 specification.
 * 
 * @example
 * ```typescript
 * import { HSKClient, ConsentLedgerClient } from './hsk-client';
 * 
 * // HSK Verifier client
 * const hsk = new HSKClient('https://verifier.hskernel.dev', { apiKey: 'your-api-key' });
 * 
 * // Challenge a system
 * const challenge = await hsk.challenge('my-ai-system', { timeoutHours: 72 });
 * console.log(`Challenge ID: ${challenge.requestId}`);
 * 
 * // Submit response
 * const result = await hsk.submitResponse({ requestId, systemId, providedProofs });
 * console.log(`Status: ${result.status}`);
 * ```
 */

export type ProofType = 'ConsentLedger' | 'MemoryPassport' | 'DeletionProof' | 'PredictionScope';

export interface Challenge {
  requestId: string;
  systemId: string;
  deadline: Date;
  requestedProofs: ProofType[];
  issuedAt?: Date;
  nonce?: string;
}

export interface EvaluationResult {
  status: 'compliant' | 'violation';
  certificateId?: string;
  reason?: string;
  missingProofs: ProofType[];
  invalidProofs: ProofType[];
}

export interface Certificate {
  certificateId: string;
  systemId: string;
  evaluationTime: Date;
  hsCompliant: boolean;
  violations: string[];
  missingProofs: string[];
  invalidProofs: string[];
  issuerPublicKey?: string;
  issuerSignature?: string;
}

export interface TransparencyEntry {
  certificateId: string;
  systemId: string;
  timestamp: Date;
  compliant: boolean;
  merkleRoot?: string;
  position?: number;
}

export interface Citizen {
  id: string;
  did: string;
  publicKey: string;
  createdAt: Date;
}

export interface ConsentEntry {
  entryId: string;
  action: 'grant' | 'revoke' | 'amend';
  scope: string[];
  purpose: string;
  durationSeconds: number;
  grantedAt: Date;
  expiresAt: Date;
  previousEntryId: string;
  publicKey: string;
  signature: string;
  systemSignature?: string;
}

export interface HSKProofs {
  citizenDid: string;
  publicKey: string;
  entryCount: number;
  entries: ConsentEntry[];
  proofType: string;
  latestEntryId?: string;
}

export class HSKError extends Error {
  constructor(
    message: string,
    public statusCode?: number,
    public response?: Record<string, unknown>
  ) {
    super(message);
    this.name = 'HSKError';
  }
}

export interface HSKClientOptions {
  apiKey?: string;
  timeout?: number;
  verifySsl?: boolean;
}

export interface ChallengeOptions {
  timeoutHours?: number;
}

export interface ListCertificatesOptions {
  systemId?: string;
  compliant?: boolean;
  limit?: number;
}

export interface QueryTransparencyOptions {
  certificateId?: string;
  systemId?: string;
  startTime?: Date;
  endTime?: Date;
}

/**
 * Client for the HSK Falsification Machine API
 */
export class HSKClient {
  private baseUrl: string;
  private apiKey?: string;
  private timeout: number;
  private verifySsl: boolean;

  constructor(baseUrl: string, options: HSKClientOptions = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.apiKey = options.apiKey;
    this.timeout = options.timeout ?? 30000;
    this.verifySsl = options.verifySsl ?? true;
  }

  private async request<T>(
    method: string,
    path: string,
    options: { params?: Record<string, unknown>; body?: unknown } = {}
  ): Promise<T> {
    const url = new URL(path, this.baseUrl);
    
    if (options.params) {
      Object.entries(options.params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.append(key, String(value));
        }
      });
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'Accept': 'application/json',
      'User-Agent': 'hsk-typescript-client/0.1.0',
    };

    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`;
    }

    try {
      const response = await fetch(url.toString(), {
        method,
        headers,
        body: options.body ? JSON.stringify(options.body) : undefined,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new HSKError(
          errorData.error || `HTTP ${response.status}`,
          response.status,
          errorData
        );
      }

      return await response.json() as T;
    } catch (error) {
      if (error instanceof HSKError) {
        throw error;
      }
      throw new HSKError(`Request failed: ${error}`);
    }
  }

  /**
   * Check the health of the HSK verifier server
   */
  async healthCheck(): Promise<{ status: string; keyId?: string; timestamp?: string }> {
    return this.request('GET', '/health');
  }

  /**
   * Create a new challenge for a system
   */
  async challenge(systemId: string, options: ChallengeOptions = {}): Promise<Challenge> {
    const data = await this.request<{
      requestId: string;
      systemId: string;
      deadline: string;
      requestedProofs: ProofType[];
      issuedAt?: string;
      nonce?: string;
    }>('POST', '/challenge', {
      body: {
        system_id: systemId,
        timeout_hours: options.timeoutHours ?? 72,
      },
    });

    return {
      requestId: data.requestId,
      systemId: data.systemId,
      deadline: new Date(data.deadline),
      requestedProofs: data.requestedProofs,
      issuedAt: data.issuedAt ? new Date(data.issuedAt) : undefined,
      nonce: data.nonce,
    };
  }

  /**
   * Get challenge details by request ID
   */
  async getChallenge(requestId: string): Promise<Challenge> {
    const data = await this.request<{
      requestId: string;
      systemId: string;
      deadline: string;
      requestedProofs: ProofType[];
    }>('GET', `/challenge/${requestId}`);

    return {
      requestId: data.requestId,
      systemId: data.systemId,
      deadline: new Date(data.deadline),
      requestedProofs: data.requestedProofs,
    };
  }

  /**
   * Submit a system's response to a challenge
   */
  async submitResponse(params: {
    requestId: string;
    systemId: string;
    providedProofs: Array<{ proofType: ProofType; data: string }>;
    submittedAt?: Date;
  }): Promise<EvaluationResult> {
    const data = await this.request<{
      status: 'compliant' | 'violation';
      certificateId?: string;
      reason?: string;
      missingProofs?: ProofType[];
      invalidProofs?: ProofType[];
    }>('POST', '/response', {
      body: {
        request_id: params.requestId,
        system_id: params.systemId,
        provided_proofs: params.providedProofs,
        submitted_at: (params.submittedAt ?? new Date()).toISOString(),
      },
    });

    return {
      status: data.status,
      certificateId: data.certificateId,
      reason: data.reason,
      missingProofs: data.missingProofs ?? [],
      invalidProofs: data.invalidProofs ?? [],
    };
  }

  /**
   * List issued certificates
   */
  async listCertificates(options: ListCertificatesOptions = {}): Promise<Certificate[]> {
    const data = await this.request<Array<{
      certificateId: string;
      systemId: string;
      evaluationTime: string;
      hsCompliant: boolean;
      violations?: string[];
      missingProofs?: string[];
      invalidProofs?: string[];
    }>>('GET', '/certificates', {
      params: {
        system_id: options.systemId,
        compliant: options.compliant,
        limit: Math.min(options.limit ?? 100, 1000),
      },
    });

    return data.map(c => ({
      certificateId: c.certificateId,
      systemId: c.systemId,
      evaluationTime: new Date(c.evaluationTime),
      hsCompliant: c.hsCompliant,
      violations: c.violations ?? [],
      missingProofs: c.missingProofs ?? [],
      invalidProofs: c.invalidProofs ?? [],
    }));
  }

  /**
   * Get certificate details by ID
   */
  async getCertificate(certId: string): Promise<Certificate> {
    const data = await this.request<{
      certificateId: string;
      systemId: string;
      evaluationTime: string;
      hsCompliant: boolean;
      violations?: string[];
      missingProofs?: string[];
      invalidProofs?: string[];
    }>('GET', `/certificates/${certId}`);

    return {
      certificateId: data.certificateId,
      systemId: data.systemId,
      evaluationTime: new Date(data.evaluationTime),
      hsCompliant: data.hsCompliant,
      violations: data.violations ?? [],
      missingProofs: data.missingProofs ?? [],
      invalidProofs: data.invalidProofs ?? [],
    };
  }

  /**
   * Verify a certificate's signature
   */
  async verifyCertificate(certId: string): Promise<{ certificateId: string; valid: boolean; systemId: string; compliant: boolean }> {
    return this.request('GET', `/verify/${certId}`);
  }

  /**
   * Submit a certificate to a transparency log
   */
  async submitToTransparencyLog(
    certificate: Record<string, unknown>,
    logUrl: string
  ): Promise<{ status: string; position?: number; merkleRoot?: string }> {
    return this.request('POST', '/transparency/submit', {
      body: { certificate, log_url: logUrl },
    });
  }

  /**
   * Query the transparency log
   */
  async queryTransparencyLog(options: QueryTransparencyOptions = {}): Promise<TransparencyEntry[]> {
    const data = await this.request<Array<{
      certificateId: string;
      systemId: string;
      timestamp: string;
      compliant: boolean;
      merkleRoot?: string;
      position?: number;
    }>>('GET', '/transparency/query', {
      params: {
        certificate_id: options.certificateId,
        system_id: options.systemId,
        start_time: options.startTime?.toISOString(),
        end_time: options.endTime?.toISOString(),
      },
    });

    return data.map(e => ({
      certificateId: e.certificateId,
      systemId: e.systemId,
      timestamp: new Date(e.timestamp),
      compliant: e.compliant,
      merkleRoot: e.merkleRoot,
      position: e.position,
    }));
  }
}

export interface ConsentLedgerOptions {
  timeout?: number;
  verifySsl?: boolean;
}

export interface GrantConsentOptions {
  citizenDid: string;
  scope: string[];
  purpose: string;
  durationSeconds: number;
  citizenSignature: string;
  constraints?: Record<string, unknown>;
}

export interface RevokeConsentOptions {
  citizenDid: string;
  entryIdToRevoke: string;
  citizenSignature: string;
}

export interface CheckAccessOptions {
  citizenDid: string;
  resource: string;
  purpose: string;
}

/**
 * Client for the Digital Identity + Consent Ledger API
 */
export class ConsentLedgerClient {
  private baseUrl: string;
  private timeout: number;
  private verifySsl: boolean;

  constructor(baseUrl: string, options: ConsentLedgerOptions = {}) {
    this.baseUrl = baseUrl.replace(/\/$/, '');
    this.timeout = options.timeout ?? 30000;
    this.verifySsl = options.verifySsl ?? true;
  }

  private async request<T>(
    method: string,
    path: string,
    options: { params?: Record<string, unknown>; body?: unknown } = {}
  ): Promise<T> {
    const url = new URL(path, this.baseUrl);
    
    if (options.params) {
      Object.entries(options.params).forEach(([key, value]) => {
        if (value !== undefined && value !== null) {
          url.searchParams.append(key, String(value));
        }
      });
    }

    const headers: Record<string, string> = {
      'Content-Type': 'application/json',
      'Accept': 'application/json',
      'User-Agent': 'hsk-typescript-client/0.1.0',
    };

    try {
      const response = await fetch(url.toString(), {
        method,
        headers,
        body: options.body ? JSON.stringify(options.body) : undefined,
      });

      if (!response.ok) {
        const errorData = await response.json().catch(() => ({}));
        throw new HSKError(
          errorData.error || `HTTP ${response.status}`,
          response.status,
          errorData
        );
      }

      return await response.json() as T;
    } catch (error) {
      if (error instanceof HSKError) {
        throw error;
      }
      throw new HSKError(`Request failed: ${error}`);
    }
  }

  /**
   * Check the health of the consent ledger server
   */
  async healthCheck(): Promise<{ status: string; database?: string; timestamp?: string }> {
    return this.request('GET', '/health');
  }

  /**
   * Register a new citizen
   */
  async registerCitizen(did: string, publicKey: string): Promise<Citizen> {
    const data = await this.request<{
      id: string;
      did: string;
      publicKey: string;
      createdAt: string;
    }>('POST', '/citizens', {
      body: { did, public_key: publicKey },
    });

    return {
      id: data.id,
      did: data.did,
      publicKey: data.publicKey,
      createdAt: new Date(data.createdAt),
    };
  }

  /**
   * Get citizen information by DID
   */
  async getCitizen(did: string): Promise<Citizen> {
    const data = await this.request<{
      id: string;
      did: string;
      publicKey: string;
      createdAt: string;
    }>('GET', `/citizens/${did}`);

    return {
      id: data.id,
      did: data.did,
      publicKey: data.publicKey,
      createdAt: new Date(data.createdAt),
    };
  }

  /**
   * Get all consent entries for a citizen
   */
  async getCitizenConsents(did: string): Promise<ConsentEntry[]> {
    const data = await this.request<Array<{
      entryId: string;
      action: 'grant' | 'revoke' | 'amend';
      scope: string[];
      purpose: string;
      durationSeconds: number;
      grantedAt: string;
      expiresAt: string;
      previousEntryId: string;
      publicKey: string;
      signature: string;
      systemSignature?: string;
    }>>('GET', `/citizens/${did}/consents`);

    return data.map(e => ({
      entryId: e.entryId,
      action: e.action,
      scope: e.scope,
      purpose: e.purpose,
      durationSeconds: e.durationSeconds,
      grantedAt: new Date(e.grantedAt),
      expiresAt: new Date(e.expiresAt),
      previousEntryId: e.previousEntryId,
      publicKey: e.publicKey,
      signature: e.signature,
      systemSignature: e.systemSignature,
    }));
  }

  /**
   * Grant consent for data access
   */
  async grantConsent(options: GrantConsentOptions): Promise<ConsentEntry> {
    const data = await this.request<{
      entryId: string;
      action: 'grant' | 'revoke' | 'amend';
      scope: string[];
      purpose: string;
      durationSeconds: number;
      grantedAt: string;
      expiresAt: string;
      previousEntryId: string;
      publicKey: string;
      signature: string;
      systemSignature?: string;
    }>('POST', '/consent/grant', {
      body: {
        citizen_did: options.citizenDid,
        scope: options.scope,
        purpose: options.purpose,
        duration_seconds: options.durationSeconds,
        citizen_signature: options.citizenSignature,
        constraints: options.constraints,
      },
    });

    return {
      entryId: data.entryId,
      action: data.action,
      scope: data.scope,
      purpose: data.purpose,
      durationSeconds: data.durationSeconds,
      grantedAt: new Date(data.grantedAt),
      expiresAt: new Date(data.expiresAt),
      previousEntryId: data.previousEntryId,
      publicKey: data.publicKey,
      signature: data.signature,
      systemSignature: data.systemSignature,
    };
  }

  /**
   * Revoke a previously granted consent
   */
  async revokeConsent(options: RevokeConsentOptions): Promise<ConsentEntry> {
    const data = await this.request<{
      entryId: string;
      action: 'grant' | 'revoke' | 'amend';
      scope: string[];
      purpose: string;
      durationSeconds: number;
      grantedAt: string;
      expiresAt: string;
      previousEntryId: string;
      publicKey: string;
      signature: string;
      systemSignature?: string;
    }>('POST', '/consent/revoke', {
      body: {
        citizen_did: options.citizenDid,
        entry_id_to_revoke: options.entryIdToRevoke,
        citizen_signature: options.citizenSignature,
      },
    });

    return {
      entryId: data.entryId,
      action: data.action,
      scope: data.scope,
      purpose: data.purpose,
      durationSeconds: data.durationSeconds,
      grantedAt: new Date(data.grantedAt),
      expiresAt: new Date(data.expiresAt),
      previousEntryId: data.previousEntryId,
      publicKey: data.publicKey,
      signature: data.signature,
      systemSignature: data.systemSignature,
    };
  }

  /**
   * Verify a consent entry's signature
   */
  async verifyConsentEntry(entryId: string): Promise<{ valid: boolean; entryId: string; error?: string }> {
    return this.request('GET', `/consent/verify/${entryId}`);
  }

  /**
   * Verify the hash chain for a citizen
   */
  async verifyCitizenChain(did: string): Promise<{
    citizenDid: string;
    valid: boolean;
    entryCount: number;
    invalidEntries?: string[];
  }> {
    return this.request('GET', `/verify/chain/${did}`);
  }

  /**
   * Check if a specific access is consented
   */
  async checkAccess(options: CheckAccessOptions): Promise<{
    citizenDid: string;
    resource: string;
    purpose: string;
    consented: boolean;
  }> {
    return this.request('POST', '/verify/access', {
      body: {
        citizen_did: options.citizenDid,
        resource: options.resource,
        purpose: options.purpose,
      },
    });
  }

  /**
   * Get HSK proof package for a citizen
   */
  async getHskProofs(did: string): Promise<HSKProofs> {
    const data = await this.request<{
      citizenDid: string;
      publicKey: string;
      entryCount: number;
      entries: Array<{
        entryId: string;
        action: 'grant' | 'revoke' | 'amend';
        scope: string[];
        purpose: string;
        durationSeconds: number;
        grantedAt: string;
        expiresAt: string;
        previousEntryId: string;
        publicKey: string;
        signature: string;
        systemSignature?: string;
      }>;
      proofType: string;
      latestEntryId?: string;
    }>('GET', `/hsk/proofs/${did}`);

    return {
      citizenDid: data.citizenDid,
      publicKey: data.publicKey,
      entryCount: data.entryCount,
      entries: data.entries.map(e => ({
        entryId: e.entryId,
        action: e.action,
        scope: e.scope,
        purpose: e.purpose,
        durationSeconds: e.durationSeconds,
        grantedAt: new Date(e.grantedAt),
        expiresAt: new Date(e.expiresAt),
        previousEntryId: e.previousEntryId,
        publicKey: e.publicKey,
        signature: e.signature,
        systemSignature: e.systemSignature,
      })),
      proofType: data.proofType,
      latestEntryId: data.latestEntryId,
    };
  }
}

// Export all types
export * from './hsk-client';

// Example usage
async function example() {
  console.log('HSK TypeScript Client Example');
  console.log('=' .repeat(50));

  try {
    const hsk = new HSKClient('http://localhost:8081');
    
    console.log('\n1. HSK Verifier Health Check:');
    const health = await hsk.healthCheck();
    console.log(`   Status: ${health.status}`);
    console.log(`   Key ID: ${health.keyId}`);

    console.log('\n2. Create Challenge:');
    const challenge = await hsk.challenge('test-system', { timeoutHours: 48 });
    console.log(`   Request ID: ${challenge.requestId}`);
    console.log(`   Deadline: ${challenge.deadline}`);
    console.log(`   Requested Proofs: ${challenge.requestedProofs.join(', ')}`);

  } catch (error) {
    if (error instanceof HSKError) {
      console.log(`   Error: ${error.message}`);
    } else {
      console.log(`   Error: ${error}`);
    }
  }

  try {
    const consent = new ConsentLedgerClient('http://localhost:8080');
    
    console.log('\n3. Consent Ledger Health Check:');
    const health = await consent.healthCheck();
    console.log(`   Status: ${health.status}`);

  } catch (error) {
    if (error instanceof HSKError) {
      console.log(`   Error: ${error.message}`);
    } else {
      console.log(`   Error: ${error}`);
    }
  }

  console.log('\n' + '='.repeat(50));
  console.log('Example complete!');
}

// Run example if executed directly
if (require.main === module) {
  example().catch(console.error);
}
