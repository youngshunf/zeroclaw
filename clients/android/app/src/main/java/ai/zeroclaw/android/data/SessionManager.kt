package ai.zeroclaw.android.data

import android.content.Context
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import ai.zeroclaw.android.network.LoginResponse
import ai.zeroclaw.android.network.UserInfo
import java.time.Instant
import java.time.format.DateTimeFormatter

/**
 * 唤星会话管理器。
 *
 * 使用 EncryptedSharedPreferences 安全存储登录会话。
 * 包含 access_token、refresh_token、llm_token、用户信息等。
 */
class SessionManager(context: Context) {

    companion object {
        private const val TAG = "SessionManager"
        private const val PREFS_NAME = "huanxing_session"

        private const val KEY_ACCESS_TOKEN = "access_token"
        private const val KEY_ACCESS_TOKEN_EXPIRE = "access_token_expire"
        private const val KEY_REFRESH_TOKEN = "refresh_token"
        private const val KEY_REFRESH_TOKEN_EXPIRE = "refresh_token_expire"
        private const val KEY_LLM_TOKEN = "llm_token"
        private const val KEY_LLM_BASE_URL = "llm_base_url"
        private const val KEY_GATEWAY_TOKEN = "gateway_token"
        private const val KEY_USER_UUID = "user_uuid"
        private const val KEY_USER_NICKNAME = "user_nickname"
        private const val KEY_USER_PHONE = "user_phone"
        private const val KEY_USER_AVATAR = "user_avatar"
        private const val KEY_IS_NEW_USER = "is_new_user"
        private const val KEY_LOGIN_AT = "login_at"

        /** Token 过期前提前刷新的时间（毫秒） */
        private const val REFRESH_AHEAD_MS = 2 * 60 * 1000L // 2 分钟
    }

    private val prefs by lazy {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()

        EncryptedSharedPreferences.create(
            context,
            PREFS_NAME,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

    /** 保存登录会话 */
    fun saveSession(response: LoginResponse) {
        prefs.edit().apply {
            putString(KEY_ACCESS_TOKEN, response.access_token)
            putString(KEY_ACCESS_TOKEN_EXPIRE, response.access_token_expire_time)
            putString(KEY_REFRESH_TOKEN, response.refresh_token)
            putString(KEY_REFRESH_TOKEN_EXPIRE, response.refresh_token_expire_time)
            putString(KEY_LLM_TOKEN, response.llm_token)
            putString(KEY_LLM_BASE_URL, response.llm_base_url ?: "")
            putString(KEY_GATEWAY_TOKEN, response.gateway_token ?: "")
            putString(KEY_USER_UUID, response.user.uuid)
            putString(KEY_USER_NICKNAME, response.user.nickname)
            putString(KEY_USER_PHONE, response.user.phone ?: "")
            putString(KEY_USER_AVATAR, response.user.avatar ?: "")
            putBoolean(KEY_IS_NEW_USER, response.is_new_user)
            putString(KEY_LOGIN_AT, Instant.now().toString())
            apply()
        }
        Log.i(TAG, "会话已保存: ${response.user.nickname}")
    }

    /** 获取当前会话 */
    fun getSession(): HuanxingSession? {
        val accessToken = prefs.getString(KEY_ACCESS_TOKEN, null) ?: return null
        return HuanxingSession(
            accessToken = accessToken,
            accessTokenExpireTime = prefs.getString(KEY_ACCESS_TOKEN_EXPIRE, "") ?: "",
            refreshToken = prefs.getString(KEY_REFRESH_TOKEN, "") ?: "",
            refreshTokenExpireTime = prefs.getString(KEY_REFRESH_TOKEN_EXPIRE, "") ?: "",
            llmToken = prefs.getString(KEY_LLM_TOKEN, "") ?: "",
            llmBaseUrl = prefs.getString(KEY_LLM_BASE_URL, "") ?: "",
            gatewayToken = prefs.getString(KEY_GATEWAY_TOKEN, "") ?: "",
            userUuid = prefs.getString(KEY_USER_UUID, "") ?: "",
            userNickname = prefs.getString(KEY_USER_NICKNAME, "") ?: "",
            userPhone = prefs.getString(KEY_USER_PHONE, "") ?: "",
            userAvatar = prefs.getString(KEY_USER_AVATAR, "") ?: ""
        )
    }

    /** 是否已登录（有 token 且 refresh_token 未过期） */
    fun isLoggedIn(): Boolean {
        val session = getSession() ?: return false
        if (session.refreshToken.isBlank()) return false
        // refresh_token 过期则需要重新登录
        return !isExpired(session.refreshTokenExpireTime)
    }

    /** access_token 是否需要刷新（过期前 2 分钟） */
    fun needsRefresh(): Boolean {
        val session = getSession() ?: return false
        return isExpiringSoon(session.accessTokenExpireTime, REFRESH_AHEAD_MS)
    }

    /** 更新 access_token（刷新后调用） */
    fun updateAccessToken(
        newToken: String,
        expireTime: String,
        newRefreshToken: String? = null,
        newRefreshExpireTime: String? = null
    ) {
        prefs.edit().apply {
            putString(KEY_ACCESS_TOKEN, newToken)
            putString(KEY_ACCESS_TOKEN_EXPIRE, expireTime)
            newRefreshToken?.let { putString(KEY_REFRESH_TOKEN, it) }
            newRefreshExpireTime?.let { putString(KEY_REFRESH_TOKEN_EXPIRE, it) }
            apply()
        }
    }

    /** 更新 LLM 配置（llm-config 接口返回后调用） */
    fun updateLlmConfig(llmToken: String, llmBaseUrl: String) {
        prefs.edit().apply {
            putString(KEY_LLM_TOKEN, llmToken)
            putString(KEY_LLM_BASE_URL, llmBaseUrl)
            apply()
        }
        Log.i(TAG, "LLM 配置已更新: $llmBaseUrl")
    }

    /** 清除会话（登出） */
    fun clearSession() {
        prefs.edit().clear().apply()
        Log.i(TAG, "会话已清除")
    }

    /** 判断时间字符串是否已过期 */
    private fun isExpired(timeStr: String): Boolean {
        if (timeStr.isBlank()) return true
        return try {
            val expireInstant = parseDateTime(timeStr)
            Instant.now().isAfter(expireInstant)
        } catch (_: Exception) {
            true
        }
    }

    /** 判断时间字符串是否即将过期 */
    private fun isExpiringSoon(timeStr: String, aheadMs: Long): Boolean {
        if (timeStr.isBlank()) return true
        return try {
            val expireInstant = parseDateTime(timeStr)
            val threshold = Instant.now().plusMillis(aheadMs)
            threshold.isAfter(expireInstant)
        } catch (_: Exception) {
            true
        }
    }

    /** 解析后端返回的时间字符串（ISO 格式或 yyyy-MM-dd HH:mm:ss） */
    private fun parseDateTime(timeStr: String): Instant {
        return try {
            Instant.parse(timeStr)
        } catch (_: Exception) {
            // 后端可能返回 "2026-03-23 10:30:00" 格式
            val normalized = timeStr.replace(" ", "T") + "Z"
            Instant.parse(normalized)
        }
    }
}

/** 本地存储的唤星会话 */
data class HuanxingSession(
    val accessToken: String,
    val accessTokenExpireTime: String,
    val refreshToken: String,
    val refreshTokenExpireTime: String,
    val llmToken: String,
    val llmBaseUrl: String,
    val gatewayToken: String,
    val userUuid: String,
    val userNickname: String,
    val userPhone: String,
    val userAvatar: String
)
