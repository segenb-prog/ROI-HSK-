part of '../hsk_sdk.dart';

/// Cryptographic utilities for the HSK SDK
class _CryptoUtils {
  /// Generate an Ed25519 key pair
  static Future<KeyPair> generateKeyPair() async {
    final keyPair = await ed25519.generateKey();
    return KeyPair(
      publicKey: keyPair.publicKey.bytes,
      privateKey: keyPair.privateKey.bytes,
    );
  }
  
  /// Sign data with the private key
  static Future<Uint8List> sign(List<int> data, Uint8List privateKey) async {
    final privKey = ed25519.PrivateKey(privateKey);
    final signature = await ed25519.sign(privKey, Uint8List.fromList(data));
    return signature;
  }
  
  /// Verify a signature with the public key
  static Future<bool> verify(
    List<int> data,
    Uint8List signature,
    Uint8List publicKey,
  ) async {
    try {
      final pubKey = ed25519.PublicKey(publicKey);
      return await ed25519.verify(pubKey, Uint8List.fromList(data), signature);
    } catch (e) {
      return false;
    }
  }
  
  /// Compute SHA-256 hash
  static Uint8List sha256(List<int> data) {
    // In a real implementation, use pointycastle or crypto package
    // This is a placeholder
    return Uint8List(32);
  }
  
  /// Encode bytes to base64 URL-safe string
  static String base64UrlEncode(List<int> bytes) {
    return base64Url.encode(bytes).replaceAll('=', '');
  }
  
  /// Decode base64 URL-safe string to bytes
  static Uint8List base64UrlDecode(String str) {
    // Add padding if needed
    final padding = 4 - (str.length % 4);
    if (padding != 4) {
      str = str + ('=' * padding);
    }
    return base64Url.decode(str);
  }
}

/// Ed25519 key pair
class KeyPair {
  final Uint8List publicKey;
  final Uint8List privateKey;
  
  KeyPair({
    required this.publicKey,
    required this.privateKey,
  });
}
