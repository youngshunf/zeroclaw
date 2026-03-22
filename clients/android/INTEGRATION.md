# Android 端 ZeroClaw 集成方案

**架构决策 · 实现路径 · 工作量估算**

---

## 一、方案选择

### 当前状态

`clients/android/` 已有完整的 App 骨架（Kotlin + Jetpack Compose），包括：

- Chat UI、Settings UI
- `ZeroClawService`（前台服务，开机自启，心跳保活）
- `ZeroClawBridge`（JNI 占位，所有方法是 TODO）
- WorkManager、Widget、QuickSettings Tile

骨架的 **通信层是空的**——`ZeroClawBridge.kt` 里的所有 native 方法都是注释掉的 TODO。

### 三种集成路径对比

| 方案 | 原理 | 工作量 | 风险 |
|------|------|--------|------|
| **A. 子进程方案（推荐）** | 将 zeroclaw 编译为 Android binary，App 管理其生命周期，WS 通信 | **3-5 天** | 低 |
| B. JNI/UniFFI 方案 | 将 zeroclaw 编译为 `.so`，JNI 嵌入 App 进程 | 2-3 个月 | 高 |
| C. 连接云端/桌面端 | App 通过 HASN 或 Bridge Channel 连远端 zeroclaw | 1-2 周 | 低，但需要联网 |

### 选择方案 A 的理由

与桌面端（Tauri sidecar）**完全同架构**：

```
桌面端：Tauri App → spawn zeroclaw binary → WS localhost:42620
Android：Android Service → spawn zeroclaw binary → WS localhost:42620
```

- 不需要 JNI/NDK 编译链
- zeroclaw 已支持 `aarch64-linux-android` 交叉编译（上游有 CI）
- 核心依赖（tokio/axum/rustls/reqwest）全部支持 Android，无 OpenSSL 依赖
- Android 允许 App 在自己的 `filesDir` 执行 binary，无需 root

---

## 二、整体架构

```
┌─────────────────────────────────────────────────────┐
│  Android App                                        │
│                                                     │
│  ┌──────────────────────────────────────────────┐   │
│  │  UI Layer (Jetpack Compose)                  │   │
│  │  ChatScreen · SettingsScreen · Widget        │   │
│  └─────────────────┬────────────────────────────┘   │
│                    │ StateFlow                       │
│  ┌─────────────────▼────────────────────────────┐   │
│  │  ZeroClawService (Foreground Service)        │   │
│  │  ├─ ZeroClawProcessManager  ← 新增核心类     │   │
│  │  │   ├─ 解压 binary                          │   │
│  │  │   ├─ 启动子进程 (ProcessBuilder)          │   │
│  │  │   ├─ 健康检查 (GET /health)               │   │
│  │  │   └─ 自动重启 (HeartbeatWorker 配合)      │   │
│  │  └─ ZeroClawWsClient        ← 新增通信层     │   │
│  │      ├─ WS 连接 ws://127.0.0.1:42620         │   │
│  │      ├─ 发送消息 / 接收消息                  │   │
│  │      └─ 心跳 Ping/Pong                       │   │
│  └──────────────────────────────────────────────┘   │
│                                                     │
│  app/src/main/assets/                               │
│  └─ zeroclaw-arm64   ← 编译产物（~6MB）             │
│                                                     │
└─────────────────────────────────────────────────────┘
           │ localhost:42620
           ▼
┌─────────────────────────────────────────────────────┐
│  zeroclaw daemon                                    │
│  ├─ HTTP /health · /api/status                     │
│  ├─ WS /api/v1/{agent}/ws?token=xxx                │
│  └─ 完整 Agent 能力（LLM · Memory · Tools · Skills）│
└─────────────────────────────────────────────────────┘
```

---

## 三、实现步骤

### Step 1：交叉编译 zeroclaw Android binary

在项目根目录（`huanxing-zeroclaw/`）执行：

