package ai.zeroclaw.android

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.ServiceConnection
import android.os.Bundle
import android.os.IBinder
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import ai.zeroclaw.android.data.SessionManager
import ai.zeroclaw.android.onboard.OnboardManager
import ai.zeroclaw.android.service.ZeroClawService
import ai.zeroclaw.android.ui.LoginScreen
import ai.zeroclaw.android.ui.theme.ZeroClawTheme
import ai.zeroclaw.android.ws.WsInboundMessage
import kotlinx.coroutines.launch

/** 导航目标 */
private enum class Screen {
    LOGIN, CHAT
}

class MainActivity : ComponentActivity() {

    private var service: ZeroClawService? = null
    private var bound = false
    private lateinit var sessionManager: SessionManager
    private lateinit var onboardManager: OnboardManager

    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            service = (binder as ZeroClawService.LocalBinder).getService()
            bound = true
        }

        override fun onServiceDisconnected(name: ComponentName) {
            service = null
            bound = false
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        sessionManager = SessionManager(this)
        onboardManager = OnboardManager(this)

        // 绑定 Service
        bindService(
            Intent(this, ZeroClawService::class.java),
            connection,
            Context.BIND_AUTO_CREATE
        )

        setContent {
            ZeroClawTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    // 根据登录状态决定初始页面
                    var currentScreen by remember {
                        mutableStateOf(
                            if (sessionManager.isLoggedIn()) Screen.CHAT else Screen.LOGIN
                        )
                    }

                    when (currentScreen) {
                        Screen.LOGIN -> LoginScreen(
                            sessionManager = sessionManager,
                            onboardManager = onboardManager,
                            onLoginComplete = {
                                currentScreen = Screen.CHAT
                            }
                        )

                        Screen.CHAT -> ZeroClawChatApp(
                            serviceProvider = { service },
                            onStartAgent = {
                                val intent = Intent(this@MainActivity, ZeroClawService::class.java)
                                    .setAction(ZeroClawService.ACTION_START)
                                startForegroundService(intent)
                            },
                            onStopAgent = {
                                val intent = Intent(this@MainActivity, ZeroClawService::class.java)
                                    .setAction(ZeroClawService.ACTION_STOP)
                                startService(intent)
                            },
                            onLogout = {
                                // 停止 Agent → 清除会话 → 跳转登录
                                val intent = Intent(this@MainActivity, ZeroClawService::class.java)
                                    .setAction(ZeroClawService.ACTION_STOP)
                                startService(intent)
                                sessionManager.clearSession()
                                currentScreen = Screen.LOGIN
                            }
                        )
                    }
                }
            }
        }
    }

    override fun onDestroy() {
        if (bound) {
            unbindService(connection)
            bound = false
        }
        super.onDestroy()
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ZeroClawChatApp(
    serviceProvider: () -> ZeroClawService?,
    onStartAgent: () -> Unit,
    onStopAgent: () -> Unit,
    onLogout: () -> Unit
) {
    val svc = serviceProvider()

    // 从 Service 收集状态
    val serviceStatus by svc?.status?.collectAsState()
        ?: remember { mutableStateOf(ZeroClawService.Status.Stopped) }

    val agentStatus = when (serviceStatus) {
        is ZeroClawService.Status.Running -> AgentStatus.Running
        is ZeroClawService.Status.Starting -> AgentStatus.Starting
        is ZeroClawService.Status.Error -> AgentStatus.Error
        else -> AgentStatus.Stopped
    }

    val errorMessage = (serviceStatus as? ZeroClawService.Status.Error)?.message

    // 消息列表
    var messages by remember { mutableStateOf(listOf<ChatMessage>()) }
    var inputText by remember { mutableStateOf("") }
    // 流式响应缓冲
    var streamingContent by remember { mutableStateOf("") }
    var isStreaming by remember { mutableStateOf(false) }

    // 订阅 WS 消息
    val coroutineScope = rememberCoroutineScope()
    LaunchedEffect(svc) {
        svc?.chatMessages?.collect { msg ->
            handleWsMessage(
                msg = msg,
                currentMessages = messages,
                streamingContent = streamingContent,
                onMessagesUpdate = { messages = it },
                onStreamingUpdate = { content, streaming ->
                    streamingContent = content
                    isStreaming = streaming
                }
            )
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("ZeroClaw") },
                actions = {
                    StatusIndicator(status = agentStatus)
                    Spacer(modifier = Modifier.width(8.dp))
                    TextButton(onClick = onLogout) {
                        Text("退出", style = MaterialTheme.typography.labelMedium)
                    }
                }
            )
        },
        bottomBar = {
            ChatInput(
                text = inputText,
                onTextChange = { inputText = it },
                onSend = {
                    if (inputText.isNotBlank() && agentStatus == AgentStatus.Running) {
                        messages = messages + ChatMessage(
                            content = inputText,
                            isUser = true
                        )
                        svc?.sendMessage(inputText)
                        inputText = ""
                        // 重置流式状态
                        streamingContent = ""
                        isStreaming = false
                    }
                },
                enabled = agentStatus == AgentStatus.Running
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
        ) {
            // 错误提示
            errorMessage?.let { err ->
                Surface(
                    color = MaterialTheme.colorScheme.errorContainer,
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = err,
                        modifier = Modifier.padding(12.dp),
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        style = MaterialTheme.typography.bodySmall
                    )
                }
            }

            if (messages.isEmpty() && !isStreaming) {
                EmptyState(
                    status = agentStatus,
                    onStart = onStartAgent
                )
            } else {
                ChatMessageList(
                    messages = messages,
                    streamingContent = if (isStreaming) streamingContent else null,
                    modifier = Modifier.weight(1f)
                )
            }
        }
    }
}

