part of '../hsk_sdk.dart';

/// Configuration for the HSK SDK
class HSKConfig {
  /// Base URL of the HSK platform API
  final String baseUrl;
  
  /// API key for authentication
  final String apiKey;
  
  /// Request timeout in milliseconds (default: 30000)
  final int timeout;
  
  /// Enable debug logging
  final bool debug;
  
  HSKConfig({
    required this.baseUrl,
    required this.apiKey,
    this.timeout = 30000,
    this.debug = false,
  });
  
  Map<String, dynamic> toJson() => {
    'baseUrl': baseUrl,
    'apiKey': apiKey,
    'timeout': timeout,
    'debug': debug,
  };
}

/// Main HSK SDK class for Flutter
/// 
/// Provides cryptographic identity management and consent operations
/// for the Human Sovereignty Kernel platform.
/// 
/// Example:
/// ```dart
/// import 'package:hsk_sdk/hsk_sdk.dart';
/// 
/// final sdk = HSKSDK();
/// await sdk.initialize(HSKConfig(
///   baseUrl: 'https://api.hsk.platform',
///   apiKey: 'your-api-key',
/// ));
/// 
/// final identity = await sdk.createIdentity();
/// final consent = await sdk.grantConsent(ConsentRequest(
///   purpose: 'analytics',
///   dataCategories: ['usage_data'],
/// ));
/// ```
class HSKSDK {
  static final HSKSDK _instance = HSKSDK._internal();
  
  factory HSKSDK() => _instance;
  
  HSKSDK._internal();
  
  late final HSKConfig _config;
  late final _IdentityManager _identityManager;
  late final _ConsentManager _consentManager;
  late final _ApiClient _apiClient;
  late final LocalAuthentication _localAuth;
  
  bool _initialized = false;
  
  /// Initialize the SDK with configuration
  /// 
  /// Must be called before any other SDK methods.
  /// 
  /// Throws [HSKInitializationException] if initialization fails
  Future<void> initialize(HSKConfig config) async {
    if (_initialized) {
      throw HSKInitializationException('SDK already initialized');
    }
    
    _config = config;
    _apiClient = _ApiClient(config);
    _identityManager = _IdentityManager(_apiClient);
    _consentManager = _ConsentManager(_apiClient, _identityManager);
    _localAuth = LocalAuthentication();
    
    // Verify connection to server
    try {
      final response = await _apiClient.get('/v1/health');
      if (response.statusCode != 200) {
        throw HSKInitializationException(
          'Failed to connect to HSK server: ${response.statusCode}'
        );
      }
      
      if (_config.debug) {
        print('[HSK SDK] Initialized successfully');
        print('[HSK SDK] Server version: ${response.body}');
      }
      
      _initialized = true;
    } catch (e) {
      throw HSKInitializationException('Initialization failed: $e');
    }
  }
  
  /// Create a new cryptographic identity
  /// 
  /// Generates an Ed25519 key pair and registers the identity with the HSK platform.
  /// The private key is securely stored in the device's secure storage.
  /// 
  /// Returns the created [HSKIdentity]
  /// 
  /// Throws [HSKIdentityException] if identity creation fails
  Future<HSKIdentity> createIdentity() async {
    _ensureInitialized();
    return _identityManager.createIdentity();
  }
  
  /// Get the current identity if one exists
  /// 
  /// Returns the [HSKIdentity] or null if no identity exists
  Future<HSKIdentity?> getCurrentIdentity() async {
    _ensureInitialized();
    return _identityManager.getCurrentIdentity();
  }
  
  /// Check if an identity exists
  Future<bool> hasIdentity() async {
    _ensureInitialized();
    return _identityManager.hasIdentity();
  }
  
  /// Grant consent for data processing
  /// 
  /// Creates a cryptographically signed consent record that is stored
  /// in the HSK transparency ledger.
  /// 
  /// Returns the granted [HSKConsent]
  /// 
  /// Throws [HSKConsentException] if consent grant fails
  Future<HSKConsent> grantConsent(ConsentRequest request) async {
    _ensureInitialized();
    return _consentManager.grantConsent(request);
  }
  
