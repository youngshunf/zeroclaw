# Android 端 Agent 手机能力 MVP 设计方案

**对接 zeroclaw Tool 体系 · 桥接架构 · MVP 六大能力**

---

## 一、核心架构：Android Tool Bridge

zeroclaw 是 Android App 的**子进程**，没有 Android Context，无法直接调用系统 API。
解决方案：**双向本地 HTTP 桥接**。

```
┌─────────────────────────────────────────────────────────────┐
│  LLM 决策层                                                  │
│  "帮我拍一张照片分析热量"                                    │
└──────────────────────┬──────────────────────────────────────┘
                       │ Tool Call: camera_take_photo()
┌──────────────────────▼──────────────────────────────────────┐
│  zeroclaw 子进程 (:42620)                                    │
│                                                              │
│  AndroidTool（Rust）                                         │
│  实现标准 Tool trait                                         │
│  └─ execute() → POST http://127.0.0.1:42621/tools/camera    │
└──────────────────────┬──────────────────────────────────────┘
                       │ HTTP localhost（进程间通信）
┌──────────────────────▼──────────────────────────────────────┐
│  Android App 进程                                            │
│                                                              │
│  AndroidToolBridgeServer（Kotlin，:42621）                   │
│  路由分发 → 调用真实 Android API                             │
│  ├─ /tools/camera    → CameraHandler（调 CameraX）          │
│  ├─ /tools/location  → LocationHandler（调 FusedLocation）  │
│  ├─ /tools/apps      → AppsHandler（调 PackageManager）     │
│  ├─ /tools/clipboard → ClipboardHandler（调 ClipboardManager）│
│  └─ /tools/notify    → NotifyHandler（调 NotificationManager）│
└─────────────────────────────────────────────────────────────┘
```

**为什么用 HTTP 而不是 UNIX Socket / IPC？**
- zeroclaw 已有完整 HTTP/reqwest 客户端，直接复用
- 无需处理跨进程序列化
- 调试方便，可以直接 curl 测试
- 与 zeroclaw 现有 `HttpRequestTool` 模式完全一致

---

## 二、MVP 六大能力

### 能力一：camera_take_photo

```
用途：拍照并返回 base64 图片，直接传给 LLM 视觉分析
示例：
  用户："帮我看看这道菜有多少热量"
  Agent：调用 camera_take_photo() → 得到图片 → 传给 LLM 分析
```

**参数**

```json
{
  "type": "object",
  "properties": {
    "quality": {
      "type": "string",
      "enum": ["low", "medium", "high"],
      "default": "medium",
      "description": "图片质量，low=512px，medium=1024px，high=原图"
    }
  }
}
```

**返回**

```json
{
  "success": true,
  "output": "{\"image_base64\": \"...\", \"width\": 1024, \"height\": 768, \"format\": \"jpeg\"}"
}
```

**Android 实现**

- 使用 `ActivityResultContracts.TakePicture()`
- 需要权限：`CAMERA`
- 图片存 `cacheDir`，返回后删除
- quality=low 时 resize 到 512px（节省 token）

---

### 能力二：location_get

```
用途：获取当前 GPS 位置
示例：
  用户："帮我找附近的饺子馆"
  Agent：调用 location_get() → 得到坐标 → 调用 web_search("饺子馆 附近 北京朝阳")
```

**参数**

```json
{
  "type": "object",
  "properties": {
    "accuracy": {
      "type": "string",
      "enum": ["coarse", "fine"],
      "default": "coarse",
      "description": "coarse=网络定位(快,省电)，fine=GPS(精确但慢)"
    },
    "timeout_seconds": {
      "type": "integer",
      "default": 10
    }
  }
}
```

**返回**

```json
{
  "success": true,
  "output": "{\"latitude\": 39.9042, \"longitude\": 116.4074, \"accuracy_meters\": 15, \"city\": \"北京市\", \"district\": \"朝阳区\"}"
}
```

**Android 实现**

