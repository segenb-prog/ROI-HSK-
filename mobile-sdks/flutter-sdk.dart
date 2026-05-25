// HSK Flutter SDK
// Dart 3.0+

import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/services.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:http/http.dart' as http;
import 'package:local_auth/local_auth.dart';
import 'package:pointycastle/export.dart';

/// HSK SDK Configuration
class HSKConfig {
  final String baseURL;
  final String apiKey;
  final String? relyingPartyID;

  HSKConfig({
    required this.baseURL,
    required this.apiKey,
    this.relyingPartyID = 'hskernel.io',
  });
}

/// Legal Basis for consent
enum HSKLegalBasis {
  consent,
  contract,
  legalObligation,
  vitalInterests,
  publicTask,
  legitimateInterests,
}

/// HSK Identity
class HSKIdentity {
  final String did;
  final Uint8List publicKey;

  HSKIdentity({required this.did, required this.publicKey});

  Map<String, dynamic> toJson() => {
    'did': did,
    'publicKey': base64Encode(publicKey),
  };
}

/// HSK Consent
class HSKConsent {
  final String id;
  final String purpose;
  final List<String> dataCategories;
  final int retentionDays;
  final HSKLegalBasis legalBasis;
  final DateTime grantedAt;
  final String dataSubject;
  Uint8List? signature;

  HSKConsent({
    required this.id,
    required this.purpose,
    required this.dataCategories,
    required this.retentionDays,
    required this.legalBasis,
    required this.grantedAt,
    required this.dataSubject,
    this.signature,
  });

  Map<String, dynamic> toJson() => {
    'id': id,
    'purpose': purpose,
    'dataCategories': dataCategories,
    'retentionDays': retentionDays,
    'legalBasis': legalBasis.name,
    'grantedAt': grantedAt.toIso8601String(),
    'dataSubject': dataSubject,
    'signature': signature != null ? base64Encode(signature!) : null,
  };

  Uint8List hash() {
    final data = jsonEncode({
      'purpose': purpose,
      'dataCategories': dataCategories,
      'retentionDays': retentionDays,
      'legalBasis': legalBasis.name,
      'grantedAt': grantedAt.toIso8601String(),
      'dataSubject': dataSubject,
    });
    return _sha256(Uint8List.fromList(utf8.encode(data)));
  }
}

/// HSK SDK Exception
class HSKException implements Exception {
  final String message;
  final HSKErrorCode code;

  HSKException(this.message, this.code);

  @override
  String toString() => 'HSKException: $message';
}

enum HSKErrorCode {
  notInitialized,
  notAuthenticated,
  identityNotFound,
  privateKeyNotFound,
  networkError,
  serverError,
  biometricError,
}

/// HSK SDK
class HSKSDK {
  static final HSKSDK _instance = HSKSDK._internal();
  factory HSKSDK() => _instance;
  HSKSDK._internal();

  HSKConfig? _config;
  final _secureStorage = const FlutterSecureStorage();
  final _localAuth = LocalAuthentication();
  HSKIdentity? _currentIdentity;

  bool get isInitialized => _config != null;
  HSKIdentity? get currentIdentity => _currentIdentity;

  /// Initialize the SDK
  Future<void> initialize(HSKConfig config) async {
    _config = config;
    
    // Try to restore existing identity
    final savedDID = await _secureStorage.read(key: 'hsk_current_did');
    if (savedDID != null) {
      _currentIdentity = await restoreIdentity(savedDID);
    }
  }

  /// Create a new identity
  Future<HSKIdentity> createIdentity() async {
    _ensureInitialized();

    // Generate Ed25519 key pair
    final keyPair = _generateKeyPair();
    final publicKey = keyPair.publicKey;
    
    // Create DID
    final did = 'did:hsk:${base64Encode(publicKey)}';
    
    // Store private key securely
    await _secureStorage.write(
      key: 'hsk_private_key_$did',
      value: base64Encode(keyPair.privateKey),
    );
    
    final identity = HSKIdentity(did: did, publicKey: publicKey);
    _currentIdentity = identity;
    await _secureStorage.write(key: 'hsk_current_did', value: did);
    
    return identity;
  }

