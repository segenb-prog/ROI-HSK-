import Foundation
import CryptoKit
import LocalAuthentication
import AuthenticationServices
import Alamofire
import KeychainAccess

/// Main HSK SDK class
public final class HSKSDK {
    public static let shared = HSKSDK()
    
    private var config: HSKConfig?
    private let keychain = Keychain(service: "io.hskernel.sdk")
    private let session: Session
    private var currentIdentity: HSKIdentity?
    
    private init() {
        let configuration = URLSessionConfiguration.default
        configuration.timeoutIntervalForRequest = 30
        configuration.timeoutIntervalForResource = 300
        self.session = Session(configuration: configuration)
    }
    
    // MARK: - Configuration
    
    /// Initialize the SDK with configuration
    public func initialize(config: HSKConfig) {
        self.config = config
        
        // Try to restore existing identity
        if let savedDID = UserDefaults.standard.string(forKey: "hsk_current_did") {
            _ = try? restoreIdentity(did: savedDID)
        }
    }
    
    /// Check if SDK is initialized
    public var isInitialized: Bool {
        return config != nil
    }
    
    /// Get current identity
    public var identity: HSKIdentity? {
        return currentIdentity
    }
    
    // MARK: - Identity Management
    
    /// Create a new identity
    public func createIdentity() async throws -> HSKIdentity {
        guard isInitialized else {
            throw HSKError.notInitialized
        }
        
        // Generate Ed25519 key pair
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        
        // Create DID
        let publicKeyBase64 = publicKey.rawRepresentation.base64EncodedString()
        let did = "did:hsk:\(publicKeyBase64)"
        
        // Store private key in keychain
        let privateKeyData = privateKey.rawRepresentation
        try keychain.set(privateKeyData, key: "hsk_private_key_\(did)")
        
        // Create identity object
        let identity = HSKIdentity(
            did: did,
            publicKey: publicKeyBase64,
            createdAt: Date()
        )
        
        // Save as current identity
        currentIdentity = identity
        UserDefaults.standard.set(did, forKey: "hsk_current_did")
        
        // Register with server
        try await registerIdentity(identity)
        
        return identity
    }
    
    /// Restore an existing identity
    public func restoreIdentity(did: String) async throws -> HSKIdentity {
        guard isInitialized else {
            throw HSKError.notInitialized
        }
        
        // Verify private key exists
        guard let privateKeyData = try keychain.getData("hsk_private_key_\(did)") else {
            throw HSKError.identityNotFound
        }
        
        // Verify key is valid
        _ = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyData)
        
        // Extract public key from DID
        let publicKeyBase64 = did.replacingOccurrences(of: "did:hsk:", with: "")
        
        let identity = HSKIdentity(
            did: did,
            publicKey: publicKeyBase64,
            createdAt: Date() // Would be fetched from server
        )
        