```bash
# 安装 Android 编译目标
rustup target add aarch64-linux-android armv7-linux-androideabi

# 安装 cargo-ndk（管理 NDK 工具链）
cargo install cargo-ndk

# 设置 NDK 路径（Android Studio 自带 NDK）
export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/<version>

# 编译 Android 版本（关闭桌面端专属 features）
cargo ndk -t arm64-v8a build --release --bin zeroclaw \
    --no-default-features \
    --features "skill-creation"
    # 去掉：observability-prometheus channel-nostr（体积优化）
    # 去掉：sandbox-landlock（Android 不支持）
    # 去掉：browser-native（移动端无浏览器自动化需求）

# 产物路径
target/aarch64-linux-android/release/zeroclaw
# 目标大小：~5-8MB（rustls 替换 OpenSSL 后）

# 复制到 assets
cp target/aarch64-linux-android/release/zeroclaw \
   clients/android/app/src/main/assets/zeroclaw-arm64
```

> **Cargo.toml 中已确认 Android 兼容的依赖：**
> - `tokio`：纯 Rust，Android 支持完整
> - `axum`：基于 tokio，无平台限制
> - `reqwest`：已配置 `rustls-tls`，不依赖 OpenSSL
> - `ring`：加密库，支持 aarch64-linux-android
> - `libc`：Android 为 Linux 族，`cfg(unix)` 完全满足

---

### Step 2：新增 ZeroClawProcessManager

替换 `ZeroClawBridge.kt` 的 TODO，实现真正的进程管理。
逻辑参考桌面端 `clients/desktop/src-tauri/src/sidecar.rs`。

```kotlin
// app/src/main/java/ai/zeroclaw/android/process/ZeroClawProcessManager.kt

class ZeroClawProcessManager(private val context: Context) {

    private var process: Process? = null
    private val port = 42620
    private val binaryName = "zeroclaw-arm64"  // 或根据 ABI 选择

    /** 从 assets 解压 binary 到 filesDir（首次运行或版本升级时） */
    fun extractBinary(): File {
        val binFile = File(context.filesDir, "bin/$binaryName")
        val versionFile = File(context.filesDir, "bin/version")

        val currentVersion = BuildConfig.VERSION_CODE.toString()
        if (binFile.exists() && versionFile.exists() &&
            versionFile.readText() == currentVersion) {
            return binFile  // 已是最新版本，跳过解压
        }

        binFile.parentFile?.mkdirs()
        context.assets.open(binaryName).use { input ->
            binFile.outputStream().use { output -> input.copyTo(output) }
        }
        binFile.setExecutable(true)
        versionFile.writeText(currentVersion)

        return binFile
    }

    /** 确保配置目录存在 */
    fun ensureConfigDir(): File {
        val configDir = File(context.filesDir, "zeroclaw")
        configDir.mkdirs()
        return configDir
    }

    /** 启动 zeroclaw daemon */
    suspend fun start(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            val bin = extractBinary()
            val configDir = ensureConfigDir()

            // 等价于桌面端：zeroclaw daemon --port 42620 --config-dir ~/.huanxing
            process = ProcessBuilder(
                bin.absolutePath,
                "gateway",          // 启动 gateway 模式（含 WS）
                "-p", port.toString(),
                "--home", configDir.absolutePath
            )
                .directory(context.filesDir)
                .redirectErrorStream(false)
                .start()

            // 等待健康检查（最多 15 秒，每 500ms 轮询）
            waitUntilHealthy()
        }
    }

    /** 健康检查：轮询 GET /health 直到成功 */
    private suspend fun waitUntilHealthy(
        timeoutMs: Long = 15_000,
        intervalMs: Long = 500
    ) {
        val deadline = System.currentTimeMillis() + timeoutMs
        while (System.currentTimeMillis() < deadline) {
            delay(intervalMs)
            try {
                val conn = URL("http://127.0.0.1:$port/health")
                    .openConnection() as HttpURLConnection
                conn.connectTimeout = 2000
                conn.readTimeout = 2000
                if (conn.responseCode == 200) return
            } catch (_: Exception) { }
        }
        throw RuntimeException("zeroclaw 启动超时（15s），健康检查未通过")
    }

    /** 停止 daemon */
    fun stop() {
        process?.destroy()
        process = null
    }

    /** 检查是否在运行（通过健康检查接口） */
    suspend fun isHealthy(): Boolean = withContext(Dispatchers.IO) {
        try {
            val conn = URL("http://127.0.0.1:$port/health")
                .openConnection() as HttpURLConnection
            conn.connectTimeout = 2000
            conn.readTimeout = 2000
            conn.responseCode == 200
        } catch (_: Exception) { false }
    }

    val wsUrl get() = "ws://127.0.0.1:$port"
}
```

