import { NativeModules, Platform } from 'react-native';

const { HSKSDK } = NativeModules;

/**
 * HSK SDK Configuration
 */
export interface HSKConfig {
  /** Base URL of the HSK platform API */
  baseUrl: string;
  /** API key for authentication */
  apiKey: string;
  /** Optional: Request timeout in milliseconds (default: 30000) */
  timeout?: number;
  /** Optional: Enable debug logging */
  debug?: boolean;
}

/**
 * Identity information
 */
export interface HSKIdentity {
  /** Decentralized identifier */
  did: string;
  /** Creation timestamp */
  createdAt: string;
}

/**
 * Consent request parameters
 */
export interface ConsentRequest {
  /** Purpose of data processing */
  purpose: string;
  /** Categories of data being consented to */
  dataCategories: string[];
  /** Optional: Number of days until expiration (default: 365) */
  expiresInDays?: number;
  /** Optional: Specific constraints on data usage */
  constraints?: Record<string, any>;
  /** Optional: Scope of consent (default: 'general') */
  scope?: string;
}

/**
 * Granted consent information
 */
export interface HSKConsent {
  /** Unique consent identifier */
  consentId: string;
  /** Merkle root for verification */
  merkleRoot: string;
  /** Timestamp of consent creation */
  timestamp: string;
  /** Transaction hash (if applicable) */
  txHash?: string;
}

/**
 * Consent verification result
 */
export interface ConsentVerification {
  /** Whether the consent is valid */
  valid: boolean;
  /** Verification timestamp */
  verifiedAt: string;
  /** Merkle proof for verification */
  merkleProof?: string;
}

/**
 * Consent summary for listing
 */
export interface ConsentSummary {
  consentId: string;
  purpose: string;
  status: 'active' | 'revoked' | 'expired';
  createdAt: string;
  expiresAt?: string;
}

/**
 * Authentication result
 */
export interface AuthResult {
  success: boolean;
  method: string;
}

/**
 * HSK SDK for React Native
 * 
 * Provides cryptographic identity management and consent operations
 * for the Human Sovereignty Kernel platform.
 * 
 * @example
 * ```typescript
 * import HSKSDK from '@hsk/sdk-react-native';
 * 
 * await HSKSDK.initialize({
 *   baseUrl: 'https://api.hsk.platform',
 *   apiKey: 'your-api-key'
 * });
 * 
 * const identity = await HSKSDK.createIdentity();
 * const consent = await HSKSDK.grantConsent({
 *   purpose: 'analytics',
 *   dataCategories: ['usage_data']
 * });
 * ```
 */
class HSKSDKClass {
  private initialized = false;
  private config?: HSKConfig;

  /**
   * Initialize the SDK with configuration
   * 
   * @param config - SDK configuration
   * @returns Promise resolving to initialization result
   * @throws Error if initialization fails
   */
  async initialize(config: HSKConfig): Promise<{ success: boolean; version: string }> {
    if (this.initialized) {
      throw new Error('SDK already initialized');
    }

    if (!config.baseUrl || !config.apiKey) {
      throw new Error('baseUrl and apiKey are required');
    }

    const result = await HSKSDK.initialize({
      baseUrl: config.baseUrl.replace(/\/$/, ''), // Remove trailing slash
      apiKey: config.apiKey,
      timeout: config.timeout || 30000,
      debug: config.debug || false,
    });

    this.initialized = true;
    this.config = config;

    if (config.debug) {
      console.log('[HSK SDK] Initialized successfully');
    }

    return result;
  }

  /**
   * Create a new cryptographic identity
   * 
   * Generates an Ed25519 key pair and registers the identity with the HSK platform.
   * The private key is securely stored in the device's keychain/keystore.
   * 
   * @returns Promise resolving to the created identity
   * @throws Error if identity creation fails
   */
  async createIdentity(): Promise<HSKIdentity> {
    this.ensureInitialized();
    return HSKSDK.createIdentity();
  }