        currentIdentity = identity
        return identity
    }
    
    /// Delete identity
    public func deleteIdentity(did: String) async throws {
        try keychain.remove("hsk_private_key_\(did)")
        if currentIdentity?.did == did {
            currentIdentity = nil
            UserDefaults.standard.removeObject(forKey: "hsk_current_did")
        }
    }
    
    // MARK: - Consent Management
    
    /// Grant consent
    public func grantConsent(
        purpose: String,
        dataCategories: [String],
        retentionDays: Int = 365,
        legalBasis: HSKLegalBasis = .consent
    ) async throws -> HSKConsent {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let consent = HSKConsent(
            id: UUID().uuidString,
            purpose: purpose,
            dataCategories: dataCategories,
            retentionDays: retentionDays,
            legalBasis: legalBasis,
            grantedAt: Date(),
            expiresAt: Calendar.current.date(byAdding: .day, value: retentionDays, to: Date())!,
            dataSubject: identity.did,
            status: .active
        )
        
        // Sign consent
        let signature = try await sign(data: consent.hash(), did: identity.did)
        consent.signature = signature.base64EncodedString()
        
        // Submit to server
        let url = "\(config.baseURL)/consent"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)",
            "Content-Type": "application/json"
        ]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .post, parameters: consent.toDictionary(), encoding: JSONEncoding.default, headers: headers)
                .validate()
                .responseDecodable(of: HSKConsent.self) { response in
                    switch response.result {
                    case .success(let serverConsent):
                        continuation.resume(returning: serverConsent)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    /// Revoke consent
    public func revokeConsent(consentId: String, reason: String = "User initiated") async throws {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let revocation = HSKRevocation(
            consentId: consentId,
            revokedAt: Date(),
            reason: reason
        )
        
        // Sign revocation
        let signature = try await sign(data: revocation.hash(), did: identity.did)
        revocation.signature = signature.base64EncodedString()
        
        let url = "\(config.baseURL)/consent/revoke"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)",
            "Content-Type": "application/json"
        ]
        
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            session.request(url, method: .post, parameters: revocation.toDictionary(), encoding: JSONEncoding.default, headers: headers)
                .validate()
                .response { response in
                    switch response.result {
                    case .success:
                        continuation.resume()
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    /// Get consent history
    public func getConsentHistory() async throws -> [HSKConsent] {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let url = "\(config.baseURL)/consent/history"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)"
        ]
        let parameters = ["did": identity.did]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .get, parameters: parameters, headers: headers)
                .validate()
                .responseDecodable(of: [HSKConsent].self) { response in
                    switch response.result {
                    case .success(let consents):
                        continuation.resume(returning: consents)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    /// Verify consent
    public func verifyConsent(consentId: String) async throws -> HSKVerificationResult {
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let url = "\(config.baseURL)/consent/verify/\(consentId)"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)"
        ]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .get, headers: headers)
                .validate()
                .responseDecodable(of: HSKVerificationResult.self) { response in
                    switch response.result {
                    case .success(let result):
                        continuation.resume(returning: result)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    // MARK: - WebAuthn
    
    /// Register WebAuthn credential
    public func registerWebAuthn(presentationAnchor: ASPresentationAnchor) async throws {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        // Get challenge from server
        let challenge = try await getWebAuthnChallenge()
        
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: config.relyingPartyID
        )
        
        let request = provider.createCredentialRegistrationRequest(
            challenge: challenge,
            name: identity.did,
            userID: Data(identity.did.utf8)
        )
        
        let controller = ASAuthorizationController(authorizationRequests: [request])
        
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let delegate = WebAuthnDelegate { result in
                switch result {
                case .success:
                    continuation.resume()
                case .failure(let error):
                    continuation.resume(throwing: error)
                }
            }
            controller.delegate = delegate
            controller.presentationContextProvider = presentationAnchor as? ASAuthorizationControllerPresentationContextProviding
            controller.performRequests()
        }
    }
    
    /// Authenticate with WebAuthn
    public func authenticateWithWebAuthn(presentationAnchor: ASPresentationAnchor) async throws {
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let challenge = try await getWebAuthnChallenge()
        
        let provider = ASAuthorizationPlatformPublicKeyCredentialProvider(
            relyingPartyIdentifier: config.relyingPartyID
        )
        
        let request = provider.createCredentialAssertionRequest(challenge: challenge)
        
        let controller = ASAuthorizationController(authorizationRequests: [request])
        
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            let delegate = WebAuthnDelegate { result in
                switch result {
                case .success:
                    continuation.resume()
                case .failure(let error):
                    continuation.resume(throwing: error)
                }
            }
            controller.delegate = delegate
            controller.presentationContextProvider = presentationAnchor as? ASAuthorizationControllerPresentationContextProviding
            controller.performRequests()
        }
    }
    
    // MARK: - GDPR
    
    /// Export personal data (GDPR Article 20)
    public func exportPersonalData(format: HSKExportFormat = .json) async throws -> URL {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let url = "\(config.baseURL)/gdpr/export"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)"
        ]
        let parameters = [
            "did": identity.did,
            "format": format.rawValue
        ]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .post, parameters: parameters, headers: headers)
                .validate()
                .responseDecodable(of: HSKExportResponse.self) { response in
                    switch response.result {
                    case .success(let exportResponse):
                        continuation.resume(returning: URL(string: exportResponse.downloadUrl)!)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    /// Request data deletion (GDPR Article 17)
    public func requestDataDeletion(reason: String) async throws -> String {
        guard let identity = currentIdentity else {
            throw HSKError.notAuthenticated
        }
        
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let url = "\(config.baseURL)/gdpr/delete"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)",
            "Content-Type": "application/json"
        ]
        let parameters = [
            "did": identity.did,
            "reason": reason
        ]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .post, parameters: parameters, encoding: JSONEncoding.default, headers: headers)
                .validate()
                .responseDecodable(of: HSKDeletionResponse.self) { response in
                    switch response.result {
                    case .success(let deletionResponse):
                        continuation.resume(returning: deletionResponse.requestId)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    // MARK: - Private Methods
    
    private func registerIdentity(_ identity: HSKIdentity) async throws {
        guard let config = config else { return }
        
        let url = "\(config.baseURL)/identity"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)",
            "Content-Type": "application/json"
        ]
        
        _ = try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void, Error>) in
            session.request(url, method: .post, parameters: identity.toDictionary(), encoding: JSONEncoding.default, headers: headers)
                .validate()
                .response { response in
                    switch response.result {
                    case .success:
                        continuation.resume()
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
    
    private func sign(data: Data, did: String) async throws -> Data {
        guard let privateKeyData = try keychain.getData("hsk_private_key_\(did)") else {
            throw HSKError.privateKeyNotFound
        }
        
        let privateKey = try Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyData)
        return try privateKey.signature(for: data)
    }
    
    private func getWebAuthnChallenge() async throws -> Data {
        guard let config = config else {
            throw HSKError.notInitialized
        }
        
        let url = "\(config.baseURL)/webauthn/challenge"
        let headers: HTTPHeaders = [
            "Authorization": "Bearer \(config.apiKey)"
        ]
        
        return try await withCheckedThrowingContinuation { continuation in
            session.request(url, method: .get, headers: headers)
                .validate()
                .responseDecodable(of: HSKChallengeResponse.self) { response in
                    switch response.result {
                    case .success(let challengeResponse):
                        continuation.resume(returning: Data(base64Encoded: challengeResponse.challenge)!)
                    case .failure(let error):
                        continuation.resume(throwing: HSKError.networkError(error))
                    }
                }
        }
    }
}