- `FusedLocationProviderClient.getCurrentLocation()`
- 需要权限：`ACCESS_COARSE_LOCATION` / `ACCESS_FINE_LOCATION`
- 通过高德/腾讯地图 API 反地理编码得到中文地址（可选）

---

### 能力三：apps_list / apps_launch

#### apps_list

```
用途：获取已安装的 App 列表（过滤系统 App）
示例：
  用户："我手机上有什么音乐 App？"
  Agent：调用 apps_list(category="music")
```

**参数**

```json
{
  "type": "object",
  "properties": {
    "include_system": {
      "type": "boolean",
      "default": false
    },
    "keyword": {
      "type": "string",
      "description": "按名称过滤"
    }
  }
}
```

**返回**

```json
{
  "success": true,
  "output": "[{\"name\":\"网易云音乐\",\"package\":\"com.netease.cloudmusic\",\"version\":\"8.9.0\"},{\"name\":\"QQ音乐\",\"package\":\"com.tencent.qqmusic\",\"version\":\"12.3.0\"}]"
}
```

#### apps_launch

```
用途：打开指定 App 或通过深链接跳转
示例：
  用户："帮我打开微信"
  用户："打开高德地图导航到北京西站"
```

**参数**

```json
{
  "type": "object",
  "properties": {
    "package": {
      "type": "string",
      "description": "包名，如 com.tencent.mm"
    },
    "deep_link": {
      "type": "string",
      "description": "深链接 URL，如 maps://route?to=北京西站"
    }
  },
  "oneOf": [
    {"required": ["package"]},
    {"required": ["deep_link"]}
  ]
}
```

**返回**

```json
{"success": true, "output": "已打开 微信"}
```

**Android 实现**

- `PackageManager.getInstalledApplications()`，过滤 `FLAG_SYSTEM`
- `context.startActivity(Intent(Intent.ACTION_MAIN))`
- 深链接：`Intent.ACTION_VIEW` + URI

---

### 能力四：clipboard_read / clipboard_write

```
用途：读写剪贴板，配合其他 App 协作
示例：
  用户复制了一段英文文本
  用户："帮我翻译剪贴板里的内容"
  Agent：调用 clipboard_read() → 得到文本 → 翻译 → clipboard_write(译文)
```

**clipboard_read 参数**：无

**返回**

```json
{"success": true, "output": "{\"text\": \"Hello world\", \"has_content\": true}"}
```

**clipboard_write 参数**

```json
{
  "type": "object",
  "required": ["text"],
  "properties": {
    "text": {"type": "string"}
  }
}
```

**Android 实现**

- `ClipboardManager.primaryClip`
- Android 10+ 限制后台读取剪贴板：需要 App 在前台，或有 `READ_CLIPBOARD_IN_BACKGROUND` 权限（仅系统 App 可用）
- **应对方案**：读剪贴板时通过通知提示用户切回 App 前台

---

### 能力五：notify_send

```
用途：Agent 主动推送通知给用户（任务完成、提醒等）
示例：
  Agent 完成长任务后："已帮你整理完日程，点击查看"
  用户设置的提醒到期时：Agent 发通知
```

**参数**

```json
{
  "type": "object",
  "required": ["title", "body"],
  "properties": {
    "title":    {"type": "string"},
    "body":     {"type": "string"},
    "priority": {"type": "string", "enum": ["low", "normal", "high"], "default": "normal"},
    "action":   {"type": "string", "description": "点击通知触发的深链接或消息"}
  }
}
```

**返回**

```json
{"success": true, "output": "通知已发送"}
```

**Android 实现**

- `NotificationCompat.Builder` + `NotificationManagerCompat`
- 权限：`POST_NOTIFICATIONS`（已在 Manifest 声明）
- 通知渠道：已在 `ZeroClawApp.kt` 创建

---

### 能力六：device_info

```
用途：获取设备基础状态，Agent 可据此调整行为
示例：
  Agent 检测到电量低 → 提醒用户充电再执行耗电任务
  Agent 检测到无网络 → 提示用户
```