  /**
   * Grant consent for data processing
   * 
   * Creates a cryptographically signed consent record that is stored
   * in the HSK transparency ledger.
   * 
   * @param request - Consent request parameters
   * @returns Promise resolving to the granted consent
   * @throws Error if consent grant fails
   */
  async grantConsent(request: ConsentRequest): Promise<HSKConsent> {
    this.ensureInitialized();
    
    if (!request.purpose || !request.dataCategories?.length) {
      throw new Error('purpose and dataCategories are required');
    }

    return HSKSDK.grantConsent({
      purpose: request.purpose,
      dataCategories: request.dataCategories,
      expiresInDays: request.expiresInDays,
      constraints: request.constraints,
      scope: request.scope,
    });
  }

  /**
   * Revoke a previously granted consent
   * 
   * Creates a deletion proof in the transparency ledger.
   * 
   * @param consentId - ID of the consent to revoke
   * @returns Promise resolving to revocation confirmation
   * @throws Error if revocation fails
   */
  async revokeConsent(consentId: string): Promise<{ consentId: string; revokedAt: string }> {
    this.ensureInitialized();
    
    if (!consentId) {
      throw new Error('consentId is required');
    }

    return HSKSDK.revokeConsent(consentId);
  }

  /**
   * Verify a consent record's integrity
   * 
   * Verifies the consent against the Merkle tree and hash chain.
   * 
   * @param consentId - ID of the consent to verify
   * @returns Promise resolving to verification result
   * @throws Error if verification fails
   */
  async verifyConsent(consentId: string): Promise<ConsentVerification> {
    this.ensureInitialized();
    
    if (!consentId) {
      throw new Error('consentId is required');
    }

    return HSKSDK.verifyConsent(consentId);
  }

  /**
   * Get all consents for the current identity
   * 
   * @returns Promise resolving to list of consents
   * @throws Error if fetch fails
   */
  async getMyConsents(): Promise<{ consents: ConsentSummary[]; total: number }> {
    this.ensureInitialized();
    return HSKSDK.getMyConsents();
  }

  /**
   * Authenticate using biometric (Face ID / Touch ID / Fingerprint)
   * 
   * @returns Promise resolving to authentication result
   * @throws Error if biometric authentication fails or is unavailable
   */
  async authenticateWithBiometric(): Promise<AuthResult> {
    this.ensureInitialized();
    return HSKSDK.authenticateWithBiometric();
  }

  /**
   * Export all data associated with the current identity
   * 
   * @returns Promise resolving to JSON string of all user data
   * @throws Error if export fails
   */
  async exportMyData(): Promise<string> {
    this.ensureInitialized();
    return HSKSDK.exportMyData();
  }

  /**
   * Delete the current identity and all associated data
   * 
   * This action is irreversible. All consents will be revoked
   * and the identity will be permanently deleted.
   * 
   * @returns Promise resolving to deletion confirmation
   * @throws Error if deletion fails
   */
  async deleteMyIdentity(): Promise<{ deleted: boolean; deletedAt: string }> {
    this.ensureInitialized();
    return HSKSDK.deleteMyIdentity();
  }

  /**
   * Check if the SDK has been initialized
   */
  isInitialized(): boolean {
    return this.initialized;
  }

  /**
   * Get the current SDK configuration
   */
  getConfig(): HSKConfig | undefined {
    return this.config;
  }

  private ensureInitialized(): void {
    if (!this.initialized) {
      throw new Error('HSK SDK not initialized. Call initialize() first.');
    }
  }
}

// Export singleton instance
export const HSKSDKInstance = new HSKSDKClass();
export default HSKSDKInstance;

// Re-export types
export {
  HSKConfig,
  HSKIdentity,
  ConsentRequest,
  HSKConsent,
  ConsentVerification,
  ConsentSummary,
  AuthResult,
};