---

### Step 3：新增 ZeroClawWsClient

替换 `ZeroClawBridge.kt` 中 `sendMessage` / `pollMessage` 的 TODO，实现真正的 WS 通信。

```kotlin
// app/src/main/java/ai/zeroclaw/android/ws/ZeroClawWsClient.kt
// 依赖：implementation("org.java-websocket:Java-WebSocket:1.5.4")
// 或使用 OkHttp WebSocket（项目中若已有 OkHttp 更佳）

class ZeroClawWsClient(
    private val baseWsUrl: String,
    private val agentName: String,
    private val token: String
) {
    // 消息流：UI 层订阅
    private val _messages = MutableSharedFlow<AgentMessage>(replay = 0)
    val messages: SharedFlow<AgentMessage> = _messages.asSharedFlow()

    private val _status = MutableStateFlow(ConnectionStatus.Disconnected)
    val status: StateFlow<ConnectionStatus> = _status.asStateFlow()

    private var webSocket: WebSocket? = null

    /** 建立 WS 连接
     *  连接地址格式与桌面端前端相同：
     *  ws://127.0.0.1:42620/api/v1/{agentName}/ws?token=xxx
     */
    fun connect() {
        val url = URI("$baseWsUrl/api/v1/$agentName/ws?token=$token")
        webSocket = object : WebSocketClient(url) {
            override fun onOpen(handshakedata: ServerHandshake) {
                _status.value = ConnectionStatus.Connected
            }
            override fun onMessage(message: String) {
                val msg = Json.decodeFromString<AgentMessage>(message)
                CoroutineScope(Dispatchers.IO).launch { _messages.emit(msg) }
            }
            override fun onClose(code: Int, reason: String, remote: Boolean) {
                _status.value = ConnectionStatus.Disconnected
            }
            override fun onError(ex: Exception) {
                _status.value = ConnectionStatus.Error(ex.message ?: "Unknown")
            }
        }.apply { connect() }
    }

    /** 发送消息到 Agent */
    fun sendMessage(content: String) {
        val payload = """{"type":"message","content":${Json.encodeToString(content)}}"""
        webSocket?.send(payload)
    }

    fun disconnect() {
        webSocket?.close()
        webSocket = null
    }
}

@Serializable
data class AgentMessage(
    val type: String,
    val content: String = "",
    val role: String = "assistant"
)

enum class ConnectionStatus {
    Disconnected, Connecting, Connected;
    data class Error(val msg: String) : ConnectionStatus()  // sealed 扩展
}
```

---

### Step 4：更新 ZeroClawService

将现有 TODO 替换为对 `ZeroClawProcessManager` 和 `ZeroClawWsClient` 的真实调用：

