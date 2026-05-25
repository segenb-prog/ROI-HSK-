part of '../hsk_sdk.dart';

/// Manages cryptographic identity operations
class _IdentityManager {
  final _ApiClient _apiClient;
  final FlutterSecureStorage _secureStorage = const FlutterSecureStorage(
    aOptions: AndroidOptions(
      encryptedSharedPreferences: true,
    ),
    iOptions: IOSOptions(
      accessibility: KeychainAccessibility.first_unlock_this_device,
    ),
  );
  
  static const String _didKey = 'hsk_did';
  static const String _privateKeyKey = 'hsk_private_key';
  
  _IdentityManager(this._apiClient);
  
  /// Check if an identity exists
  Future<bool> hasIdentity() async {
    final did = await _secureStorage.read(key: _didKey);
    final privateKey = await _secureStorage.read(key: _privateKeyKey);
    return did != null && privateKey != null;
  }
  
  /// Get the current identity if one exists
  Future<HSKIdentity?> getCurrentIdentity() async {
    final did = await _secureStorage.read(key: _didKey);
    final createdAt = await _secureStorage.read(key: 'hsk_created_at');
    
    if (did == null) return null;
    
    return HSKIdentity(
      did: did,
      createdAt: createdAt ?? DateTime.now().toIso8601String(),
    );
  }
  
  /// Create a new identity
  Future<HSKIdentity> createIdentity() async {
    // Generate Ed25519 key pair
    final keyPair = await _CryptoUtils.generateKeyPair();
    
    // Create DID from public key
    final publicKeyBase64 = base64Encode(keyPair.publicKey);
    final did = 'did:hsk:${publicKeyBase64.replaceAll('=', '').replaceAll('+', '-').replaceAll('/', '_')}';
    
    // Get device info
    final deviceInfo = await DeviceInfo.current();
    
    // Register with server
    final body = {
      'did': did,
      'public_key': publicKeyBase64,
      'device_info': deviceInfo.toJson(),
    };
    
    final response = await _apiClient.post(
      '/v1/identities',
      body: body,
    );
    
    if (response.statusCode != 201 && response.statusCode != 200) {
      throw HSKIdentityException(
        'Server registration failed: ${response.statusCode}'
      );
    }
    
    // Store identity locally
    final createdAt = DateTime.now().toIso8601String();
    await _secureStorage.write(key: _didKey, value: did);
    await _secureStorage.write(key: _privateKeyKey, value: base64Encode(keyPair.privateKey));
    await _secureStorage.write(key: 'hsk_created_at', value: createdAt);
    
    return HSKIdentity(did: did, createdAt: createdAt);
  }
  
  /// Get the private key for signing
  Future<Uint8List?> getPrivateKey() async {
    final privateKeyBase64 = await _secureStorage.read(key: _privateKeyKey);
    if (privateKeyBase64 == null) return null;
    return base64Decode(privateKeyBase64);
  }
  
  /// Get the DID
  Future<String?> getDID() async {
    return await _secureStorage.read(key: _didKey);
  }
  
  /// Delete the identity (right to be forgotten)
  Future<IdentityDeletion> deleteIdentity() async {
    final did = await getDID();
    if (did == null) {
      throw HSKIdentityException('No identity exists');
    }
    
    final response = await _apiClient.delete(
      '/v1/identities/$did',
      headers: {'Authorization': 'Bearer $did'},
    );
    
    if (response.statusCode != 200 && response.statusCode != 204) {
      throw HSKIdentityException('Deletion failed: ${response.statusCode}');
    }
    
    // Clear local storage
    await _secureStorage.delete(key: _didKey);
    await _secureStorage.delete(key: _privateKeyKey);
    await _secureStorage.delete(key: 'hsk_created_at');
    
    return IdentityDeletion(
      deleted: true,
      deletedAt: DateTime.now().toIso8601String(),
    );
  }
}