**参数**：无

**返回**

```json
{
  "success": true,
  "output": "{\"battery_level\": 42, \"charging\": false, \"network\": \"wifi\", \"storage_free_gb\": 12.5, \"model\": \"Pixel 8\"}"
}
```

**Android 实现**

- `BatteryManager` / `ConnectivityManager` / `StatFs`
- 无需额外权限

---

## 三、代码实现

### 3.1 Android 侧：AndroidToolBridgeServer

```kotlin
// app/src/main/java/ai/zeroclaw/android/bridge/AndroidToolBridgeServer.kt

/**
 * 本地 HTTP 服务器，暴露 Android 系统能力给 zeroclaw 子进程
 * 监听 127.0.0.1:42621（仅本地，不对外）
 *
 * 依赖：implementation("com.sun.net.httpserver") 或 NanoHTTPD
 * 推荐：NanoHTTPD（轻量，无额外依赖）
 * implementation("org.nanohttpd:nanohttpd:2.3.1")
 */
class AndroidToolBridgeServer(private val context: Context) :
    NanoHTTPD("127.0.0.1", BRIDGE_PORT) {

    companion object {
        const val BRIDGE_PORT = 42621
        private val JSON = "application/json"
    }

    // 各能力 Handler
    private val cameraHandler    = CameraHandler(context)
    private val locationHandler  = LocationHandler(context)
    private val appsHandler      = AppsHandler(context)
    private val clipboardHandler = ClipboardHandler(context)
    private val notifyHandler    = NotifyHandler(context)
    private val deviceHandler    = DeviceInfoHandler(context)

    override fun serve(session: IHTTPSession): Response {
        if (session.method != Method.POST) {
            return newFixedLengthResponse(
                Response.Status.METHOD_NOT_ALLOWED, JSON,
                """{"error":"only POST allowed"}"""
            )
        }

        // 读取请求体
        val body = mutableMapOf<String, String>()
        session.parseBody(body)
        val args = body["postData"] ?: "{}"

        return try {
            val result = when (session.uri) {
                "/tools/camera"    -> cameraHandler.handle(args)
                "/tools/location"  -> locationHandler.handle(args)
                "/tools/apps/list" -> appsHandler.handleList(args)
                "/tools/apps/launch" -> appsHandler.handleLaunch(args)
                "/tools/clipboard/read"  -> clipboardHandler.handleRead()
                "/tools/clipboard/write" -> clipboardHandler.handleWrite(args)
                "/tools/notify"    -> notifyHandler.handle(args)
                "/tools/device"    -> deviceHandler.handle()
                "/health"          -> """{"status":"ok"}"""
                else -> """{"error":"unknown tool: ${session.uri}"}"""
            }
            newFixedLengthResponse(Response.Status.OK, JSON, result)
        } catch (e: Exception) {
            newFixedLengthResponse(
                Response.Status.INTERNAL_ERROR, JSON,
                """{"error":"${e.message?.replace("\"","\'")}"}"""
            )
        }
    }
}
```

### 3.2 典型 Handler 实现（location 为例）

