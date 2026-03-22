package ai.zeroclaw.android.ui

import android.util.Log
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import ai.zeroclaw.android.data.SessionManager
import ai.zeroclaw.android.network.HuanxingApi
import ai.zeroclaw.android.onboard.OnboardManager
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

private const val TAG = "LoginScreen"

/** 品牌色 */
private val BrandPurple = Color(0xFF6C5CE7)

/** 登录流程步骤 */
private enum class LoginStep {
    PHONE, CODE, ONBOARD
}

/** Onboard 进度项 */
private data class OnboardStep(
    val id: String,
    val label: String,
    val status: String // pending / running / done / error
)

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun LoginScreen(
    sessionManager: SessionManager,
    onboardManager: OnboardManager,
    onLoginComplete: () -> Unit
) {
    var step by remember { mutableStateOf(LoginStep.PHONE) }
    var phone by remember { mutableStateOf("") }
    var code by remember { mutableStateOf("") }
    var error by remember { mutableStateOf<String?>(null) }
    var loading by remember { mutableStateOf(false) }

    // 验证码倒计时
    var countdown by remember { mutableIntStateOf(0) }
    val scope = rememberCoroutineScope()

    // Onboard 进度
    var onboardSteps by remember {
        mutableStateOf(listOf(
            OnboardStep("login", "登录验证", "done"),
            OnboardStep("config", "创建 AI 引擎配置", "pending"),
            OnboardStep("agent", "初始化默认助手", "pending"),
            OnboardStep("engine", "启动 AI 引擎", "pending"),
            OnboardStep("ready", "一切就绪", "pending")
        ))
    }

    // 倒计时效果
    LaunchedEffect(countdown) {
        if (countdown > 0) {
            delay(1000)
            countdown--
        }
    }

    Scaffold { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(horizontal = 32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            // Logo
            Text(
                text = "✨",
                style = MaterialTheme.typography.displayLarge
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = "唤星",
                style = MaterialTheme.typography.headlineMedium,
                color = BrandPurple
            )
            Text(
                text = "唤醒你的星，AI 与你共生",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(48.dp))

            when (step) {
                LoginStep.PHONE -> PhoneStep(
                    phone = phone,
                    onPhoneChange = { phone = it; error = null },
                    countdown = countdown,
                    loading = loading,
                    error = error,
                    onSendCode = {
                        if (phone.length != 11) {
                            error = "请输入 11 位手机号"
                            return@PhoneStep
                        }
                        loading = true
                        error = null
                        scope.launch {
                            val result = HuanxingApi.sendVerifyCode(phone)
                            loading = false
                            result.fold(
                                onSuccess = {
                                    countdown = 60
                                    step = LoginStep.CODE
                                },
                                onFailure = { e ->
                                    error = e.message ?: "发送失败"
                                }
                            )
                        }
                    }
                )

                LoginStep.CODE -> CodeStep(
                    phone = phone,
                    code = code,
                    onCodeChange = { code = it; error = null },
                    loading = loading,
                    error = error,
                    onBack = { step = LoginStep.PHONE; code = ""; error = null },
                    onLogin = {
                        if (code.length != 6) {
                            error = "请输入 6 位验证码"
                            return@CodeStep
                        }
                        loading = true
                        error = null
                        scope.launch {
                            val result = HuanxingApi.phoneLogin(phone, code)
                            result.fold(
                                onSuccess = { loginResp ->
                                    // 保存会话
                                    sessionManager.saveSession(loginResp)
                                    loading = false
                                    // 进入 onboard
                                    step = LoginStep.ONBOARD
                                    // 执行 onboard
                                    runOnboard(
                                        sessionManager = sessionManager,
                                        onboardManager = onboardManager,
                                        onStepsUpdate = { onboardSteps = it },
                                        onComplete = onLoginComplete,
                                        onError = { msg -> error = msg }
                                    )
                                },
                                onFailure = { e ->
                                    loading = false
                                    error = e.message ?: "登录失败"
                                }
                            )
                        }
                    }
                )

                LoginStep.ONBOARD -> OnboardStep(
                    steps = onboardSteps,
                    error = error
                )
            }
        }
    }
}

@Composable
private fun PhoneStep(
    phone: String,
    onPhoneChange: (String) -> Unit,
    countdown: Int,
    loading: Boolean,
    error: String?,
    onSendCode: () -> Unit
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        OutlinedTextField(
            value = phone,
            onValueChange = { if (it.length <= 11) onPhoneChange(it) },
            label = { Text("手机号") },
            placeholder = { Text("请输入手机号") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Phone),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            isError = error != null
        )

        error?.let {
            Text(
                text = it,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp)
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = onSendCode,
            enabled = phone.length == 11 && !loading && countdown == 0,
            modifier = Modifier.fillMaxWidth().height(48.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BrandPurple)
        ) {
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = Color.White,
                    strokeWidth = 2.dp
                )
            } else {
                Text(
                    if (countdown > 0) "${countdown}s 后重新发送" else "获取验证码"
                )
            }
        }
    }
}

