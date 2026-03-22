package ai.zeroclaw.android.network

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.json.Json
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.concurrent.TimeUnit

/**
 * 唤星后端 API 客户端。
 *
 * 所有认证相关的 HTTP 调用集中在此。
 */
object HuanxingApi {

    private const val TAG = "HuanxingApi"

    /** 唤星后端 API 基地址 */
    const val API_BASE_URL = "https://api.huanxing.dcfuture.cn"

    /** 默认 LLM 模型 */
    const val DEFAULT_MODEL = "claude-sonnet-4-6"

    /** 标题模型 */
    const val TITLE_MODEL = "claude-haiku-4-5"

    /** 默认 Agent 名称 */
    const val DEFAULT_AGENT_NAME = "小星"

    /** 默认温度 */
    const val DEFAULT_TEMPERATURE = 0.7

    private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()

    private val json = Json {
        ignoreUnknownKeys = true
        coerceInputValues = true
    }

    private val client = OkHttpClient.Builder()
        .connectTimeout(15, TimeUnit.SECONDS)
        .readTimeout(15, TimeUnit.SECONDS)
        .writeTimeout(15, TimeUnit.SECONDS)
        .build()

    /** 发送验证码 */
    suspend fun sendVerifyCode(phone: String): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val body = json.encodeToString(SendCodeRequest.serializer(), SendCodeRequest(phone))
            val request = Request.Builder()
                .url("$API_BASE_URL/api/v1/auth/send-code")
                .post(body.toRequestBody(JSON_MEDIA_TYPE))
                .header("X-App-Code", "huanxing")
                .build()

            val response = client.newCall(request).execute()
            val responseBody = response.body?.string() ?: throw RuntimeException("空响应")

            val apiResp = json.decodeFromString(
                ApiResponse.serializer(SendCodeResponse.serializer()),
                responseBody
            )

            if (apiResp.code != 200) {
                throw RuntimeException(apiResp.msg)
            }

            Log.i(TAG, "验证码已发送: $phone")
            Unit
        }
    }

    /** 手机号+验证码登录 */
    suspend fun phoneLogin(phone: String, code: String): Result<LoginResponse> = withContext(Dispatchers.IO) {
        runCatching {
            val body = json.encodeToString(
                PhoneLoginRequest.serializer(),
                PhoneLoginRequest(phone, code)
            )
            val request = Request.Builder()
                .url("$API_BASE_URL/api/v1/auth/phone-login")
                .post(body.toRequestBody(JSON_MEDIA_TYPE))
                .header("X-App-Code", "huanxing")
                .build()

            val response = client.newCall(request).execute()
            val responseBody = response.body?.string() ?: throw RuntimeException("空响应")

            val apiResp = json.decodeFromString(
                ApiResponse.serializer(LoginResponse.serializer()),
                responseBody
            )

            if (apiResp.code != 200 || apiResp.data == null) {
                throw RuntimeException(apiResp.msg)
            }

            Log.i(TAG, "登录成功: ${apiResp.data.user.nickname}")
            apiResp.data
        }
    }

    /** 刷新 access_token */
    suspend fun refreshToken(refreshToken: String): Result<RefreshResponse> = withContext(Dispatchers.IO) {
        runCatching {
            val body = json.encodeToString(
                RefreshRequest.serializer(),
                RefreshRequest(refreshToken)
            )
            val request = Request.Builder()
                .url("$API_BASE_URL/api/v1/auth/refresh")
                .post(body.toRequestBody(JSON_MEDIA_TYPE))
                .header("X-App-Code", "huanxing")
                .build()

            val response = client.newCall(request).execute()
            val responseBody = response.body?.string() ?: throw RuntimeException("空响应")

            val apiResp = json.decodeFromString(
                ApiResponse.serializer(RefreshResponse.serializer()),
                responseBody
            )

            if (apiResp.code != 200 || apiResp.data == null) {
                throw RuntimeException(apiResp.msg)
            }

            apiResp.data
        }
    }

    /** 获取最新 LLM 配置（需要 JWT） */
    suspend fun getLlmConfig(accessToken: String): Result<LlmConfigResponse> = withContext(Dispatchers.IO) {
        runCatching {
            val request = Request.Builder()
                .url("$API_BASE_URL/api/v1/auth/llm-config")
                .get()
                .header("Authorization", "Bearer $accessToken")
                .header("X-App-Code", "huanxing")
                .build()

            val response = client.newCall(request).execute()
            val responseBody = response.body?.string() ?: throw RuntimeException("空响应")

            val apiResp = json.decodeFromString(
                ApiResponse.serializer(LlmConfigResponse.serializer()),
                responseBody
            )

            if (apiResp.code != 200 || apiResp.data == null) {
                throw RuntimeException(apiResp.msg)
            }

            apiResp.data
        }
    }
}