// MARK: - Supporting Types

public struct HSKConfig {
    public let baseURL: String
    public let apiKey: String
    public let relyingPartyID: String
    
    public init(baseURL: String, apiKey: String, relyingPartyID: String = "hskernel.io") {
        self.baseURL = baseURL
        self.apiKey = apiKey
        self.relyingPartyID = relyingPartyID
    }
}

public struct HSKIdentity: Codable {
    public let did: String
    public let publicKey: String
    public let createdAt: Date
    
    func toDictionary() -> [String: Any] {
        return [
            "did": did,
            "publicKey": publicKey,
            "createdAt": ISO8601DateFormatter().string(from: createdAt)
        ]
    }
}

public class HSKConsent: Codable {
    public let id: String
    public let purpose: String
    public let dataCategories: [String]
    public let retentionDays: Int
    public let legalBasis: HSKLegalBasis
    public let grantedAt: Date
    public let expiresAt: Date
    public let dataSubject: String
    public var status: HSKConsentStatus
    public var signature: String?
    
    enum CodingKeys: String, CodingKey {
        case id, purpose, dataCategories, retentionDays, legalBasis
        case grantedAt, expiresAt, dataSubject, status, signature
    }
    
    init(id: String, purpose: String, dataCategories: [String], retentionDays: Int,
         legalBasis: HSKLegalBasis, grantedAt: Date, expiresAt: Date,
         dataSubject: String, status: HSKConsentStatus = .active) {
        self.id = id
        self.purpose = purpose
        self.dataCategories = dataCategories
        self.retentionDays = retentionDays
        self.legalBasis = legalBasis
        self.grantedAt = grantedAt
        self.expiresAt = expiresAt
        self.dataSubject = dataSubject
        self.status = status
    }
    