/** 处理 WS 入站消息，更新 UI 状态 */
private fun handleWsMessage(
    msg: WsInboundMessage,
    currentMessages: List<ChatMessage>,
    streamingContent: String,
    onMessagesUpdate: (List<ChatMessage>) -> Unit,
    onStreamingUpdate: (String, Boolean) -> Unit
) {
    when (msg.type) {
        "chunk" -> {
            val newContent = streamingContent + (msg.content ?: "")
            onStreamingUpdate(newContent, true)
        }
        "done" -> {
            val fullResponse = msg.fullResponse ?: streamingContent
            if (fullResponse.isNotBlank()) {
                onMessagesUpdate(currentMessages + ChatMessage(
                    content = fullResponse,
                    isUser = false
                ))
            }
            onStreamingUpdate("", false)
        }
        "tool_call" -> {
            val toolInfo = "🔧 ${msg.displayName ?: msg.name ?: "工具调用"}"
            onStreamingUpdate(toolInfo, true)
        }
        "tool_result" -> {
            // tool_result 后等待后续 chunk/done
        }
        "error" -> {
            val errorMsg = msg.message ?: "未知错误"
            onMessagesUpdate(currentMessages + ChatMessage(
                content = "⚠️ $errorMsg",
                isUser = false
            ))
            onStreamingUpdate("", false)
        }
    }
}

@Composable
fun StatusIndicator(status: AgentStatus) {
    val (color, text) = when (status) {
        AgentStatus.Running -> MaterialTheme.colorScheme.primary to "Running"
        AgentStatus.Starting -> MaterialTheme.colorScheme.tertiary to "Starting"
        AgentStatus.Stopped -> MaterialTheme.colorScheme.outline to "Stopped"
        AgentStatus.Error -> MaterialTheme.colorScheme.error to "Error"
    }

    Surface(
        color = color.copy(alpha = 0.2f),
        shape = MaterialTheme.shapes.small
    ) {
        Text(
            text = text,
            modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
            color = color,
            style = MaterialTheme.typography.labelMedium
        )
    }
}

@Composable
fun EmptyState(status: AgentStatus, onStart: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(32.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text(
            text = "🦀",
            style = MaterialTheme.typography.displayLarge
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text(
            text = "ZeroClaw",
            style = MaterialTheme.typography.headlineMedium
        )
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            text = "Your AI assistant, running locally",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center
        )
        Spacer(modifier = Modifier.height(32.dp))

        when (status) {
            AgentStatus.Stopped -> {
                Button(onClick = onStart) {
                    Text("Start Agent")
                }
            }
            AgentStatus.Starting -> {
                CircularProgressIndicator()
                Spacer(modifier = Modifier.height(8.dp))
                Text(
                    text = "正在启动...",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
            else -> {}
        }
    }
}

@Composable
fun ChatInput(
    text: String,
    onTextChange: (String) -> Unit,
    onSend: () -> Unit,
    enabled: Boolean = true
) {
    Surface(
        tonalElevation = 3.dp
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(8.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            OutlinedTextField(
                value = text,
                onValueChange = onTextChange,
                modifier = Modifier.weight(1f),
                placeholder = { Text("Message ZeroClaw...") },
                singleLine = true,
                enabled = enabled
            )
            Spacer(modifier = Modifier.width(8.dp))
            IconButton(onClick = onSend, enabled = enabled && text.isNotBlank()) {
                Text("→")
            }
        }
    }
}

@Composable
fun ChatMessageList(
    messages: List<ChatMessage>,
    streamingContent: String? = null,
    modifier: Modifier = Modifier
) {
    val listState = rememberLazyListState()
    val coroutineScope = rememberCoroutineScope()

    // 自动滚动到底部
    LaunchedEffect(messages.size, streamingContent) {
        val totalItems = messages.size + if (streamingContent != null) 1 else 0
        if (totalItems > 0) {
            coroutineScope.launch {
                listState.animateScrollToItem(totalItems - 1)
            }
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier.padding(horizontal = 16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        items(messages) { message ->
            ChatBubble(message = message)
        }
        // 流式响应气泡
        if (streamingContent != null && streamingContent.isNotBlank()) {
            item {
                ChatBubble(
                    message = ChatMessage(content = streamingContent, isUser = false)
                )
            }
        }
    }
}

@Composable
fun ChatBubble(message: ChatMessage) {
    val color = if (message.isUser)
        MaterialTheme.colorScheme.primaryContainer
    else
        MaterialTheme.colorScheme.surfaceVariant

    Box(
        modifier = Modifier.fillMaxWidth(),
        contentAlignment = if (message.isUser) Alignment.CenterEnd else Alignment.CenterStart
    ) {
        Surface(
            color = color,
            shape = MaterialTheme.shapes.medium
        ) {
            Text(
                text = message.content,
                modifier = Modifier.padding(12.dp)
            )
        }
    }
}

data class ChatMessage(
    val content: String,
    val isUser: Boolean,
    val timestamp: Long = System.currentTimeMillis()
)

enum class AgentStatus {
    Running, Starting, Stopped, Error
}
