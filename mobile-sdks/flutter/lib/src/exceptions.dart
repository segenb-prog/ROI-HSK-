part of '../hsk_sdk.dart';

/// Base exception for HSK SDK errors
class HSKException implements Exception {
  final String message;
  final dynamic cause;
  
  HSKException(this.message, {this.cause});
  
  @override
  String toString() {
    if (cause != null) {
      return 'HSKException: $message (caused by: $cause)';
    }
    return 'HSKException: $message';
  }
}

/// Exception thrown during SDK initialization
class HSKInitializationException extends HSKException {
  HSKInitializationException(String message, {dynamic cause}) 
      : super(message, cause: cause);
}

/// Exception thrown during identity operations
class HSKIdentityException extends HSKException {
  HSKIdentityException(String message, {dynamic cause}) 
      : super(message, cause: cause);
}

/// Exception thrown during consent operations
class HSKConsentException extends HSKException {
  HSKConsentException(String message, {dynamic cause}) 
      : super(message, cause: cause);
}

/// Exception thrown during authentication
class HSKAuthException extends HSKException {
  HSKAuthException(String message, {dynamic cause}) 
      : super(message, cause: cause);
}

/// Exception thrown during cryptographic operations
class HSKCryptoException extends HSKException {
  HSKCryptoException(String message, {dynamic cause}) 
      : super(message, cause: cause);
}

/// Exception thrown during API calls
class HSKApiException extends HSKException {
  final int statusCode;
  final String? responseBody;
  
  HSKApiException(
    String message, {
    required this.statusCode,
    this.responseBody,
    dynamic cause,
  }) : super(message, cause: cause);
  
  @override
  String toString() {
    return 'HSKApiException: $message (status: $statusCode, response: $responseBody)';
  }
}

/// Exception thrown when a required resource is not found
class HSKNotFoundException extends HSKApiException {
  HSKNotFoundException(String resource, {String? responseBody})
      : super(
          '$resource not found',
          statusCode: 404,
          responseBody: responseBody,
        );
}

/// Exception thrown when authentication fails
class HSKUnauthorizedException extends HSKApiException {
  HSKUnauthorizedException({String? message, String? responseBody})
      : super(
          message ?? 'Authentication required',
          statusCode: 401,
          responseBody: responseBody,
        );
}

/// Exception thrown when permission is denied
class HSKForbiddenException extends HSKApiException {
  HSKForbiddenException({String? message, String? responseBody})
      : super(
          message ?? 'Permission denied',
          statusCode: 403,
          responseBody: responseBody,
        );
}

/// Exception thrown when rate limit is exceeded
class HSKRateLimitException extends HSKApiException {
  final int? retryAfter;
  
  HSKRateLimitException({this.retryAfter, String? responseBody})
      : super(
          'Rate limit exceeded. Retry after ${retryAfter ?? "unknown"} seconds',
          statusCode: 429,
          responseBody: responseBody,
        );
}

/// Exception thrown when server error occurs
class HSKServerException extends HSKApiException {
  HSKServerException({String? message, int statusCode = 500, String? responseBody})
      : super(
          message ?? 'Server error',
          statusCode: statusCode,
          responseBody: responseBody,
        );
}
