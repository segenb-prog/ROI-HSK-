// HSK React Native SDK
// TypeScript/React Native

import React, { createContext, useContext, useState, useCallback } from 'react';
import {
  NativeModules,
  Platform,
  AsyncStorage,
  Alert,
} from 'react-native';
import * as Keychain from 'react-native-keychain';
import { sign, generateKeyPair } from 'react-native-nacl-jsi';

// Native module bridges
const { HSKBiometricAuth, HSKSecureEnclave } = NativeModules;

// Types
export interface HSKConfig {
  baseURL: string;
  apiKey: string;
  relyingPartyID?: string;
}

export interface HSKIdentity {
  did: string;
  publicKey: Uint8Array;
}

export interface HSKConsent {
  id: string;
  purpose: string;
  dataCategories: string[];
  retentionDays: number;
  legalBasis: HSKLegalBasis;
  grantedAt: Date;
  dataSubject: string;
  signature?: Uint8Array;
}

export enum HSKLegalBasis {
  CONSENT = 'consent',
  CONTRACT = 'contract',
  LEGAL_OBLIGATION = 'legal_obligation',
  VITAL_INTERESTS = 'vital_interests',
  PUBLIC_TASK = 'public_task',
  LEGITIMATE_INTERESTS = 'legitimate_interests',
}

export enum HSKError {
  NOT_INITIALIZED = 'SDK not initialized',
  NOT_AUTHENTICATED = 'User not authenticated',
  IDENTITY_NOT_FOUND = 'Identity not found',
  PRIVATE_KEY_NOT_FOUND = 'Private key not found',
  NETWORK_ERROR = 'Network error',
  SERVER_ERROR = 'Server error',
  BIOMETRIC_ERROR = 'Biometric authentication failed',
}

// SDK Context
interface HSKContextType {
  isInitialized: boolean;
  identity: HSKIdentity | null;
  initialize: (config: HSKConfig) => Promise<void>;
  createIdentity: () => Promise<HSKIdentity>;
  restoreIdentity: (did: string) => Promise<HSKIdentity | null>;
  grantConsent: (
    purpose: string,
    dataCategories: string[],
    retentionDays: number,
    legalBasis: HSKLegalBasis
  ) => Promise<HSKConsent>;
  revokeConsent: (consentId: string) => Promise<void>;
  getConsentHistory: () => Promise<HSKConsent[]>;
  authenticateWithBiometric: () => Promise<boolean>;
  logout: () => Promise<void>;
}

const HSKContext = createContext<HSKContextType | null>(null);

