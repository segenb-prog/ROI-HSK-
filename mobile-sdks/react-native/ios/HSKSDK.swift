import Foundation
import CryptoKit
import LocalAuthentication
import Combine

@objc(HSKSDK)
class HSKSDK: NSObject {
    
    // MARK: - Properties
    
    private var baseUrl: URL?
    private var apiKey: String?
    private var did: String?
    private var privateKey: Curve25519.Signing.PrivateKey?
    private let keychain = Keychain(service: "io.hskernel.react-native")
    private let session: URLSession
    private var cancellables = Set<AnyCancellable>()
    
    // MARK: - Initialization
    
    override init() {
        let config = URLSessionConfiguration.default
        config.timeoutIntervalForRequest = 30
        config.timeoutIntervalForResource = 300
        self.session = URLSession(configuration: config)
        super.init()
        
        // Restore identity if exists
        self.did = keychain.get("did")
        if let privateKeyData = keychain.getData("privateKey") {
            self.privateKey = try? Curve25519.Signing.PrivateKey(rawRepresentation: privateKeyData)
        }
    }
    
    // MARK: - Public Methods
    
    @objc(initialize:withResolver:withRejecter:)
    func initialize(
        config: [String: Any],
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let baseUrlString = config["baseUrl"] as? String,
              let apiKey = config["apiKey"] as? String,
              let url = URL(string: baseUrlString) else {
            reject("INIT_ERROR", "Invalid configuration", nil)
            return
        }
        
        self.baseUrl = url
        self.apiKey = apiKey
        
        // Verify connection
        let request = URLRequest(url: url.appendingPathComponent("/v1/health"))
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("INIT_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  httpResponse.statusCode == 200 else {
                reject("INIT_ERROR", "Failed to connect to server", nil)
                return
            }
            
            resolve([
                "success": true,
                "version": String(data: data ?? Data(), encoding: .utf8) ?? "unknown"
            ])
        }.resume()
    }
    
    @objc(createIdentity:withRejecter:)
    func createIdentity(
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        // Generate Ed25519 key pair
        let privateKey = Curve25519.Signing.PrivateKey()
        let publicKey = privateKey.publicKey
        
        // Create DID
        let publicKeyBase64 = publicKey.rawRepresentation.base64EncodedString()
        let did = "did:hsk:\(publicKeyBase64)"
        
        // Store in keychain
        do {
            try keychain.set(did, key: "did")
            try keychain.set(privateKey.rawRepresentation, key: "privateKey")
        } catch {
            reject("KEYCHAIN_ERROR", "Failed to store identity", error)
            return
        }
        
        self.did = did
        self.privateKey = privateKey
        
        // Register with server
        let deviceInfo: [String: Any] = [
            "platform": "ios",
            "model": UIDevice.current.model,
            "os_version": UIDevice.current.systemVersion,
            "device_id": UIDevice.current.identifierForVendor?.uuidString ?? "unknown"
        ]
        
        let body: [String: Any] = [
            "did": did,
            "public_key": publicKeyBase64,
            "device_info": deviceInfo
        ]
        
        guard let url = baseUrl?.appendingPathComponent("/v1/identities"),
              let jsonData = try? JSONSerialization.data(withJSONObject: body) else {
            reject("REGISTRATION_ERROR", "Failed to create request", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.httpBody = jsonData
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { [weak self] data, response, error in
            if let error = error {
                reject("REGISTRATION_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  (200...299).contains(httpResponse.statusCode) else {
                reject("REGISTRATION_ERROR", "Server registration failed", nil)
                return
            }
            
            resolve([
                "did": did,
                "createdAt": ISO8601DateFormatter().string(from: Date())
            ])
        }.resume()
    }
    
    @objc(grantConsent:withResolver:withRejecter:)
    func grantConsent(
        params: [String: Any],
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let did = self.did,
              let privateKey = self.privateKey else {
            reject("IDENTITY_ERROR", "No identity created", nil)
            return
        }
        
        guard let purpose = params["purpose"] as? String,
              let dataCategories = params["dataCategories"] as? [String] else {
            reject("PARAMS_ERROR", "Missing required parameters", nil)
            return
        }
        
        let validFrom = ISO8601DateFormatter().string(from: Date())
        let validUntil: String
        if let expiresIn = params["expiresInDays"] as? Int {
            validUntil = ISO8601DateFormatter().string(
                from: Calendar.current.date(byAdding: .day, value: expiresIn, to: Date())!
            )
        } else {
            validUntil = ISO8601DateFormatter().string(
                from: Calendar.current.date(byAdding: .year, value: 1, to: Date())!
            )
        }
        
        let payload: [String: Any] = [
            "did": did,
            "purpose": purpose,
            "data_categories": dataCategories,
            "valid_from": validFrom,
            "valid_until": validUntil,
            "constraints": params["constraints"] ?? [:],
            "scope": params["scope"] as? String ?? "general"
        ]
        
        guard let payloadData = try? JSONSerialization.data(withJSONObject: payload),
              let signature = try? privateKey.signature(for: payloadData) else {
            reject("SIGNING_ERROR", "Failed to sign consent", nil)
            return
        }
        
        let body: [String: Any] = [
            "payload": payload,
            "signature": signature.rawRepresentation.base64EncodedString(),
            "algorithm": "Ed25519"
        ]
        
        guard let url = baseUrl?.appendingPathComponent("/v1/consent"),
              let jsonData = try? JSONSerialization.data(withJSONObject: body) else {
            reject("REQUEST_ERROR", "Failed to create request", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.httpBody = jsonData
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.setValue("Bearer \(did)", forHTTPHeaderField: "Authorization")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("CONSENT_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  (200...299).contains(httpResponse.statusCode),
                  let data = data,
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                reject("CONSENT_ERROR", "Failed to grant consent", nil)
                return
            }
            
            resolve([
                "consentId": json["consent_id"] as? String ?? "",
                "merkleRoot": json["merkle_root"] as? String ?? "",
                "timestamp": json["timestamp"] as? String ?? "",
                "txHash": json["tx_hash"] as? String ?? ""
            ])
        }.resume()
    }
    
    @objc(revokeConsent:withResolver:withRejecter:)
    func revokeConsent(
        consentId: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let did = self.did else {
            reject("IDENTITY_ERROR", "No identity created", nil)
            return
        }
        
        guard let url = baseUrl?.appendingPathComponent("/v1/consent/\(consentId)") else {
            reject("URL_ERROR", "Invalid URL", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("Bearer \(did)", forHTTPHeaderField: "Authorization")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("REVOKE_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  (200...299).contains(httpResponse.statusCode) else {
                reject("REVOKE_ERROR", "Failed to revoke consent", nil)
                return
            }
            
            resolve([
                "consentId": consentId,
                "revokedAt": ISO8601DateFormatter().string(from: Date())
            ])
        }.resume()
    }
    
    @objc(verifyConsent:withResolver:withRejecter:)
    func verifyConsent(
        consentId: String,
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let url = baseUrl?.appendingPathComponent("/v1/consent/\(consentId)/verify") else {
            reject("URL_ERROR", "Invalid URL", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("VERIFY_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let data = data,
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                reject("VERIFY_ERROR", "Invalid response", nil)
                return
            }
            
            resolve([
                "valid": json["valid"] as? Bool ?? false,
                "verifiedAt": json["verified_at"] as? String ?? "",
                "merkleProof": json["merkle_proof"] as? String ?? ""
            ])
        }.resume()
    }
    
    @objc(getMyConsents:withRejecter:)
    func getMyConsents(
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let did = self.did else {
            reject("IDENTITY_ERROR", "No identity created", nil)
            return
        }
        
        guard let url = baseUrl?.appendingPathComponent("/v1/identities/\(did)/consent") else {
            reject("URL_ERROR", "Invalid URL", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.setValue("Bearer \(did)", forHTTPHeaderField: "Authorization")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("FETCH_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let data = data,
                  let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let consents = json["consents"] as? [[String: Any]] else {
                reject("FETCH_ERROR", "Invalid response", nil)
                return
            }
            
            let mappedConsents = consents.map { consent -> [String: Any] in
                return [
                    "consentId": consent["consent_id"] as? String ?? "",
                    "purpose": consent["purpose"] as? String ?? "",
                    "status": consent["status"] as? String ?? "",
                    "createdAt": consent["created_at"] as? String ?? "",
                    "expiresAt": consent["expires_at"] as? String ?? ""
                ]
            }
            
            resolve([
                "consents": mappedConsents,
                "total": json["total"] as? Int ?? 0
            ])
        }.resume()
    }
    
    @objc(authenticateWithBiometric:withRejecter:)
    func authenticateWithBiometric(
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        let context = LAContext()
        var error: NSError?
        
        guard context.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &error) else {
            reject("BIOMETRIC_UNAVAILABLE", error?.localizedDescription ?? "Biometric not available", nil)
            return
        }
        
        context.evaluatePolicy(
            .deviceOwnerAuthenticationWithBiometrics,
            localizedReason: "Authenticate to access your HSK identity"
        ) { success, error in
            DispatchQueue.main.async {
                if success {
                    resolve([
                        "success": true,
                        "method": "biometric"
                    ])
                } else if let error = error {
                    reject("AUTH_FAILED", error.localizedDescription, error)
                } else {
                    reject("AUTH_FAILED", "Authentication failed", nil)
                }
            }
        }
    }
    
    @objc(exportMyData:withRejecter:)
    func exportMyData(
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let did = self.did else {
            reject("IDENTITY_ERROR", "No identity created", nil)
            return
        }
        
        guard let url = baseUrl?.appendingPathComponent("/v1/identities/\(did)/export") else {
            reject("URL_ERROR", "Invalid URL", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.setValue("Bearer \(did)", forHTTPHeaderField: "Authorization")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { data, response, error in
            if let error = error {
                reject("EXPORT_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let data = data,
                  let jsonString = String(data: data, encoding: .utf8) else {
                reject("EXPORT_ERROR", "Invalid response", nil)
                return
            }
            
            resolve(jsonString)
        }.resume()
    }
    
    @objc(deleteMyIdentity:withRejecter:)
    func deleteMyIdentity(
        resolve: @escaping RCTPromiseResolveBlock,
        reject: @escaping RCTPromiseRejectBlock
    ) {
        guard let did = self.did else {
            reject("IDENTITY_ERROR", "No identity created", nil)
            return
        }
        
        guard let url = baseUrl?.appendingPathComponent("/v1/identities/\(did)") else {
            reject("URL_ERROR", "Invalid URL", nil)
            return
        }
        
        var request = URLRequest(url: url)
        request.httpMethod = "DELETE"
        request.setValue("Bearer \(did)", forHTTPHeaderField: "Authorization")
        request.setValue(apiKey, forHTTPHeaderField: "X-API-Key")
        
        session.dataTask(with: request) { [weak self] data, response, error in
            if let error = error {
                reject("DELETE_ERROR", error.localizedDescription, error)
                return
            }
            
            guard let httpResponse = response as? HTTPURLResponse,
                  (200...299).contains(httpResponse.statusCode) else {
                reject("DELETE_ERROR", "Deletion failed", nil)
                return
            }
            
            // Clear local data
            self?.keychain.delete("did")
            self?.keychain.delete("privateKey")
            self?.did = nil
            self?.privateKey = nil
            
            resolve([
                "deleted": true,
                "deletedAt": ISO8601DateFormatter().string(from: Date())
            ])
        }.resume()
    }
}

// MARK: - Keychain Helper

class Keychain {
    let service: String
    
    init(service: String) {
        self.service = service
    }
    
    func set(_ value: String, key: String) throws {
        guard let data = value.data(using: .utf8) else {
            throw KeychainError.conversionFailed
        }
        try set(data, key: key)
    }
    
    func set(_ data: Data, key: String) throws {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecValueData as String: data
        ]
        
        SecItemDelete(query as CFDictionary)
        
        let status = SecItemAdd(query as CFDictionary, nil)
        guard status == errSecSuccess else {
            throw KeychainError.saveFailed(status)
        }
    }
    
    func get(_ key: String) -> String? {
        guard let data = getData(key) else { return nil }
        return String(data: data, encoding: .utf8)
    }
    
    func getData(_ key: String) -> Data? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        
        var result: AnyObject?
        let status = SecItemCopyMatching(query as CFDictionary, &result)
        
        guard status == errSecSuccess else { return nil }
        return result as? Data
    }
    
    func delete(_ key: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: key
        ]
        SecItemDelete(query as CFDictionary)
    }
}

enum KeychainError: Error {
    case conversionFailed
    case saveFailed(OSStatus)
}