```kotlin
// 在 ZeroClawService.kt 中替换

class ZeroClawService : Service() {

    private val processManager by lazy { ZeroClawProcessManager(this) }
    private lateinit var wsClient: ZeroClawWsClient
    private val scope = CoroutineScope(Dispatchers.Default + SupervisorJob())

    override fun onCreate() {
        super.onCreate()
        startForeground(NOTIFICATION_ID, createNotification())
    }

    private fun startAgent() {
        if (_status.value == Status.Running) return
        _status.value = Status.Starting

        scope.launch {
            try {
                // 1. 启动 zeroclaw 子进程
                processManager.start().getOrThrow()

                // 2. 从配置获取 agent 名称和 token
                val settings = SettingsRepository(applicationContext).getSettings()
                wsClient = ZeroClawWsClient(
                    baseWsUrl = "ws://127.0.0.1:42620",
                    agentName = settings.agentName,
                    token = settings.token
                )

                // 3. 建立 WS 连接
                wsClient.connect()

                // 4. 订阅消息
                launch {
                    wsClient.messages.collect { msg ->
                        _lastMessage.value = msg.content
                    }
                }

                _status.value = Status.Running

            } catch (e: Exception) {
                _status.value = Status.Error(e.message ?: "启动失败")
            }
        }
    }

    private fun stopAgent() {
        scope.launch {
            wsClient.disconnect()
            processManager.stop()
            _status.value = Status.Stopped
        }
    }

    private fun sendMessage(message: String) {
        if (_status.value == Status.Running) {
            wsClient.sendMessage(message)
        }
    }
}
```

---

### Step 5：更新 HeartbeatWorker（保活）

`HeartbeatWorker` 已有骨架，补充健康检查逻辑：

```kotlin
class HeartbeatWorker(ctx: Context, params: WorkerParameters) :
    CoroutineWorker(ctx, params) {

    override suspend fun doWork(): Result {
        val manager = ZeroClawProcessManager(applicationContext)

        return if (manager.isHealthy()) {
            Result.success()
        } else {
            // zeroclaw 意外退出，重启 Service
            val intent = Intent(applicationContext, ZeroClawService::class.java)
                .setAction(ZeroClawService.ACTION_START)
            applicationContext.startForegroundService(intent)
            Result.retry()
        }
    }
}
```

---

### Step 6：更新 SettingsRepository

在设置中增加 `agentName` 和 `token` 字段（用于 WS 连接认证）：

```kotlin
// 在现有 DataStore 基础上新增字段
data class AppSettings(
    val provider: String = "anthropic",
    val model: String = "claude-sonnet-4-5",
    val apiKey: String = "",
    val agentName: String = "star",      // zeroclaw agent 名称
    val token: String = ""               // WS 认证 token
)
```

---

## 四、文件变更清单

### 新增文件

```
app/src/main/assets/
  zeroclaw-arm64                   ← 编译产物

app/src/main/java/ai/zeroclaw/android/
  process/ZeroClawProcessManager.kt   ← 进程管理（核心新增）
  ws/ZeroClawWsClient.kt              ← WS 通信（核心新增）
```

### 修改文件

```
app/src/main/java/ai/zeroclaw/android/
  service/ZeroClawService.kt          ← 接入 ProcessManager + WsClient
  worker/HeartbeatWorker.kt           ← 补充健康检查逻辑
  data/SettingsRepository.kt          ← 新增 agentName/token 字段
  ui/SettingsScreen.kt                ← 新增 Agent 名称/Token 配置项
  bridge/ZeroClawBridge.kt            ← 可废弃（或保留作接口门面）

build.gradle.kts                      ← 新增 WebSocket 依赖
```

### 不需要修改

```
MainActivity.kt                       ← Chat UI 不变
widget/ZeroClawWidget.kt              ← 不变
tile/ZeroClawTileService.kt           ← 不变
receiver/BootReceiver.kt              ← 不变
```

---

## 五、build.gradle.kts 依赖变更

```kotlin
dependencies {
    // 新增：WebSocket 客户端
    // 选项 A：Java-WebSocket（轻量，无额外依赖）
    implementation("org.java-websocket:Java-WebSocket:1.5.4")

    // 选项 B：OkHttp（如果项目中已有 OkHttp 更推荐）
    // implementation("com.squareup.okhttp3:okhttp:4.12.0")

    // 新增：JSON 序列化（Kotlin Serialization）
    implementation("org.jetbrains.kotlinx:kotlinx-serialization-json:1.7.3")

    // 已有，无需修改：
    // WorkManager, DataStore, security-crypto, Compose, Material3...
}
```