    public func hash() -> Data {
        let data = "\(id):\(purpose):\(dataCategories.joined(separator: ",")):\(retentionDays):\(legalBasis.rawValue):\(grantedAt.iso8601):\(dataSubject)"
        return Data(data.utf8)
    }
    
    func toDictionary() -> [String: Any] {
        return [
            "id": id,
            "purpose": purpose,
            "dataCategories": dataCategories,
            "retentionDays": retentionDays,
            "legalBasis": legalBasis.rawValue,
            "grantedAt": grantedAt.iso8601,
            "expiresAt": expiresAt.iso8601,
            "dataSubject": dataSubject,
            "status": status.rawValue,
            "signature": signature as Any
        ]
    }
}

public struct HSKRevocation {
    let consentId: String
    let revokedAt: Date
    let reason: String
    var signature: String?
    
    func hash() -> Data {
        let data = "\(consentId):\(revokedAt.iso8601):\(reason)"
        return Data(data.utf8)
    }
    
    func toDictionary() -> [String: Any] {
        return [
            "consentId": consentId,
            "revokedAt": revokedAt.iso8601,
            "reason": reason,
            "signature": signature as Any
        ]
    }
}

public struct HSKVerificationResult: Codable {
    public let valid: Bool
    public let message: String
    public let details: [String: String]?
}

public enum HSKLegalBasis: String, Codable {
    case consent = "consent"
    case contract = "contract"
    case legalObligation = "legal_obligation"
    case vitalInterests = "vital_interests"
    case publicTask = "public_task"
    case legitimateInterests = "legitimate_interests"
}

public enum HSKConsentStatus: String, Codable {
    case active = "active"
    case revoked = "revoked"
    case expired = "expired"
}

public enum HSKExportFormat: String {
    case json = "json"
    case jsonld = "jsonld"
    case csv = "csv"
    case xml = "xml"
    case pdf = "pdf"
}

public struct HSKExportResponse: Codable {
    public let requestId: String
    public let downloadUrl: String
    public let expiresAt: String
}

public struct HSKDeletionResponse: Codable {
    public let requestId: String
    public let status: String
    public let estimatedCompletion: String
}

public struct HSKChallengeResponse: Codable {
    public let challenge: String
}

public enum HSKError: Error {
    case notInitialized
    case notAuthenticated
    case identityNotFound
    case privateKeyNotFound
    case networkError(Error)
    case serverError(String)
    case invalidResponse
    case biometricError(String)
    case webAuthnError(String)
}

// MARK: - Extensions

extension Date {
    var iso8601: String {
        return ISO8601DateFormatter().string(from: self)
    }
}

// MARK: - WebAuthn Delegate

private class WebAuthnDelegate: NSObject, ASAuthorizationControllerDelegate {
    typealias CompletionHandler = (Result<Void, Error>) -> Void
    let completion: CompletionHandler
    
    init(completion: @escaping CompletionHandler) {
        self.completion = completion
    }
    
    func authorizationController(controller: ASAuthorizationController, didCompleteWithAuthorization authorization: ASAuthorization) {
        completion(.success(()))
    }
    
    func authorizationController(controller: ASAuthorizationController, didCompleteWithError error: Error) {
        completion(.failure(HSKError.webAuthnError(error.localizedDescription)))
    }
}
