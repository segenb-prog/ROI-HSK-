part of '../hsk_sdk.dart';

/// Identity information
class HSKIdentity {
  /// Decentralized identifier
  final String did;
  
  /// Creation timestamp (ISO 8601)
  final String createdAt;
  
  HSKIdentity({
    required this.did,
    required this.createdAt,
  });
  
  factory HSKIdentity.fromJson(Map<String, dynamic> json) {
    return HSKIdentity(
      did: json['did'] as String,
      createdAt: json['createdAt'] as String,
    );
  }
  
  Map<String, dynamic> toJson() => {
    'did': did,
    'createdAt': createdAt,
  };
  
  @override
  String toString() => 'HSKIdentity(did: $did, createdAt: $createdAt)';
}

/// Consent request parameters
class ConsentRequest {
  /// Purpose of data processing
  final String purpose;
  
  /// Categories of data being consented to
  final List<String> dataCategories;
  
  /// Number of days until expiration (default: 365)
  final int? expiresInDays;
  
  /// Specific constraints on data usage
  final Map<String, dynamic>? constraints;
  
  /// Scope of consent (default: 'general')
  final String? scope;
  
  ConsentRequest({
    required this.purpose,
    required this.dataCategories,
    this.expiresInDays,
    this.constraints,
    this.scope,
  });
  
  Map<String, dynamic> toJson() => {
    'purpose': purpose,
    'dataCategories': dataCategories,
    if (expiresInDays != null) 'expiresInDays': expiresInDays,
    if (constraints != null) 'constraints': constraints,
    if (scope != null) 'scope': scope,
  };
}

/// Granted consent information
class HSKConsent {
  /// Unique consent identifier
  final String consentId;
  
  /// Merkle root for verification
  final String merkleRoot;
  
  /// Timestamp of consent creation (ISO 8601)
  final String timestamp;
  
  /// Transaction hash (if applicable)
  final String? txHash;
  
  HSKConsent({
    required this.consentId,
    required this.merkleRoot,
    required this.timestamp,
    this.txHash,
  });
  
  factory HSKConsent.fromJson(Map<String, dynamic> json) {
    return HSKConsent(
      consentId: json['consentId'] as String,
      merkleRoot: json['merkleRoot'] as String,
      timestamp: json['timestamp'] as String,
      txHash: json['txHash'] as String?,
    );
  }
  
  Map<String, dynamic> toJson() => {
    'consentId': consentId,
    'merkleRoot': merkleRoot,
    'timestamp': timestamp,
    if (txHash != null) 'txHash': txHash,
  };
  
  @override
  String toString() => 'HSKConsent(consentId: $consentId, timestamp: $timestamp)';
}

/// Consent verification result
class ConsentVerification {
  /// Whether the consent is valid
  final bool valid;
  
  /// Verification timestamp (ISO 8601)
  final String verifiedAt;
  
  /// Merkle proof for verification
  final String? merkleProof;
  
  ConsentVerification({
    required this.valid,
    required this.verifiedAt,
    this.merkleProof,
  });
  
  factory ConsentVerification.fromJson(Map<String, dynamic> json) {
    return ConsentVerification(
      valid: json['valid'] as bool,
      verifiedAt: json['verifiedAt'] as String,
      merkleProof: json['merkleProof'] as String?,
    );
  }
  
  Map<String, dynamic> toJson() => {
    'valid': valid,
    'verifiedAt': verifiedAt,
    if (merkleProof != null) 'merkleProof': merkleProof,
  };
}

/// Consent summary for listing
class ConsentSummary {
  /// Unique consent identifier
  final String consentId;
  
  /// Purpose of the consent
  final String purpose;
  
  /// Current status
  final ConsentStatus status;
  
  /// Creation timestamp (ISO 8601)
  final String createdAt;
  
  /// Expiration timestamp (ISO 8601), if applicable
  final String? expiresAt;
  
  ConsentSummary({
    required this.consentId,
    required this.purpose,
    required this.status,
    required this.createdAt,
    this.expiresAt,
  });
  
