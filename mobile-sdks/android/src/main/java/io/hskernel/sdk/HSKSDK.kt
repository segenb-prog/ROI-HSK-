package io.hskernel.sdk

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import androidx.biometric.BiometricPrompt
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import io.hskernel.sdk.api.HSKApiService
import io.hskernel.sdk.api.RetrofitClient
import io.hskernel.sdk.models.*
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.Signature
import java.util.*
import java.util.concurrent.Executors

/**
 * Main HSK SDK class for Android
 */
class HSKSDK private constructor() {
    
    companion object {
        @Volatile
        private var instance: HSKSDK? = null
        
        fun getInstance(): HSKSDK {
            return instance ?: synchronized(this) {
                instance ?: HSKSDK().also { instance = it }
            }
        }
    }
    
    private lateinit var context: Context
    private lateinit var config: HSKConfig
    private lateinit var apiService: HSKApiService
    private lateinit var securePrefs: EncryptedSharedPreferences
    private var currentIdentity: HSKIdentity? = null
    
    /**
     * Initialize the SDK
     */
    fun initialize(context: Context, config: HSKConfig) {
        this.context = context.applicationContext
        this.config = config
        this.apiService = RetrofitClient.create(config.baseURL, config.apiKey)
        
        // Initialize encrypted preferences
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        
        securePrefs = EncryptedSharedPreferences.create(
            context,
            "hsk_secure_prefs",
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        ) as EncryptedSharedPreferences
        
        // Try to restore existing identity
        val savedDID = securePrefs.getString("current_did", null)
        if (savedDID != null) {
            runCatching { restoreIdentitySync(savedDID) }
        }
    }
    
    val isInitialized: Boolean
        get() = ::config.isInitialized
    
    val identity: HSKIdentity?
        get() = currentIdentity
    
    // ==================== Identity Management ====================
    
    /**
     * Create a new identity
     */
    suspend fun createIdentity(): HSKIdentity = withContext(Dispatchers.IO) {
        checkInitialized()
        
        // Generate Ed25519 key pair using Android Keystore
        val keyPairGenerator = KeyPairGenerator.getInstance(
            KeyProperties.KEY_ALGORITHM_EC,
            "AndroidKeyStore"
        )
        
        val alias = "hsk_identity_${System.currentTimeMillis()}"
        val spec = KeyGenParameterSpec.Builder(
            alias,
            KeyProperties.PURPOSE_SIGN or KeyProperties.PURPOSE_VERIFY
        )
            .setDigests(KeyProperties.DIGEST_SHA256, KeyProperties.DIGEST_SHA512)
            .setUserAuthenticationRequired(false)
            .setRandomizedEncryptionRequired(true)
            .build()
        
        keyPairGenerator.initialize(spec)
        val keyPair = keyPairGenerator.generateKeyPair()
        
        // Create DID
        val publicKeyBytes = keyPair.public.encoded
        val publicKeyBase64 = Base64.getEncoder().encodeToString(publicKeyBytes)
        val did = "did:hsk:$publicKeyBase64"
        
        // Store key alias
        securePrefs.edit()
            .putString("key_alias_$did", alias)
            .putString("public_key_$did", publicKeyBase64)
            .apply()
        
        // Create identity
        val identity = HSKIdentity(
            did = did,
            publicKey = publicKeyBase64,
            createdAt = Date()
        )
        
        currentIdentity = identity
        securePrefs.edit()
            .putString("current_did", did)
            .apply()
        
        // Register with server
        registerIdentity(identity)
        
        identity
    }
    
    /**
     * Restore an existing identity
     */
    suspend fun restoreIdentity(did: String): HSKIdentity? = withContext(Dispatchers.IO) {
        checkInitialized()
        
        val alias = securePrefs.getString("key_alias_$did", null)
            ?: return@withContext null
        
        val keyStore = KeyStore.getInstance("AndroidKeyStore")
        keyStore.load(null)
        
        if (!keyStore.containsAlias(alias)) {
            return@withContext null
        }
        
        val publicKey = securePrefs.getString("public_key_$did", null)
            ?: return@withContext null
        
        val identity = HSKIdentity(
            did = did,
            publicKey = publicKey,
            createdAt = Date() // Would be fetched from server
        )
        
        currentIdentity = identity
        identity
    }
    
    private fun restoreIdentitySync(did: String): HSKIdentity? {
        val alias = securePrefs.getString("key_alias_$did", null) ?: return null
        
        val keyStore = KeyStore.getInstance("AndroidKeyStore")
        keyStore.load(null)
        
        if (!keyStore.containsAlias(alias)) return null
        
        val publicKey = securePrefs.getString("public_key_$did", null) ?: return null
        
        return HSKIdentity(did, publicKey, Date()).also { currentIdentity = it }
    }
    
