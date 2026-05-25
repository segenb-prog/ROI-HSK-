// HSK iOS SDK
// Swift 5.9+

import Foundation
import CryptoKit
import AuthenticationServices

public class HSKSDK {
    public static let shared = HSKSDK()
    
    private var config: HSKConfig?
    private var keychain: HSKKeychain
    private var apiClient: HSKAPIClient
    
    private init() {
        self.keychain = HSKKeychain()
        self.apiClient = HSKAPIClient()
    }
    
    public func initialize(config: HSKConfig) {
        self.config = config
        self.apiClient.configure(baseURL: config.baseURL, apiKey: config.apiKey)
    }
    
    // MARK: - Identity
    
    public func createIdentity() async throws -> HSKIdentity {
        let privateKey = try Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        
        let did = "did:hsk:\(publicKey.rawRepresentation.base64EncodedString())"
        
        // Store private key in Secure Enclave
        try await keychain.storePrivateKey(privateKey, forDID: did)
        
        return HSKIdentity(did: did, publicKey: publicKey)
    }
    
    public func restoreIdentity(fromDID did: String) async throws -> HSKIdentity {
        guard let privateKey = try await keychain.retrievePrivateKey(forDID: did) else {
            throw HSKError.identityNotFound
        }
        
        return HSKIdentity(did: did, publicKey: privateKey.publicKey)
    }
    
    // MARK: - Consent
    
    public func grantConsent(
        purpose: String,
        dataCategories: [String],
        retentionDays: Int,
        legalBasis: HSKLegalBasis
    ) async throws -> HSKConsent {
        guard let identity = try await getCurrentIdentity() else {
            throw HSKError.notAuthenticated
        }
        
        let consent = HSKConsent(
            purpose: purpose,
            dataCategories: dataCategories,
            retentionDays: retentionDays,
            legalBasis: legalBasis,
            grantedAt: Date(),
            dataSubject: identity.did
        )
        
        // Sign consent
        let signature = try await sign(consent.hash(), with: identity)
        consent.signature = signature
        
        // Submit to server
        return try await apiClient.submitConsent(consent)
    }
    
    public func revokeConsent(consentId: String) async throws {
        guard let identity = try await getCurrentIdentity() else {
            throw HSKError.notAuthenticated
        }
        
        let revocation = HSKRevocation(
            consentId: consentId,
            revokedAt: Date(),
            reason: "User initiated"
        )
        
        let signature = try await sign(revocation.hash(), with: identity)
        revocation.signature = signature
        
        try await apiClient.revokeConsent(revocation)
    }
    
    public func getConsentHistory() async throws -> [HSKConsent] {
        guard let identity = try await getCurrentIdentity() else {
            throw HSKError.notAuthenticated
        }
        
        return try await apiClient.getConsentHistory(for: identity.did)
    }
    
    // MARK: - WebAuthn
    
    public func registerWebAuthn(presentationContext: ASPresentationAnchor) async throws {
        guard let identity = try await getCurrentIdentity() else {
            throw HSKError.notAuthenticated
        }
        
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: config?.relyingPartyID ?? "hskernel.io"
        )
        
        let challenge = try await apiClient.getWebAuthnChallenge(for: identity.did)
        
        let request = provider.createCredentialRegistrationRequest(
            challenge: challenge,
            name: identity.did,
            userID: identity.did.data(using: .utf8)!
        )
        
        let controller = ASAuthorizationController(authorizationRequests: [request])
        
        return try await withCheckedThrowingContinuation { continuation in
            let delegate = WebAuthnDelegate(continuation: continuation)
            controller.delegate = delegate
            controller.presentationContextProvider = presentationContext as? ASAuthorizationControllerPresentationContextProviding
            controller.performRequests()
        }
    }
    
    // MARK: - Private
    
    private func getCurrentIdentity() async throws -> HSKIdentity? {
        // Implementation
        return nil
    }
    
    private func sign(_ data: Data, with identity: HSKIdentity) async throws -> Data {
        guard let privateKey = try await keychain.retrievePrivateKey(forDID: identity.did) else {
            throw HSKError.privateKeyNotFound
        }
        
        return try privateKey.signature(for: data)
    }
}

// MARK: - Models

public struct HSKConfig {
    let baseURL: URL
    let apiKey: String
    let relyingPartyID: String
}

public struct HSKIdentity {
    let did: String
    let publicKey: Curve25519.Signing.PublicKey
}

public struct HSKConsent {
    let id: String
    let purpose: String
    let dataCategories: [String]
    let retentionDays: Int
    let legalBasis: HSKLegalBasis
    let grantedAt: Date
    let dataSubject: String
    var signature: Data?
    
    func hash() -> Data {
        // Implementation
        return Data()
    }
}

public struct HSKRevocation {
    let consentId: String
    let revokedAt: Date
    let reason: String
    var signature: Data?
    
    func hash() -> Data {
        // Implementation
        return Data()
    }
}

public enum HSKLegalBasis: String {
    case consent = "consent"
    case contract = "contract"
    case legalObligation = "legal_obligation"
    case vitalInterests = "vital_interests"
    case publicTask = "public_task"
    case legitimateInterests = "legitimate_interests"
}

public enum HSKError: Error {
    case notInitialized
    case notAuthenticated
    case identityNotFound
    case privateKeyNotFound
    case networkError
    case serverError
    case invalidResponse
}

// MARK: - Keychain

class HSKKeychain {
    func storePrivateKey(_ key: Curve25519.Signing.PrivateKey, forDID did: String) async throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: did,
            kSecAttrService as String: "hsk.privatekey",
            kSecValueData as String: key.rawRepresentation,
            kSecAttrAccessible as String: kSecAttrAccessibleWhenUnlockedThisDeviceOnly
        ]
        
        SecItemDelete(query as CFDictionary)
        let status = SecItemAdd(query as CFDictionary, nil)
        
        guard status == errSecSuccess else {
            throw HSKError.identityNotFound
        }
    }
    
    func retrievePrivateKey(forDID did: String) async throws -> Curve25519.Signing.PrivateKey? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrAccount as String: did,
            kSecAttrService as String: "hsk.privatekey",
            kSecReturnData as String: true
        ]
        
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        
        guard status == errSecSuccess,
              let data = result as? Data else {
            return nil
        }
        
        return try? Curve25519.Signing.PrivateKey(rawRepresentation: data)
    }
}

// MARK: - API Client

class HSKAPIClient {
    private var baseURL: URL?
    private var apiKey: String?
    
    func configure(baseURL: URL, apiKey: String) {
        self.baseURL = baseURL
        self.apiKey = apiKey
    }
    
    func submitConsent(_ consent: HSKConsent) async throws -> HSKConsent {
        // Implementation
        return consent
    }
    
    func revokeConsent(_ revocation: HSKRevocation) async throws {
        // Implementation
    }
    
    func getConsentHistory(for did: String) async throws -> [HSKConsent] {
        // Implementation
        return []
    }
    
    func getWebAuthnChallenge(for did: String) async throws -> Data {
        // Implementation
        return Data()
    }
}

// MARK: - WebAuthn Delegate

class WebAuthnDelegate: NSObject, ASAuthorizationControllerDelegate {
    let continuation: CheckedContinuation<Void, Error>
    
    init(continuation: CheckedContinuation<Void, Error>) {
        self.continuation = continuation
    }
    
    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithAuthorization authorization: ASAuthorization
    ) {
        continuation.resume()
    }
    
    func authorizationController(
        controller: ASAuthorizationController,
        didCompleteWithError error: Error
    ) {
        continuation.resume(throwing: error)
    }
}
