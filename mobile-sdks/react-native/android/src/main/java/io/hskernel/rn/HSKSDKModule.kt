package io.hskernel.rn

import com.facebook.react.bridge.*
import com.facebook.react.modules.core.DeviceEventManagerModule
import kotlinx.coroutines.*
import org.json.JSONObject
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.spec.ECGenParameterSpec
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.biometric.BiometricPrompt
import androidx.fragment.app.FragmentActivity
import okhttp3.*
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException
import java.util.concurrent.TimeUnit

class HSKSDKModule(reactContext: ReactApplicationContext) : ReactContextBaseJavaModule(reactContext) {
    
    private val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    private val mainScope = CoroutineScope(Dispatchers.Main + SupervisorJob())
    private val ioScope = CoroutineScope(Dispatchers.IO + SupervisorJob())
    private val prefs = reactContext.getSharedPreferences("HSKSDK", android.content.Context.MODE_PRIVATE)
    private val httpClient = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(30, TimeUnit.SECONDS)
        .writeTimeout(30, TimeUnit.SECONDS)
        .build()
    
    private var baseUrl: String = ""
    private var apiKey: String = ""
    
    override fun getName(): String = "HSKSDK"
    
    @ReactMethod
    fun initialize(config: ReadableMap, promise: Promise) {
        ioScope.launch {
            try {
                baseUrl = config.getString("baseUrl") ?: throw IllegalArgumentException("baseUrl required")
                apiKey = config.getString("apiKey") ?: throw IllegalArgumentException("apiKey required")
                
                // Verify connection
                val request = Request.Builder()
                    .url("$baseUrl/v1/health")
                    .header("X-API-Key", apiKey)
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    if (response.isSuccessful) {
                        promise.resolve(Arguments.createMap().apply {
                            putBoolean("success", true)
                            putString("version", response.body?.string())
                        })
                    } else {
                        promise.reject("INIT_ERROR", "Failed to connect: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("INIT_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun createIdentity(promise: Promise) {
        ioScope.launch {
            try {
                // Generate EC key pair in Android Keystore
                val keyPairGenerator = KeyPairGenerator.getInstance(
                    KeyProperties.KEY_ALGORITHM_EC,
                    "AndroidKeyStore"
                )
                
                val params = KeyGenParameterSpec.Builder(
                    "hsk_identity_key",
                    KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
                )
                    .setAlgorithmParameterSpec(ECGenParameterSpec("secp256r1"))
                    .setUserAuthenticationRequired(false)
                    .setRandomizedEncryptionRequired(true)
                    .build()
                
                keyPairGenerator.initialize(params)
                val keyPair = keyPairGenerator.generateKeyPair()
                
                val publicKey = keyPair.public.encoded
                val did = "did:hsk:${Base64.encodeToString(publicKey, Base64.URL_SAFE or Base64.NO_WRAP)}"
                
                // Store DID locally
                prefs.edit().putString("did", did).apply()
                
                // Register with server
                val registration = JSONObject().apply {
                    put("did", did)
                    put("public_key", Base64.encodeToString(publicKey, Base64.DEFAULT))
                    put("device_info", JSONObject().apply {
                        put("platform", "android")
                        put("model", android.os.Build.MODEL)
                        put("os_version", android.os.Build.VERSION.RELEASE)
                    })
                }
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/identities")
                    .header("X-API-Key", apiKey)
                    .header("Content-Type", "application/json")
                    .post(registration.toString().toRequestBody("application/json".toMediaType()))
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    if (response.isSuccessful) {
                        val result = Arguments.createMap()
                        result.putString("did", did)
                        result.putString("createdAt", java.time.Instant.now().toString())
                        promise.resolve(result)
                    } else {
                        promise.reject("REGISTRATION_ERROR", "Server registration failed: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("IDENTITY_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun grantConsent(params: ReadableMap, promise: Promise) {
        ioScope.launch {
            try {
                val did = prefs.getString("did", null) ?: throw IllegalStateException("No identity created")
                val purpose = params.getString("purpose") ?: throw IllegalArgumentException("purpose required")
                val dataCategories = params.getArray("dataCategories") ?: throw IllegalArgumentException("dataCategories required")
                
                // Get private key from Keystore
                val privateKey = keyStore.getEntry("hsk_identity_key", null) as KeyStore.PrivateKeyEntry
                
                // Build consent payload
                val consentPayload = JSONObject().apply {
                    put("did", did)
                    put("purpose", purpose)
                    put("data_categories", dataCategories.toArrayList())
                    put("valid_from", java.time.Instant.now().toString())
                    put("valid_until", params.getString("validUntil") ?: 
                        java.time.Instant.now().plusSeconds(31536000).toString())
                    put("constraints", params.getMap("constraints")?.toHashMap() ?: JSONObject.NULL)
                    put("scope", params.getString("scope") ?: "general")
                }
                
                // Sign the consent
                val signature = java.security.Signature.getInstance("SHA256withECDSA").apply {
                    initSign(privateKey.privateKey)
                    update(consentPayload.toString().toByteArray())
                }.sign()
                
                val requestBody = JSONObject().apply {
                    put("payload", consentPayload)
                    put("signature", Base64.encodeToString(signature, Base64.DEFAULT))
                    put("algorithm", "ECDSA-P256-SHA256")
                }
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/consent")
                    .header("Authorization", "Bearer $did")
                    .header("X-API-Key", apiKey)
                    .header("Content-Type", "application/json")
                    .post(requestBody.toString().toRequestBody("application/json".toMediaType()))
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    val responseBody = response.body?.string()
                    if (response.isSuccessful && responseBody != null) {
                        val json = JSONObject(responseBody)
                        val result = Arguments.createMap()
                        result.putString("consentId", json.getString("consent_id"))
                        result.putString("merkleRoot", json.getString("merkle_root"))
                        result.putString("timestamp", json.getString("timestamp"))
                        result.putString("txHash", json.optString("tx_hash", ""))
                        promise.resolve(result)
                    } else {
                        promise.reject("CONSENT_ERROR", "Failed to grant consent: ${response.code} - $responseBody")
                    }
                }
            } catch (e: Exception) {
                promise.reject("CONSENT_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun revokeConsent(consentId: String, promise: Promise) {
        ioScope.launch {
            try {
                val did = prefs.getString("did", null) ?: throw IllegalStateException("No identity created")
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/consent/$consentId")
                    .header("Authorization", "Bearer $did")
                    .header("X-API-Key", apiKey)
                    .delete()
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    if (response.isSuccessful) {
                        val result = Arguments.createMap()
                        result.putString("consentId", consentId)
                        result.putString("revokedAt", java.time.Instant.now().toString())
                        promise.resolve(result)
                    } else {
                        promise.reject("REVOKE_ERROR", "Failed to revoke consent: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("REVOKE_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun verifyConsent(consentId: String, promise: Promise) {
        ioScope.launch {
            try {
                val request = Request.Builder()
                    .url("$baseUrl/v1/consent/$consentId/verify")
                    .header("X-API-Key", apiKey)
                    .get()
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    val responseBody = response.body?.string()
                    if (response.isSuccessful && responseBody != null) {
                        val json = JSONObject(responseBody)
                        val result = Arguments.createMap()
                        result.putBoolean("valid", json.getBoolean("valid"))
                        result.putString("verifiedAt", json.getString("verified_at"))
                        result.putString("merkleProof", json.optString("merkle_proof", ""))
                        promise.resolve(result)
                    } else {
                        promise.reject("VERIFY_ERROR", "Verification failed: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("VERIFY_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun getMyConsents(promise: Promise) {
        ioScope.launch {
            try {
                val did = prefs.getString("did", null) ?: throw IllegalStateException("No identity created")
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/identities/$did/consent")
                    .header("Authorization", "Bearer $did")
                    .header("X-API-Key", apiKey)
                    .get()
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    val responseBody = response.body?.string()
                    if (response.isSuccessful && responseBody != null) {
                        val json = JSONObject(responseBody)
                        val consentsArray = json.getJSONArray("consents")
                        val result = Arguments.createArray()
                        
                        for (i in 0 until consentsArray.length()) {
                            val consent = consentsArray.getJSONObject(i)
                            val consentMap = Arguments.createMap()
                            consentMap.putString("consentId", consent.getString("consent_id"))
                            consentMap.putString("purpose", consent.getString("purpose"))
                            consentMap.putString("status", consent.getString("status"))
                            consentMap.putString("createdAt", consent.getString("created_at"))
                            consentMap.putString("expiresAt", consent.optString("expires_at", ""))
                            result.pushMap(consentMap)
                        }
                        
                        val finalResult = Arguments.createMap()
                        finalResult.putArray("consents", result)
                        finalResult.putInt("total", json.getInt("total"))
                        promise.resolve(finalResult)
                    } else {
                        promise.reject("FETCH_ERROR", "Failed to fetch consents: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("FETCH_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun authenticateWithBiometric(promise: Promise) {
        mainScope.launch {
            try {
                val activity = currentActivity as? FragmentActivity 
                    ?: throw IllegalStateException("Need FragmentActivity")
                
                val executor = androidx.core.content.ContextCompat.getMainExecutor(activity)
                
                val biometricPrompt = BiometricPrompt(activity, executor,
                    object : BiometricPrompt.AuthenticationCallback() {
                        override fun onAuthenticationSucceeded(result: AuthenticationResult) {
                            val resultMap = Arguments.createMap()
                            resultMap.putBoolean("success", true)
                            resultMap.putString("method", "biometric")
                            promise.resolve(resultMap)
                        }
                        
                        override fun onAuthenticationFailed() {
                            promise.reject("AUTH_FAILED", "Biometric authentication failed")
                        }
                        
                        override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                            promise.reject("AUTH_ERROR", "Biometric error: $errString")
                        }
                    })
                
                val promptInfo = BiometricPrompt.PromptInfo.Builder()
                    .setTitle("Authenticate")
                    .setSubtitle("Verify your identity")
                    .setNegativeButtonText("Cancel")
                    .setConfirmationRequired(false)
                    .build()
                
                biometricPrompt.authenticate(promptInfo)
            } catch (e: Exception) {
                promise.reject("BIOMETRIC_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun exportMyData(promise: Promise) {
        ioScope.launch {
            try {
                val did = prefs.getString("did", null) ?: throw IllegalStateException("No identity created")
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/identities/$did/export")
                    .header("Authorization", "Bearer $did")
                    .header("X-API-Key", apiKey)
                    .get()
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    val responseBody = response.body?.string()
                    if (response.isSuccessful && responseBody != null) {
                        promise.resolve(responseBody)
                    } else {
                        promise.reject("EXPORT_ERROR", "Export failed: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("EXPORT_ERROR", e.message)
            }
        }
    }
    
    @ReactMethod
    fun deleteMyIdentity(promise: Promise) {
        ioScope.launch {
            try {
                val did = prefs.getString("did", null) ?: throw IllegalStateException("No identity created")
                
                val request = Request.Builder()
                    .url("$baseUrl/v1/identities/$did")
                    .header("Authorization", "Bearer $did")
                    .header("X-API-Key", apiKey)
                    .delete()
                    .build()
                
                httpClient.newCall(request).execute().use { response ->
                    if (response.isSuccessful) {
                        // Clear local data
                        prefs.edit().clear().apply()
                        keyStore.deleteEntry("hsk_identity_key")
                        
                        val result = Arguments.createMap()
                        result.putBoolean("deleted", true)
                        result.putString("deletedAt", java.time.Instant.now().toString())
                        promise.resolve(result)
                    } else {
                        promise.reject("DELETE_ERROR", "Deletion failed: ${response.code}")
                    }
                }
            } catch (e: Exception) {
                promise.reject("DELETE_ERROR", e.message)
            }
        }
    }
    
    override fun onCatalystInstanceDestroy() {
        mainScope.cancel()
        ioScope.cancel()
        super.onCatalystInstanceDestroy()
    }
}