  /// Restore an existing identity
  Future<HSKIdentity?> restoreIdentity(String did) async {
    _ensureInitialized();

    // Check if private key exists
    final privateKeyStr = await _secureStorage.read(key: 'hsk_private_key_$did');
    if (privateKeyStr == null) {
      return null;
    }
    
    // Extract public key from DID
    final publicKeyBase64 = did.replaceFirst('did:hsk:', '');
    final publicKey = base64Decode(publicKeyBase64);
    
    final identity = HSKIdentity(did: did, publicKey: publicKey);
    _currentIdentity = identity;
    return identity;
  }

  /// Grant consent
  Future<HSKConsent> grantConsent({
    required String purpose,
    required List<String> dataCategories,
    required int retentionDays,
    required HSKLegalBasis legalBasis,
  }) async {
    _ensureInitialized();
    _ensureAuthenticated();

    final consent = HSKConsent(
      id: _generateUUID(),
      purpose: purpose,
      dataCategories: dataCategories,
      retentionDays: retentionDays,
      legalBasis: legalBasis,
      grantedAt: DateTime.now(),
      dataSubject: _currentIdentity!.did,
    );

    // Sign consent
    final signature = await _sign(consent.hash(), _currentIdentity!.did);
    consent.signature = signature;

    // Submit to server
    final response = await http.post(
      Uri.parse('${_config!.baseURL}/consent'),
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer ${_config!.apiKey}',
      },
      body: jsonEncode(consent.toJson()),
    );

    if (response.statusCode != 200) {
      throw HSKException('Failed to submit consent', HSKErrorCode.serverError);
    }

    return consent;
  }

  /// Revoke consent
  Future<void> revokeConsent(String consentId) async {
    _ensureInitialized();
    _ensureAuthenticated();

    final revocation = {
      'consentId': consentId,
      'revokedAt': DateTime.now().toIso8601String(),
      'reason': 'User initiated',
    };

    final response = await http.post(
      Uri.parse('${_config!.baseURL}/consent/revoke'),
      headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer ${_config!.apiKey}',
      },
      body: jsonEncode(revocation),
    );

    if (response.statusCode != 200) {
      throw HSKException('Failed to revoke consent', HSKErrorCode.serverError);
    }
  }

  /// Get consent history
  Future<List<HSKConsent>> getConsentHistory() async {
    _ensureInitialized();
    _ensureAuthenticated();

    final response = await http.get(
      Uri.parse('${_config!.baseURL}/consent/history?did=${_currentIdentity!.did}'),
      headers: {
        'Authorization': 'Bearer ${_config!.apiKey}',
      },
    );

    if (response.statusCode != 200) {
      throw HSKException('Failed to fetch consent history', HSKErrorCode.serverError);
    }

    final List<dynamic> data = jsonDecode(response.body);
    return data.map((json) => HSKConsent(
      id: json['id'],
      purpose: json['purpose'],
      dataCategories: List<String>.from(json['dataCategories']),
      retentionDays: json['retentionDays'],
      legalBasis: HSKLegalBasis.values.byName(json['legalBasis']),
      grantedAt: DateTime.parse(json['grantedAt']),
      dataSubject: json['dataSubject'],
    )).toList();
  }

  /// Authenticate with biometric
  Future<bool> authenticateWithBiometric() async {
    final isAvailable = await _localAuth.canCheckBiometrics;
    if (!isAvailable) {
      return false;
    }

    try {
      return await _localAuth.authenticate(
        localizedReason: 'Authenticate to access your HSK identity',
        options: const AuthenticationOptions(
          biometricOnly: true,
          stickyAuth: true,
        ),
      );
    } catch (e) {
      throw HSKException('Biometric authentication failed', HSKErrorCode.biometricError);
    }
  }

  /// Logout
  Future<void> logout() async {
    _currentIdentity = null;
    await _secureStorage.delete(key: 'hsk_current_did');
  }

  // Private methods
  void _ensureInitialized() {
    if (_config == null) {
      throw HSKException('SDK not initialized', HSKErrorCode.notInitialized);
    }
  }

  void _ensureAuthenticated() {
    if (_currentIdentity == null) {
      throw HSKException('User not authenticated', HSKErrorCode.notAuthenticated);
    }
  }

  _KeyPair _generateKeyPair() {
    final secureRandom = FortunaRandom();
    final keyParams = ED25519KeyGenerationParameters(secureRandom);
    final keyGenerator = ED25519KeyGenerator();
    keyGenerator.init(keyParams);
    
    final keyPair = keyGenerator.generateKeyPair();
    final privateKey = (keyPair.privateKey as EDPrivateKeyParameters).d;
    final publicKey = (keyPair.publicKey as EDPublicKeyParameters).Q;
    
    return _KeyPair(
      privateKey: privateKey,
      publicKey: publicKey,
    );
  }

  Future<Uint8List> _sign(Uint8List data, String did) async {
    final privateKeyStr = await _secureStorage.read(key: 'hsk_private_key_$did');
    if (privateKeyStr == null) {
      throw HSKException('Private key not found', HSKErrorCode.privateKeyNotFound);
    }

    final privateKey = base64Decode(privateKeyStr);
    
    // Sign using Ed25519
    final signer = ED25519Signer();
    final params = EDPrivateKeyParameters(privateKey);
    signer.init(true, params);
    
    final signature = signer.generateSignature(data);
    return signature;
  }

  Uint8List _sha256(Uint8List data) {
    final digest = SHA256Digest();
    return digest.process(data);
  }

  String _generateUUID() {
    return '${_randomHex(8)}-${_randomHex(4)}-4${_randomHex(3)}-${_randomHex(4)}-${_randomHex(12)}';
  }

  String _randomHex(int length) {
    const chars = '0123456789abcdef';
    return List.generate(length, (_) => chars[DateTime.now().millisecond % 16]).join();
  }
}