```kotlin
// app/src/main/java/ai/zeroclaw/android/bridge/handlers/LocationHandler.kt

class LocationHandler(private val context: Context) {

    @SuppressLint("MissingPermission")
    suspend fun handle(argsJson: String): String {
        val args = JSONObject(argsJson)
        val fine = args.optString("accuracy", "coarse") == "fine"
        val timeout = args.optLong("timeout_seconds", 10) * 1000

        if (!hasLocationPermission()) {
            return """{"error":"缺少位置权限，请在设置中授权"}"""
        }

        val client = LocationServices.getFusedLocationProviderClient(context)
        val priority = if (fine) Priority.PRIORITY_HIGH_ACCURACY
                       else Priority.PRIORITY_BALANCED_POWER_ACCURACY

        return withTimeoutOrNull(timeout) {
            suspendCancellableCoroutine { cont ->
                client.getCurrentLocation(priority, null)
                    .addOnSuccessListener { loc ->
                        if (loc != null) {
                            cont.resume(JSONObject().apply {
                                put("latitude",  loc.latitude)
                                put("longitude", loc.longitude)
                                put("accuracy_meters", loc.accuracy.toInt())
                            }.toString())
                        } else {
                            cont.resume("""{"error":"无法获取位置，请检查 GPS 是否开启"}""")
                        }
                    }
                    .addOnFailureListener { e ->
                        cont.resume("""{"error":"${e.message}"}""")
                    }
            }
        } ?: """{"error":"定位超时（${timeout/1000}s）"}"""
    }

    private fun hasLocationPermission() =
        ActivityCompat.checkSelfPermission(context, ACCESS_COARSE_LOCATION) == PERMISSION_GRANTED
}
```

### 3.3 zeroclaw 侧：AndroidTool（Rust）

在 `src/huanxing/` 下新增，遵循唤星扩展层规范：

```rust
// src/huanxing/tools/android_tool.rs

use crate::tools::traits::{Tool, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};

const BRIDGE_BASE: &str = "http://127.0.0.1:42621";

/// 通用 Android 桥接 Tool
/// 将 Android 系统能力包装为标准 zeroclaw Tool
pub struct AndroidTool {
    /// Tool 名称，对应桥接服务器路由
    tool_name: String,
    /// 对 LLM 展示的名称
    display_name: String,
    description: String,
    parameters_schema: Value,
    /// 桥接端点路径，如 /tools/camera
    endpoint: String,
}

impl AndroidTool {
    pub fn new(
        tool_name: impl Into<String>,
        display_name: impl Into<String>,
        description: impl Into<String>,
        parameters_schema: Value,
        endpoint: impl Into<String>,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            display_name: display_name.into(),
            description: description.into(),
            parameters_schema,
            endpoint: endpoint.into(),
        }
    }
}

#[async_trait]
impl Tool for AndroidTool {
    fn name(&self) -> &str { &self.tool_name }
    fn description(&self) -> &str { &self.description }
    fn parameters_schema(&self) -> Value { self.parameters_schema.clone() }

    async fn execute(&self, args: Value) -> anyhow::Result<ToolResult> {
        let url = format!("{}{}", BRIDGE_BASE, self.endpoint);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        let resp = client
            .post(&url)
            .json(&args)
            .send()
            .await;

        match resp {
            Ok(r) => {
                let body: Value = r.json().await.unwrap_or(json!({}));
                if let Some(err) = body.get("error").and_then(|e| e.as_str()) {
                    Ok(ToolResult { success: false, output: String::new(), error: Some(err.to_string()) })
                } else {
                    Ok(ToolResult { success: true, output: body.to_string(), error: None })
                }
            }
            Err(e) => {
                // 桥接服务不可达（App 未在前台？权限未授）
                Ok(ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Android 桥接服务不可达: {e}。请确保唤星 App 在前台运行。")),
                })
            }
        }
    }
}
```

### 3.4 构建 MVP Tools 集合