@Composable
private fun CodeStep(
    phone: String,
    code: String,
    onCodeChange: (String) -> Unit,
    loading: Boolean,
    error: String?,
    onBack: () -> Unit,
    onLogin: () -> Unit
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "验证码已发送至 $phone",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )

        Spacer(modifier = Modifier.height(16.dp))

        OutlinedTextField(
            value = code,
            onValueChange = { if (it.length <= 6) onCodeChange(it) },
            label = { Text("验证码") },
            placeholder = { Text("6 位验证码") },
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
            isError = error != null
        )

        error?.let {
            Text(
                text = it,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(top = 4.dp)
            )
        }

        Spacer(modifier = Modifier.height(24.dp))

        Button(
            onClick = onLogin,
            enabled = code.length == 6 && !loading,
            modifier = Modifier.fillMaxWidth().height(48.dp),
            colors = ButtonDefaults.buttonColors(containerColor = BrandPurple)
        ) {
            if (loading) {
                CircularProgressIndicator(
                    modifier = Modifier.size(20.dp),
                    color = Color.White,
                    strokeWidth = 2.dp
                )
            } else {
                Text("登录")
            }
        }

        Spacer(modifier = Modifier.height(12.dp))

        TextButton(onClick = onBack) {
            Text("返回修改手机号")
        }
    }
}

@Composable
private fun OnboardStep(
    steps: List<OnboardStep>,
    error: String?
) {
    Column(
        modifier = Modifier.fillMaxWidth(),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Text(
            text = "正在初始化...",
            style = MaterialTheme.typography.titleMedium
        )

        Spacer(modifier = Modifier.height(24.dp))

        steps.forEach { step ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically
            ) {
                val icon = when (step.status) {
                    "done" -> "✓"
                    "running" -> "⟳"
                    "error" -> "✗"
                    else -> "○"
                }
                val color = when (step.status) {
                    "done" -> BrandPurple
                    "running" -> MaterialTheme.colorScheme.tertiary
                    "error" -> MaterialTheme.colorScheme.error
                    else -> MaterialTheme.colorScheme.outline
                }

                Text(
                    text = icon,
                    color = color,
                    modifier = Modifier.width(24.dp),
                    textAlign = TextAlign.Center
                )
                Spacer(modifier = Modifier.width(12.dp))
                Text(
                    text = step.label,
                    color = if (step.status == "pending")
                        MaterialTheme.colorScheme.outline
                    else
                        MaterialTheme.colorScheme.onSurface
                )

                if (step.status == "running") {
                    Spacer(modifier = Modifier.width(8.dp))
                    CircularProgressIndicator(
                        modifier = Modifier.size(14.dp),
                        strokeWidth = 2.dp
                    )
                }
            }
        }

        error?.let {
            Spacer(modifier = Modifier.height(16.dp))
            Text(
                text = it,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall,
                textAlign = TextAlign.Center
            )
        }
    }
}

/** 执行 onboard 流程，更新进度 */
private suspend fun runOnboard(
    sessionManager: SessionManager,
    onboardManager: OnboardManager,
    onStepsUpdate: (List<OnboardStep>) -> Unit,
    onComplete: () -> Unit,
    onError: (String) -> Unit
) {
    val session = sessionManager.getSession()
    if (session == null) {
        onError("会话丢失，请重新登录")
        return
    }

    fun updateStep(id: String, status: String) {
        onStepsUpdate(listOf(
            OnboardStep("login", "登录验证", "done"),
            OnboardStep("config", "创建 AI 引擎配置",
                if (id == "config") status else if (listOf("agent", "engine", "ready").contains(id)) "done" else "pending"),
            OnboardStep("agent", "初始化默认助手",
                if (id == "agent") status else if (listOf("engine", "ready").contains(id)) "done" else "pending"),
            OnboardStep("engine", "启动 AI 引擎",
                if (id == "engine") status else if (id == "ready") "done" else "pending"),
            OnboardStep("ready", "一切就绪",
                if (id == "ready") status else "pending")
        ))
    }

    try {
        // 配置创建
        updateStep("config", "running")
        delay(300) // 视觉反馈

        // agent 初始化
        updateStep("agent", "running")
        delay(300)

        // 启动引擎（onboard 包含 config + agent + start）
        updateStep("engine", "running")
        onboardManager.onboard(session).getOrThrow()

        // 就绪
        updateStep("ready", "done")
        delay(500)

        onComplete()
    } catch (e: Exception) {
        Log.e(TAG, "Onboard 失败", e)
        onError("初始化失败: ${e.message}")
    }
}
