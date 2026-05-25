part of '../hsk_sdk.dart';

/// HTTP client for HSK API
class _ApiClient {
  final HSKConfig _config;
  final http.Client _client = http.Client();
  
  _ApiClient(this._config);
  
  /// Make a GET request
  Future<http.Response> get(
    String path, {
    Map<String, String>? headers,
  }) async {
    final url = _buildUrl(path);
    final requestHeaders = _buildHeaders(headers);
    
    return await _client.get(
      url,
      headers: requestHeaders,
    ).timeout(Duration(milliseconds: _config.timeout));
  }
  
  /// Make a POST request
  Future<http.Response> post(
    String path, {
    required Map<String, dynamic> body,
    Map<String, String>? headers,
  }) async {
    final url = _buildUrl(path);
    final requestHeaders = _buildHeaders(headers);
    
    return await _client.post(
      url,
      headers: requestHeaders,
      body: jsonEncode(body),
    ).timeout(Duration(milliseconds: _config.timeout));
  }
  
  /// Make a PUT request
  Future<http.Response> put(
    String path, {
    required Map<String, dynamic> body,
    Map<String, String>? headers,
  }) async {
    final url = _buildUrl(path);
    final requestHeaders = _buildHeaders(headers);
    
    return await _client.put(
      url,
      headers: requestHeaders,
      body: jsonEncode(body),
    ).timeout(Duration(milliseconds: _config.timeout));
  }
  
  /// Make a DELETE request
  Future<http.Response> delete(
    String path, {
    Map<String, String>? headers,
  }) async {
    final url = _buildUrl(path);
    final requestHeaders = _buildHeaders(headers);
    
    return await _client.delete(
      url,
      headers: requestHeaders,
    ).timeout(Duration(milliseconds: _config.timeout));
  }
  
  Uri _buildUrl(String path) {
    final baseUrl = _config.baseUrl.replaceAll(RegExp(r'/+$'), '');
    final cleanPath = path.replaceAll(RegExp(r'^/+'), '');
    return Uri.parse('$baseUrl/$cleanPath');
  }
  
  Map<String, String> _buildHeaders(Map<String, String>? additional) {
    final headers = <String, String>{
      'Content-Type': 'application/json',
      'Accept': 'application/json',
      'X-API-Key': _config.apiKey,
    };
    
    if (additional != null) {
      headers.addAll(additional);
    }
    
    return headers;
  }
  
  void dispose() {
    _client.close();
  }
}
