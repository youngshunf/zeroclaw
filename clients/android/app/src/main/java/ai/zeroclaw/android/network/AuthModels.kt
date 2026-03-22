package ai.zeroclaw.android.network

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** 后端 API 统一响应信封 */
@Serializable
data class ApiResponse<T>(
    val code: Int,
    val msg: String,
    val data: T? = null
)

/** POST /api/v1/auth/send-code 请求 */
@Serializable
data class SendCodeRequest(
    val phone: String
)

/** POST /api/v1/auth/send-code 响应 */
@Serializable
data class SendCodeResponse(
    val success: Boolean,
    val message: String? = null
)

/** POST /api/v1/auth/phone-login 请求 */
@Serializable
data class PhoneLoginRequest(
    val phone: String,
    val code: String
)

/** POST /api/v1/auth/phone-login 响应 */
@Serializable
data class LoginResponse(
    val access_token: String,
    val access_token_expire_time: String,
    val refresh_token: String,
    val refresh_token_expire_time: String,
    val llm_token: String,
    val llm_base_url: String? = null,
    val gateway_token: String? = null,
    val is_new_user: Boolean = false,
    val user: UserInfo
)

@Serializable
data class UserInfo(
    val uuid: String,
    val username: String,
    val nickname: String,
    val phone: String? = null,
    val email: String? = null,
    val avatar: String? = null,
    val is_new_user: Boolean? = null
)

/** POST /api/v1/auth/refresh 请求 */
@Serializable
data class RefreshRequest(
    val refresh_token: String
)

/** POST /api/v1/auth/refresh 响应 */
@Serializable
data class RefreshResponse(
    val access_token: String,
    val access_token_expire_time: String,
    val new_refresh_token: String? = null,
    val new_refresh_token_expire_time: String? = null
)

/** GET /api/v1/auth/llm-config 响应 */
@Serializable
data class LlmConfigResponse(
    val api_token: String,
    val llm_base_url: String,
    val expires_at: String? = null
)