class _KeyPair {
  final Uint8List privateKey;
  final Uint8List publicKey;

  _KeyPair({required this.privateKey, required this.publicKey});
}

// Widgets
import 'package:flutter/material.dart';

class HSKConsentButton extends StatelessWidget {
  final String purpose;
  final List<String> dataCategories;
  final int retentionDays;
  final HSKLegalBasis legalBasis;
  final VoidCallback? onSuccess;
  final Function(String)? onError;

  const HSKConsentButton({
    Key? key,
    required this.purpose,
    required this.dataCategories,
    required this.retentionDays,
    required this.legalBasis,
    this.onSuccess,
    this.onError,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return ElevatedButton(
      onPressed: _handlePress,
      child: const Text('Grant Consent'),
    );
  }

  Future<void> _handlePress() async {
    try {
      final sdk = HSKSDK();
      
      // Authenticate with biometric
      final authenticated = await sdk.authenticateWithBiometric();
      if (!authenticated) {
        onError?.call('Biometric authentication required');
        return;
      }

      // Grant consent
      await sdk.grantConsent(
        purpose: purpose,
        dataCategories: dataCategories,
        retentionDays: retentionDays,
        legalBasis: legalBasis,
      );

      onSuccess?.call();
    } catch (e) {
      onError?.call(e.toString());
    }
  }
}

class HSKConsentHistory extends StatelessWidget {
  const HSKConsentHistory({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<List<HSKConsent>>(
      future: HSKSDK().getConsentHistory(),
      builder: (context, snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const CircularProgressIndicator();
        }

        if (snapshot.hasError) {
          return Text('Error: ${snapshot.error}');
        }

        final consents = snapshot.data ?? [];
        
        return ListView.builder(
          itemCount: consents.length,
          itemBuilder: (context, index) {
            final consent = consents[index];
            return ListTile(
              title: Text(consent.purpose),
              subtitle: Text('Granted: ${consent.grantedAt}'),
              trailing: IconButton(
                icon: const Icon(Icons.delete),
                onPressed: () => HSKSDK().revokeConsent(consent.id),
              ),
            );
          },
        );
      },
    );
  }
}