---

## 六、编译流程

### 开发构建

```bash
# 1. 编译 zeroclaw Android binary
cd huanxing-zeroclaw
cargo ndk -t arm64-v8a build --release --bin zeroclaw \
    --no-default-features --features "skill-creation"

# 2. 复制到 assets
cp target/aarch64-linux-android/release/zeroclaw \
   clients/android/app/src/main/assets/zeroclaw-arm64

# 3. 构建 APK
cd clients/android
./gradlew assembleDebug

# 4. 安装到设备
adb install app/build/outputs/apk/debug/app-debug.apk
```

### CI 集成（ci-android.yml 补充）

```yaml
- name: Install Rust Android targets
  run: rustup target add aarch64-linux-android armv7-linux-androideabi

- name: Build zeroclaw Android binaries
  run: |
    cargo ndk -t arm64-v8a -t armeabi-v7a build --release --bin zeroclaw \
        --no-default-features --features "skill-creation"
    cp target/aarch64-linux-android/release/zeroclaw \
       clients/android/app/src/main/assets/zeroclaw-arm64
    cp target/armv7-linux-androideabi/release/zeroclaw \
       clients/android/app/src/main/assets/zeroclaw-arm32

- name: Build APK
  run: cd clients/android && ./gradlew assembleRelease
```

---

## 七、Android 平台限制与应对

| 限制 | 影响 | 应对方案 |
|------|------|---------|
| 后台进程可能被系统杀死 | zeroclaw 子进程随时终止 | Foreground Service + HeartbeatWorker 监控重启（已实现骨架） |
| Android 12+ 前台服务限制 | 启动时机受限 | `FOREGROUND_SERVICE_DATA_SYNC` 已在 Manifest 声明 |
| SELinux 沙箱 | App 只能在自己 `filesDir` 执行文件 | binary 解压到 `filesDir/bin/`，无需 root |
| Shell/文件工具功能受限 | Agent 的 shell 工具能力弱 | 对话/LLM/Memory 功能完全不受影响 |
| 冷启动时 zeroclaw 二进制解压 | 首次启动延迟约 1-2s | 后台 Service onCreate 时预解压，UI 层显示 loading |
| APK 体积增加 | +5-8MB | 用 ABI splits，arm64 用户只下载 arm64 版 |

---

## 八、工作量估算

| 任务 | 预估时间 |
|------|---------|
| 交叉编译配置 + feature 裁剪调试 | 1 天 |
| ZeroClawProcessManager 实现 | 1 天 |
| ZeroClawWsClient 实现 | 0.5 天 |
| ZeroClawService 接线 + 联调 | 1 天 |
| Settings UI 更新 + HeartbeatWorker | 0.5 天 |
| **合计** | **约 4 天** |

---

## 九、与桌面端的代码复用

桌面端 `sidecar.rs` 已经实现了完整的进程管理逻辑，Android 端是其 Kotlin 翻译：

| 功能 | 桌面端 (Rust) | Android 端 (Kotlin) |
|------|--------------|---------------------|
| 启动子进程 | `tokio::process::Command` | `ProcessBuilder` |
| 健康检查轮询 | `reqwest::get("/health")` | `HttpURLConnection` |
| WS 通信 | 前端 `useZeroClawWs()` hook | `ZeroClawWsClient` |
| 进程保活 | `monitor_loop` 协程 | `HeartbeatWorker` (WorkManager) |
| 日志收集 | `child.stdout.take()` | `process.inputStream` |

配置目录约定（与桌面端保持一致风格）：

```
桌面端：~/.huanxing/config.toml
Android：/data/data/ai.zeroclaw.android/files/zeroclaw/config.toml
```

---

*文档版本：v1.0 | 2026-03-21*
*参考：clients/desktop/src-tauri/src/sidecar.rs · docs/HASN-centralized/14-端云协同.md*
