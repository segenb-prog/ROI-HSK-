package io.hskernel.sdk.api

import io.hskernel.sdk.models.*
import retrofit2.http.*

interface HSKApiService {
    
    @POST("/identity")
    suspend fun registerIdentity(@Body identity: HSKIdentity)
    
    @POST("/consent")
    suspend fun grantConsent(@Body consent: HSKConsent): HSKConsent
    
    @POST("/consent/revoke")
    suspend fun revokeConsent(@Body revocation: HSKRevocation)
    
    @GET("/consent/history")
    suspend fun getConsentHistory(@Query("did") did: String): List<HSKConsent>
    
    @GET("/consent/verify/{consentId}")
    suspend fun verifyConsent(@Path("consentId") consentId: String): HSKVerificationResult
    
    @POST("/gdpr/export")
    suspend fun requestDataExport(
        @Query("did") did: String,
        @Query("format") format: String
    ): HSKExportResponse
    
    @POST("/gdpr/delete")
    suspend fun requestDataDeletion(
        @Query("did") did: String,
        @Query("reason") reason: String
    ): HSKDeletionResponse
    
    @GET("/health")
    suspend fun healthCheck(): HealthResponse
}

data class HealthResponse(
    val status: String,
    val version: String
)