```rust
// src/huanxing/tools/mod.rs

use super::android_tool::AndroidTool;
use crate::tools::traits::Tool;
use serde_json::json;
use std::sync::Arc;

/// 返回 Android 端所有 MVP Tools
/// 在 all_tools_with_runtime() 中通过 feature gate 注入
pub fn android_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(AndroidTool::new(
            "camera_take_photo",
            "拍照",
            "使用手机摄像头拍照，返回图片用于分析。适用于识别物体、分析食物热量、扫描文字等场景。",
            json!({
                "type": "object",
                "properties": {
                    "quality": {
                        "type": "string",
                        "enum": ["low", "medium", "high"],
                        "default": "medium",
                        "description": "图片质量：low=512px(省token), medium=1024px(推荐), high=原图"
                    }
                }
            }),
            "/tools/camera",
        )),

        Arc::new(AndroidTool::new(
            "location_get",
            "获取位置",
            "获取手机当前 GPS 位置（经纬度）。用于查找附近地点、基于位置的推荐、导航等场景。",
            json!({
                "type": "object",
                "properties": {
                    "accuracy": {
                        "type": "string",
                        "enum": ["coarse", "fine"],
                        "default": "coarse",
                        "description": "coarse=网络定位(快,省电), fine=GPS(精确,慢)"
                    }
                }
            }),
            "/tools/location",
        )),

        Arc::new(AndroidTool::new(
            "apps_list",
            "查看已安装 App",
            "获取手机上已安装的 App 列表（过滤系统 App）。可按名称关键词过滤。",
            json!({
                "type": "object",
                "properties": {
                    "keyword": {
                        "type": "string",
                        "description": "按名称过滤，如 '音乐'、'地图'"
                    },
                    "include_system": {
                        "type": "boolean",
                        "default": false
                    }
                }
            }),
            "/tools/apps/list",
        )),

        Arc::new(AndroidTool::new(
            "apps_launch",
            "打开 App",
            "打开手机上的指定 App，或通过深链接跳转到特定页面。",
            json!({
                "type": "object",
                "properties": {
                    "package": {
                        "type": "string",
                        "description": "App 包名，如 com.tencent.mm（微信）"
                    },
                    "deep_link": {
                        "type": "string",
                        "description": "深链接 URL，如 geo:39.9,116.4（地图）"
                    }
                }
            }),
            "/tools/apps/launch",
        )),

        Arc::new(AndroidTool::new(
            "clipboard_read",
            "读取剪贴板",
            "读取手机剪贴板中的文本内容。适用于翻译、处理用户复制的内容。",
            json!({"type": "object", "properties": {}}),
            "/tools/clipboard/read",
        )),

        Arc::new(AndroidTool::new(
            "clipboard_write",
            "写入剪贴板",
            "将文本写入手机剪贴板，方便用户粘贴到其他 App。",
            json!({
                "type": "object",
                "required": ["text"],
                "properties": {
                    "text": {"type": "string", "description": "要写入剪贴板的文本"}
                }
            }),
            "/tools/clipboard/write",
        )),

        Arc::new(AndroidTool::new(
            "notify_send",
            "发送通知",
            "向用户发送手机通知。适用于任务完成提醒、重要信息推送。",
            json!({
                "type": "object",
                "required": ["title", "body"],
                "properties": {
                    "title":    {"type": "string"},
                    "body":     {"type": "string"},
                    "priority": {"type": "string", "enum": ["low","normal","high"], "default": "normal"}
                }
            }),
            "/tools/notify",
        )),

        Arc::new(AndroidTool::new(
            "device_info",
            "获取设备状态",
            "获取手机电量、网络状态、存储空间等基础信息。",
            json!({"type": "object", "properties": {}}),
            "/tools/device",
        )),
    ]
}
```

### 3.5 注入 zeroclaw Tool 注册（feature gate）

```rust
// src/tools/mod.rs — 在 all_tools_with_runtime() 中追加

#[cfg(feature = "android-tools")]
{
    use crate::huanxing::tools::android_tools;
    for t in android_tools() {
        tool_arcs.push(t);
    }
}
```

```toml
# Cargo.toml — 新增 feature
[features]
android-tools = []  # 编译 Android 版本时开启
```

```bash
# 编译时启用
cargo ndk -t arm64-v8a build --release --bin zeroclaw \
    --no-default-features \
    --features "skill-creation,android-tools"
```

---

## 四、权限声明

在 `AndroidManifest.xml` 追加（已有的不重复）：

```xml
<!-- 摄像头 -->
<uses-permission android:name="android.permission.CAMERA" />

<!-- 位置 -->
<uses-permission android:name="android.permission.ACCESS_COARSE_LOCATION" />
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />

<!-- 已有：INTERNET, FOREGROUND_SERVICE, POST_NOTIFICATIONS -->
```

---