  /// Revoke a previously granted consent
  /// 
  /// Creates a deletion proof in the transparency ledger.
  /// 
  /// Returns revocation confirmation
  /// 
  /// Throws [HSKConsentException] if revocation fails
  Future<ConsentRevocation> revokeConsent(String consentId) async {
    _ensureInitialized();
    return _consentManager.revokeConsent(consentId);
  }
  
  /// Verify a consent record's integrity
  /// 
  /// Verifies the consent against the Merkle tree and hash chain.
  /// 
  /// Returns [ConsentVerification] result
  /// 
  /// Throws [HSKConsentException] if verification fails
  Future<ConsentVerification> verifyConsent(String consentId) async {
    _ensureInitialized();
    return _consentManager.verifyConsent(consentId);
  }
  
  /// Get all consents for the current identity
  /// 
  /// Returns list of [ConsentSummary] and total count
  /// 
  /// Throws [HSKConsentException] if fetch fails
  Future<ConsentList> getMyConsents() async {
    _ensureInitialized();
    return _consentManager.getMyConsents();
  }
  
  /// Authenticate using biometric (Face ID / Touch ID / Fingerprint)
  /// 
  /// Returns [AuthResult] with authentication status
  /// 
  /// Throws [HSKAuthException] if biometric authentication fails
  Future<AuthResult> authenticateWithBiometric() async {
    _ensureInitialized();
    
    final isAvailable = await _localAuth.canCheckBiometrics;
    if (!isAvailable) {
      throw HSKAuthException('Biometric authentication not available');
    }
    
    final didAuthenticate = await _localAuth.authenticate(
      localizedReason: 'Authenticate to access your HSK identity',
      options: const AuthenticationOptions(
        biometricOnly: false,
        stickyAuth: true,
      ),
    );
    
    return AuthResult(
      success: didAuthenticate,
      method: 'biometric',
    );
  }
  
  /// Check if biometric authentication is available
  Future<bool> isBiometricAvailable() async {
    _ensureInitialized();
    return await _localAuth.canCheckBiometrics;
  }
  
  /// Get available biometric types
  Future<List<BiometricType>> getAvailableBiometrics() async {
    _ensureInitialized();
    return await _localAuth.getAvailableBiometrics();
  }
  
  /// Export all data associated with the current identity
  /// 
  /// Returns JSON string of all user data
  /// 
  /// Throws [HSKException] if export fails
  Future<String> exportMyData() async {
    _ensureInitialized();
    
    final identity = await _identityManager.getCurrentIdentity();
    if (identity == null) {
      throw HSKException('No identity exists');
    }
    
    final response = await _apiClient.get(
      '/v1/identities/${identity.did}/export',
      headers: {'Authorization': 'Bearer ${identity.did}'},
    );
    
    if (response.statusCode != 200) {
      throw HSKException('Export failed: ${response.statusCode}');
    }
    
    return response.body;
  }
  
  /// Delete the current identity and all associated data
  /// 
  /// This action is irreversible. All consents will be revoked
  /// and the identity will be permanently deleted.
  /// 
  /// Returns deletion confirmation
  /// 
  /// Throws [HSKIdentityException] if deletion fails
  Future<IdentityDeletion> deleteMyIdentity() async {
    _ensureInitialized();
    return _identityManager.deleteIdentity();
  }
  
  /// Check if the SDK has been initialized
  bool get isInitialized => _initialized;
  
  /// Get the current SDK configuration
  HSKConfig? get config => _initialized ? _config : null;
  
  void _ensureInitialized() {
    if (!_initialized) {
      throw HSKInitializationException(
        'HSK SDK not initialized. Call initialize() first.'
      );
    }
  }
}

/// Convenience getter for the singleton instance
HSKSDK get HSK => HSKSDK();