    /**
     * Delete an identity
     */
    suspend fun deleteIdentity(did: String) = withContext(Dispatchers.IO) {
        val alias = securePrefs.getString("key_alias_$did", null) ?: return@withContext
        
        val keyStore = KeyStore.getInstance("AndroidKeyStore")
        keyStore.load(null)
        keyStore.deleteEntry(alias)
        
        securePrefs.edit()
            .remove("key_alias_$did")
            .remove("public_key_$did")
            .apply()
        
        if (currentIdentity?.did == did) {
            currentIdentity = null
            securePrefs.edit().remove("current_did").apply()
        }
    }
    
    // ==================== Consent Management ====================
    
    /**
     * Grant consent
     */
    suspend fun grantConsent(
        purpose: String,
        dataCategories: List<String>,
        retentionDays: Int = 365,
        legalBasis: HSKLegalBasis = HSKLegalBasis.CONSENT
    ): HSKConsent = withContext(Dispatchers.IO) {
        val identity = currentIdentity ?: throw HSKException.NotAuthenticated()
        
        val consent = HSKConsent(
            id = UUID.randomUUID().toString(),
            purpose = purpose,
            dataCategories = dataCategories,
            retentionDays = retentionDays,
            legalBasis = legalBasis,
            grantedAt = Date(),
            expiresAt = Calendar.getInstance().apply { add(Calendar.DAY_OF_YEAR, retentionDays) }.time,
            dataSubject = identity.did,
            status = HSKConsentStatus.ACTIVE
        )
        
        // Sign consent
        val signature = signData(consent.hash(), identity.did)
        consent.signature = Base64.getEncoder().encodeToString(signature)
        
        // Submit to server
        apiService.grantConsent(consent)
    }
    
    /**
     * Revoke consent
     */
    suspend fun revokeConsent(consentId: String, reason: String = "User initiated") {
        val identity = currentIdentity ?: throw HSKException.NotAuthenticated()
        
        val revocation = HSKRevocation(
            consentId = consentId,
            revokedAt = Date(),
            reason = reason
        )
        
        val signature = signData(revocation.hash(), identity.did)
        revocation.signature = Base64.getEncoder().encodeToString(signature)
        
        apiService.revokeConsent(revocation)
    }
    
    /**
     * Get consent history
     */
    suspend fun getConsentHistory(): List<HSKConsent> {
        val identity = currentIdentity ?: throw HSKException.NotAuthenticated()
        return apiService.getConsentHistory(identity.did)
    }
    
    /**
     * Verify consent
     */
    suspend fun verifyConsent(consentId: String): HSKVerificationResult {
        return apiService.verifyConsent(consentId)
    }
    
    // ==================== Biometric Authentication ====================
    
    /**
     * Authenticate with biometric
     */
    fun authenticateWithBiometric(
        activity: FragmentActivity,
        title: String = "Biometric Authentication",
        subtitle: String = "Authenticate to access your HSK identity",
        onSuccess: () -> Unit,
        onError: (HSKException) -> Unit
    ) {
        val executor = ContextCompat.getMainExecutor(activity)
        
        val promptInfo = BiometricPrompt.PromptInfo.Builder()
            .setTitle(title)
            .setSubtitle(subtitle)
            .setNegativeButtonText("Cancel")
            .setConfirmationRequired(false)
            .build()
        
        val biometricPrompt = BiometricPrompt(
            activity,
            executor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(result: BiometricPrompt.AuthenticationResult) {
                    onSuccess()
                }
                
                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    onError(HSKException.BiometricError(errString.toString()))
                }
                
                override fun onAuthenticationFailed() {
                    onError(HSKException.BiometricError("Authentication failed"))
                }
            }
        )
        
        biometricPrompt.authenticate(promptInfo)
    }
    
    // ==================== GDPR ====================
    
    /**
     * Export personal data (GDPR Article 20)
     */
    suspend fun exportPersonalData(format: HSKExportFormat = HSKExportFormat.JSON): String {
        val identity = currentIdentity ?: throw HSKException.NotAuthenticated()
        val response = apiService.requestDataExport(identity.did, format.name.lowercase())
        return response.downloadUrl
    }
    
    /**
     * Request data deletion (GDPR Article 17)
     */
    suspend fun requestDataDeletion(reason: String): String {
        val identity = currentIdentity ?: throw HSKException.NotAuthenticated()
        val response = apiService.requestDataDeletion(identity.did, reason)
        return response.requestId
    }
    
    // ==================== Private Methods ====================
    
    private fun checkInitialized() {
        if (!isInitialized) throw HSKException.NotInitialized()
    }
    
    private suspend fun registerIdentity(identity: HSKIdentity) {
        apiService.registerIdentity(identity)
    }
    
    private fun signData(data: ByteArray, did: String): ByteArray {
        val alias = securePrefs.getString("key_alias_$did", null)
            ?: throw HSKException.PrivateKeyNotFound()
        
        val keyStore = KeyStore.getInstance("AndroidKeyStore")
        keyStore.load(null)
        
        val entry = keyStore.getEntry(alias, null) as KeyStore.PrivateKeyEntry
        val signature = Signature.getInstance("SHA256withECDSA")
        signature.initSign(entry.privateKey)
        signature.update(data)
        return signature.sign()
    }
}