## 五、运行时权限请求

在 `MainActivity` 或 `SettingsScreen` 中集中处理：

```kotlin
// 需要运行时动态申请的权限（Android 6.0+）
val REQUIRED_PERMISSIONS = arrayOf(
    Manifest.permission.CAMERA,
    Manifest.permission.ACCESS_COARSE_LOCATION,
    Manifest.permission.POST_NOTIFICATIONS,  // Android 13+
)

// 第一次启动 Agent 前检查，缺少时引导用户授权
// 用户拒绝时，对应 Tool 会返回明确错误信息，Agent 会告知用户
```

---

## 六、文件变更清单

### 新增文件

```
clients/android/app/src/main/java/ai/zeroclaw/android/
  bridge/AndroidToolBridgeServer.kt     ← 桥接 HTTP 服务器
  bridge/handlers/CameraHandler.kt
  bridge/handlers/LocationHandler.kt
  bridge/handlers/AppsHandler.kt
  bridge/handlers/ClipboardHandler.kt
  bridge/handlers/NotifyHandler.kt
  bridge/handlers/DeviceInfoHandler.kt

src/huanxing/tools/
  mod.rs                                ← android_tools() 工厂函数
  android_tool.rs                       ← AndroidTool Rust 实现
```

### 修改文件

```
clients/android/app/src/main/AndroidManifest.xml   ← 新增权限
clients/android/build.gradle.kts                   ← 新增 NanoHTTPD 依赖
clients/android/app/src/main/java/.../
  service/ZeroClawService.kt                       ← onCreate 启动桥接服务器
  ZeroClawApp.kt                                   ← Application 层初始化

src/tools/mod.rs                                   ← 注入 android_tools()
Cargo.toml                                         ← 新增 android-tools feature
```

---

## 七、启动流程

```
App 启动
  │
  ├─ ZeroClawApp.onCreate()
  │   └─ AndroidToolBridgeServer.start()  ← 监听 127.0.0.1:42621
  │
  ├─ ZeroClawService.startAgent()
  │   ├─ 解压 zeroclaw binary
  │   ├─ spawn 子进程（带 --features android-tools）
  │   └─ 等待 :42620 健康检查通过
  │
  └─ 用户开始对话
      │
      用户："帮我拍张照分析这个食物"
      Agent：调用 camera_take_photo()
        → zeroclaw AndroidTool.execute()
        → POST http://127.0.0.1:42621/tools/camera
        → CameraHandler 调用 CameraX 拍照
        → 返回 base64 图片
        → LLM 分析结果返回用户
```

---

## 八、工作量估算

| 模块 | 任务 | 预估 |
|------|------|------|
| Rust 侧 | AndroidTool 通用结构 + android_tools() 工厂 | 0.5 天 |
| Kotlin 侧 | AndroidToolBridgeServer 框架 + 路由 | 0.5 天 |
| Kotlin 侧 | 6 个 Handler 实现 | 2 天 |
| 集成 | 权限处理 + Service 接线 + 联调 | 1 天 |
| **合计** | | **约 4 天** |

---

## 九、后续扩展路径

MVP 完成后，扩展新能力只需：

1. **Android 侧**：新增一个 `XxxHandler.kt`，在 `AndroidToolBridgeServer` 加路由
2. **Rust 侧**：在 `android_tools()` 追加一个 `AndroidTool::new(...)` 声明

完全不需要修改 zeroclaw 核心代码，满足唤星扩展层规范。

**下一阶段候选能力**：
- `media_pick`：从相册选图
- `contacts_search`：搜索联系人
- `calendar_query`：查询/创建日程（对接 HASN A2A 调度）
- `audio_record`：录音转文字
- `screen_capture`：截屏（为 Accessibility 做铺垫）
- `sms_read`：读取短信验证码

---

*文档版本：v1.0 | 2026-03-21*
*参考：src/tools/traits.rs · src/tools/http_request.rs · clients/desktop/src-tauri/src/sidecar.rs*