  factory ConsentSummary.fromJson(Map<String, dynamic> json) {
    return ConsentSummary(
      consentId: json['consentId'] as String,
      purpose: json['purpose'] as String,
      status: ConsentStatus.values.firstWhere(
        (e) => e.name == json['status'],
        orElse: () => ConsentStatus.unknown,
      ),
      createdAt: json['createdAt'] as String,
      expiresAt: json['expiresAt'] as String?,
    );
  }
  
  Map<String, dynamic> toJson() => {
    'consentId': consentId,
    'purpose': purpose,
    'status': status.name,
    'createdAt': createdAt,
    if (expiresAt != null) 'expiresAt': expiresAt,
  };
}

/// Consent status enum
enum ConsentStatus {
  active,
  revoked,
  expired,
  unknown,
}

/// List of consents with metadata
class ConsentList {
  /// List of consent summaries
  final List<ConsentSummary> consents;
  
  /// Total number of consents
  final int total;
  
  ConsentList({
    required this.consents,
    required this.total,
  });
  
  factory ConsentList.fromJson(Map<String, dynamic> json) {
    final consentsList = (json['consents'] as List)
        .map((e) => ConsentSummary.fromJson(e as Map<String, dynamic>))
        .toList();
    
    return ConsentList(
      consents: consentsList,
      total: json['total'] as int,
    );
  }
}

/// Consent revocation confirmation
class ConsentRevocation {
  /// ID of the revoked consent
  final String consentId;
  
  /// Revocation timestamp (ISO 8601)
  final String revokedAt;
  
  ConsentRevocation({
    required this.consentId,
    required this.revokedAt,
  });
  
  factory ConsentRevocation.fromJson(Map<String, dynamic> json) {
    return ConsentRevocation(
      consentId: json['consentId'] as String,
      revokedAt: json['revokedAt'] as String,
    );
  }
}

/// Authentication result
class AuthResult {
  /// Whether authentication succeeded
  final bool success;
  
  /// Authentication method used
  final String method;
  
  AuthResult({
    required this.success,
    required this.method,
  });
  
  factory AuthResult.fromJson(Map<String, dynamic> json) {
    return AuthResult(
      success: json['success'] as bool,
      method: json['method'] as String,
    );
  }
  
  Map<String, dynamic> toJson() => {
    'success': success,
    'method': method,
  };
}

/// Identity deletion confirmation
class IdentityDeletion {
  /// Whether deletion succeeded
  final bool deleted;
  
  /// Deletion timestamp (ISO 8601)
  final String deletedAt;
  
  IdentityDeletion({
    required this.deleted,
    required this.deletedAt,
  });
  
  factory IdentityDeletion.fromJson(Map<String, dynamic> json) {
    return IdentityDeletion(
      deleted: json['deleted'] as bool,
      deletedAt: json['deletedAt'] as String,
    );
  }
}

/// Device information for identity registration
class DeviceInfo {
  /// Platform (ios/android)
  final String platform;
  
  /// Device model
  final String model;
  
  /// OS version
  final String osVersion;
  
  /// Unique device identifier
  final String? deviceId;
  
  DeviceInfo({
    required this.platform,
    required this.model,
    required this.osVersion,
    this.deviceId,
  });
  
  Map<String, dynamic> toJson() => {
    'platform': platform,
    'model': model,
    'os_version': osVersion,
    if (deviceId != null) 'device_id': deviceId,
  };
  
  static Future<DeviceInfo> current() async {
    final deviceInfo = DeviceInfoPlugin();
    
    String platform;
    String model;
    String osVersion;
    String? deviceId;
    
    if (Platform.isIOS) {
      final iosInfo = await deviceInfo.iosInfo;
      platform = 'ios';
      model = iosInfo.model ?? 'unknown';
      osVersion = iosInfo.systemVersion ?? 'unknown';
      deviceId = iosInfo.identifierForVendor;
    } else if (Platform.isAndroid) {
      final androidInfo = await deviceInfo.androidInfo;
      platform = 'android';
      model = androidInfo.model ?? 'unknown';
      osVersion = androidInfo.version.release ?? 'unknown';
      deviceId = androidInfo.id;
    } else {
      platform = 'unknown';
      model = 'unknown';
      osVersion = 'unknown';
    }
    
    return DeviceInfo(
      platform: platform,
      model: model,
      osVersion: osVersion,
      deviceId: deviceId,
    );
  }
}
