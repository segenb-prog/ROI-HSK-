package io.hskernel.sdk.models

import com.google.gson.annotations.SerializedName
import java.util.*

data class HSKConfig(
    val baseURL: String,
    val apiKey: String,
    val relyingPartyID: String = "hskernel.io"
)

data class HSKIdentity(
    @SerializedName("did") val did: String,
    @SerializedName("publicKey") val publicKey: String,
    @SerializedName("createdAt") val createdAt: Date
)

data class HSKConsent(
    @SerializedName("id") val id: String,
    @SerializedName("purpose") val purpose: String,
    @SerializedName("dataCategories") val dataCategories: List<String>,
    @SerializedName("retentionDays") val retentionDays: Int,
    @SerializedName("legalBasis") val legalBasis: HSKLegalBasis,
    @SerializedName("grantedAt") val grantedAt: Date,
    @SerializedName("expiresAt") val expiresAt: Date,
    @SerializedName("dataSubject") val dataSubject: String,
    @SerializedName("status") var status: HSKConsentStatus = HSKConsentStatus.ACTIVE,
    @SerializedName("signature") var signature: String? = null
) {
    fun hash(): ByteArray {
        val data = "$id:$purpose:${dataCategories.joinToString(",")}:$retentionDays:${legalBasis.name}:${grantedAt.time}:$dataSubject"
        return data.toByteArray()
    }
}

data class HSKRevocation(
    @SerializedName("consentId") val consentId: String,
    @SerializedName("revokedAt") val revokedAt: Date,
    @SerializedName("reason") val reason: String,
    @SerializedName("signature") var signature: String? = null
) {
    fun hash(): ByteArray {
        val data = "$consentId:${revokedAt.time}:$reason"
        return data.toByteArray()
    }
}

data class HSKVerificationResult(
    @SerializedName("valid") val valid: Boolean,
    @SerializedName("message") val message: String,
    @SerializedName("details") val details: Map<String, String>? = null
)

data class HSKExportResponse(
    @SerializedName("requestId") val requestId: String,
    @SerializedName("downloadUrl") val downloadUrl: String,
    @SerializedName("expiresAt") val expiresAt: String
)

data class HSKDeletionResponse(
    @SerializedName("requestId") val requestId: String,
    @SerializedName("status") val status: String,
    @SerializedName("estimatedCompletion") val estimatedCompletion: String
)

enum class HSKLegalBasis {
    @SerializedName("consent") CONSENT,
    @SerializedName("contract") CONTRACT,
    @SerializedName("legal_obligation") LEGAL_OBLIGATION,
    @SerializedName("vital_interests") VITAL_INTERESTS,
    @SerializedName("public_task") PUBLIC_TASK,
    @SerializedName("legitimate_interests") LEGITIMATE_INTERESTS
}

enum class HSKConsentStatus {
    @SerializedName("active") ACTIVE,
    @SerializedName("revoked") REVOKED,
    @SerializedName("expired") EXPIRED
}

enum class HSKExportFormat {
    JSON, JSONLD, CSV, XML, PDF
}

sealed class HSKException(message: String) : Exception(message) {
    class NotInitialized : HSKException("SDK not initialized. Call initialize() first.")
    class NotAuthenticated : HSKException("No identity. Create or restore an identity first.")
    class IdentityNotFound : HSKException("Identity not found.")
    class PrivateKeyNotFound : HSKException("Private key not found.")
    class NetworkError(msg: String) : HSKException("Network error: $msg")
    class ServerError(msg: String) : HSKException("Server error: $msg")
    class BiometricError(msg: String) : HSKException("Biometric error: $msg")
}
