part of '../hsk_sdk.dart';

/// Manages consent operations
class _ConsentManager {
  final _ApiClient _apiClient;
  final _IdentityManager _identityManager;
  
  _ConsentManager(this._apiClient, this._identityManager);
  
  /// Grant consent for data processing
  Future<HSKConsent> grantConsent(ConsentRequest request) async {
    final did = await _identityManager.getDID();
    final privateKey = await _identityManager.getPrivateKey();
    
    if (did == null || privateKey == null) {
      throw HSKConsentException('No identity created');
    }
    
    // Build consent payload
    final validFrom = DateTime.now().toIso8601String();
    final validUntil = request.expiresInDays != null
        ? DateTime.now().add(Duration(days: request.expiresInDays!)).toIso8601String()
        : DateTime.now().add(const Duration(days: 365)).toIso8601String();
    
    final payload = {
      'did': did,
      'purpose': request.purpose,
      'data_categories': request.dataCategories,
      'valid_from': validFrom,
      'valid_until': validUntil,
      'constraints': request.constraints,
      'scope': request.scope ?? 'general',
    };
    
    // Sign the consent
    final payloadBytes = utf8.encode(jsonEncode(payload));
    final signature = await _CryptoUtils.sign(payloadBytes, privateKey);
    
    final body = {
      'payload': payload,
      'signature': base64Encode(signature),
      'algorithm': 'Ed25519',
    };
    
    final response = await _apiClient.post(
      '/v1/consent',
      body: body,
      headers: {'Authorization': 'Bearer $did'},
    );
    
    if (response.statusCode != 201) {
      throw HSKConsentException('Failed to grant consent: ${response.statusCode}');
    }
    
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return HSKConsent.fromJson(json);
  }
  
  /// Revoke a previously granted consent
  Future<ConsentRevocation> revokeConsent(String consentId) async {
    final did = await _identityManager.getDID();
    if (did == null) {
      throw HSKConsentException('No identity created');
    }
    
    final response = await _apiClient.delete(
      '/v1/consent/$consentId',
      headers: {'Authorization': 'Bearer $did'},
    );
    
    if (response.statusCode != 200 && response.statusCode != 204) {
      throw HSKConsentException('Failed to revoke consent: ${response.statusCode}');
    }
    
    return ConsentRevocation(
      consentId: consentId,
      revokedAt: DateTime.now().toIso8601String(),
    );
  }
  
  /// Verify a consent record's integrity
  Future<ConsentVerification> verifyConsent(String consentId) async {
    final response = await _apiClient.get('/v1/consent/$consentId/verify');
    
    if (response.statusCode != 200) {
      throw HSKConsentException('Verification failed: ${response.statusCode}');
    }
    
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return ConsentVerification.fromJson(json);
  }
  
  /// Get all consents for the current identity
  Future<ConsentList> getMyConsents() async {
    final did = await _identityManager.getDID();
    if (did == null) {
      throw HSKConsentException('No identity created');
    }
    
    final response = await _apiClient.get(
      '/v1/identities/$did/consent',
      headers: {'Authorization': 'Bearer $did'},
    );
    
    if (response.statusCode != 200) {
      throw HSKConsentException('Failed to fetch consents: ${response.statusCode}');
    }
    
    final json = jsonDecode(response.body) as Map<String, dynamic>;
    return ConsentList.fromJson(json);
  }
}