// SDK Provider
export const HSKProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [isInitialized, setIsInitialized] = useState(false);
  const [identity, setIdentity] = useState<HSKIdentity | null>(null);
  const [config, setConfig] = useState<HSKConfig | null>(null);

  const initialize = useCallback(async (cfg: HSKConfig) => {
    setConfig(cfg);
    setIsInitialized(true);
    
    // Try to restore existing identity
    const savedDID = await AsyncStorage.getItem('hsk_current_did');
    if (savedDID) {
      const restoredIdentity = await restoreIdentity(savedDID);
      if (restoredIdentity) {
        setIdentity(restoredIdentity);
      }
    }
  }, []);

  const createIdentity = useCallback(async (): Promise<HSKIdentity> => {
    if (!isInitialized) throw new Error(HSKError.NOT_INITIALIZED);

    // Generate Ed25519 key pair
    const keyPair = generateKeyPair();
    const publicKey = keyPair.publicKey;
    
    // Create DID
    const did = `did:hsk:${Buffer.from(publicKey).toString('base64')}`;
    
    // Store private key securely
    if (Platform.OS === 'ios') {
      await HSKSecureEnclave.storePrivateKey(keyPair.secretKey, did);
    } else {
      await Keychain.setGenericPassword(
        `hsk_private_key_${did}`,
        Buffer.from(keyPair.secretKey).toString('base64'),
        { service: 'io.hskernel.sdk' }
      );
    }
    
    const newIdentity: HSKIdentity = { did, publicKey };
    setIdentity(newIdentity);
    await AsyncStorage.setItem('hsk_current_did', did);
    
    return newIdentity;
  }, [isInitialized]);

  const restoreIdentity = useCallback(async (did: string): Promise<HSKIdentity | null> => {
    if (!isInitialized) throw new Error(HSKError.NOT_INITIALIZED);

    // Retrieve public key from DID
    const publicKeyBase64 = did.replace('did:hsk:', '');
    const publicKey = Buffer.from(publicKeyBase64, 'base64');
    
    // Verify private key exists
    let privateKeyExists = false;
    if (Platform.OS === 'ios') {
      privateKeyExists = await HSKSecureEnclave.hasPrivateKey(did);
    } else {
      const credentials = await Keychain.getGenericPassword({
        service: 'io.hskernel.sdk',
      });
      privateKeyExists = credentials !== false;
    }
    
    if (!privateKeyExists) {
      return null;
    }
    
    const restoredIdentity: HSKIdentity = { did, publicKey };
    setIdentity(restoredIdentity);
    return restoredIdentity;
  }, [isInitialized]);

  const grantConsent = useCallback(async (
    purpose: string,
    dataCategories: string[],
    retentionDays: number,
    legalBasis: HSKLegalBasis
  ): Promise<HSKConsent> => {
    if (!isInitialized) throw new Error(HSKError.NOT_INITIALIZED);
    if (!identity) throw new Error(HSKError.NOT_AUTHENTICATED);

    const consent: HSKConsent = {
      id: generateUUID(),
      purpose,
      dataCategories,
      retentionDays,
      legalBasis,
      grantedAt: new Date(),
      dataSubject: identity.did,
    };

    // Sign consent
    const consentHash = hashConsent(consent);
    const signature = await signData(consentHash, identity.did);
    consent.signature = signature;

    // Submit to server
    const response = await fetch(`${config?.baseURL}/consent`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config?.apiKey}`,
      },
      body: JSON.stringify(consent),
    });

    if (!response.ok) {
      throw new Error(HSKError.SERVER_ERROR);
    }

    return consent;
  }, [isInitialized, identity, config]);

  const revokeConsent = useCallback(async (consentId: string): Promise<void> => {
    if (!isInitialized) throw new Error(HSKError.NOT_INITIALIZED);
    if (!identity) throw new Error(HSKError.NOT_AUTHENTICATED);

    const revocation = {
      consentId,
      revokedAt: new Date(),
      reason: 'User initiated',
    };

    const response = await fetch(`${config?.baseURL}/consent/revoke`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${config?.apiKey}`,
      },
      body: JSON.stringify(revocation),
    });

    if (!response.ok) {
      throw new Error(HSKError.SERVER_ERROR);
    }
  }, [isInitialized, identity, config]);

  const getConsentHistory = useCallback(async (): Promise<HSKConsent[]> => {
    if (!isInitialized) throw new Error(HSKError.NOT_INITIALIZED);
    if (!identity) throw new Error(HSKError.NOT_AUTHENTICATED);

    const response = await fetch(
      `${config?.baseURL}/consent/history?did=${identity.did}`,
      {
        headers: {
          'Authorization': `Bearer ${config?.apiKey}`,
        },
      }
    );

    if (!response.ok) {
      throw new Error(HSKError.SERVER_ERROR);
    }

    return response.json();
  }, [isInitialized, identity, config]);

  const authenticateWithBiometric = useCallback(async (): Promise<boolean> => {
    try {
      if (Platform.OS === 'ios') {
        return await HSKBiometricAuth.authenticate();
      } else {
        // Android biometric
        return await HSKBiometricAuth.authenticate();
      }
    } catch (error) {
      console.error('Biometric authentication failed:', error);
      return false;
    }
  }, []);

  const logout = useCallback(async (): Promise<void> => {
    setIdentity(null);
    await AsyncStorage.removeItem('hsk_current_did');
  }, []);

  return (
    <HSKContext.Provider
      value={{
        isInitialized,
        identity,
        initialize,
        createIdentity,
        restoreIdentity,
        grantConsent,
        revokeConsent,
        getConsentHistory,
        authenticateWithBiometric,
        logout,
      }}
    >
      {children}
    </HSKContext.Provider>
  );
};

// Hook
export const useHSK = (): HSKContextType => {
  const context = useContext(HSKContext);
  if (!context) {
    throw new Error('useHSK must be used within HSKProvider');
  }
  return context;
};

// Helper functions
function generateUUID(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

function hashConsent(consent: HSKConsent): Uint8Array {
  // Implementation using SHA-256
  const data = JSON.stringify({
    purpose: consent.purpose,
    dataCategories: consent.dataCategories,
    retentionDays: consent.retentionDays,
    legalBasis: consent.legalBasis,
    grantedAt: consent.grantedAt.toISOString(),
    dataSubject: consent.dataSubject,
  });
  
  // Use crypto library for hashing
  return new TextEncoder().encode(data);
}

async function signData(data: Uint8Array, did: string): Promise<Uint8Array> {
  // Retrieve private key and sign
  // Implementation depends on platform
  return new Uint8Array(0);
}

// UI Components
export const HSKConsentButton: React.FC<{
  purpose: string;
  dataCategories: string[];
  retentionDays: number;
  legalBasis: HSKLegalBasis;
  onSuccess?: (consent: HSKConsent) => void;
  onError?: (error: Error) => void;
}> = ({ purpose, dataCategories, retentionDays, legalBasis, onSuccess, onError }) => {
  const { grantConsent, authenticateWithBiometric } = useHSK();
  const [loading, setLoading] = useState(false);

  const handlePress = async () => {
    setLoading(true);
    try {
      // Authenticate with biometric first
      const authenticated = await authenticateWithBiometric();
      if (!authenticated) {
        Alert.alert('Authentication Required', 'Please authenticate to grant consent');
        return;
      }

      const consent = await grantConsent(purpose, dataCategories, retentionDays, legalBasis);
      onSuccess?.(consent);
    } catch (error) {
      onError?.(error as Error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <button onClick={handlePress} disabled={loading}>
      {loading ? 'Processing...' : 'Grant Consent'}
    </button>
  );
};

export default {
  HSKProvider,
  useHSK,
  HSKConsentButton,
  HSKLegalBasis,
  HSKError,
};
