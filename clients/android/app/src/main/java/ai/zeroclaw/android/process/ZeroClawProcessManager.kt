package ai.zeroclaw.android.process

import android.content.Context
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL

/**
 * zeroclaw 子进程管理器。
 *
 * 负责：binary 解压 → 启动子进程 → 健康检查 → 日志收集 → 停止。
 * 架构与桌面端 Tauri sidecar（sidecar.rs）完全一致。
 */
class ZeroClawProcessManager(private val context: Context) {

    companion object {
        private const val TAG = "ProcessManager"
        const val DEFAULT_PORT = 42620
        private const val HEALTH_TIMEOUT_MS = 15_000L
        private const val HEALTH_POLL_INTERVAL_MS = 500L
        private const val HTTP_TIMEOUT_MS = 2_000
    }

    private val extractor = BinaryExtractor(context)
    private var process: Process? = null
    private var stdoutJob: Job? = null
    private var stderrJob: Job? = null

    val port: Int = DEFAULT_PORT

    val isRunning: Boolean
        get() = try {
            process?.exitValue()
            false // exitValue() 不抛异常说明已退出
        } catch (_: IllegalThreadStateException) {
            true // 抛异常说明还在运行
        }

    /** 配置目录：filesDir/zeroclaw/ */
    val configDir: File
        get() = File(context.filesDir, "zeroclaw").also { it.mkdirs() }

    /** 启动子进程 */
    suspend fun start(): Result<Unit> = withContext(Dispatchers.IO) {
        runCatching {
            if (isRunning) {
                Log.i(TAG, "子进程已在运行，跳过启动")
                return@runCatching
            }

            val bin = extractor.extractIfNeeded()
            Log.i(TAG, "启动 zeroclaw: ${bin.absolutePath} gateway -p $port")

            process = ProcessBuilder(
                bin.absolutePath,
                "gateway",
                "-p", port.toString(),
                "--home", configDir.absolutePath
            )
                .directory(context.filesDir)
                .redirectErrorStream(false)
                .start()

            startLogCollectors()
            waitUntilHealthy()

            Log.i(TAG, "zeroclaw 启动成功，端口 $port")
        }
    }

    /** 停止子进程 */
    fun stop() {
        stdoutJob?.cancel()
        stderrJob?.cancel()
        process?.destroy()
        process = null
        Log.i(TAG, "子进程已停止")
    }

    /** 单次健康检查 */
    suspend fun isHealthy(): Boolean = withContext(Dispatchers.IO) {
        try {
            val conn = URL("http://127.0.0.1:$port/health")
                .openConnection() as HttpURLConnection
            conn.connectTimeout = HTTP_TIMEOUT_MS
            conn.readTimeout = HTTP_TIMEOUT_MS
            val healthy = conn.responseCode == 200
            conn.disconnect()
            healthy
        } catch (_: Exception) {
            false
        }
    }

    /** 轮询健康检查直到成功或超时 */
    private suspend fun waitUntilHealthy() {
        val deadline = System.currentTimeMillis() + HEALTH_TIMEOUT_MS
        while (System.currentTimeMillis() < deadline) {
            delay(HEALTH_POLL_INTERVAL_MS)
            if (isHealthy()) {
                Log.i(TAG, "健康检查通过")
                return
            }
            // 检查进程是否已退出
            if (!isRunning) {
                throw RuntimeException("zeroclaw 进程意外退出")
            }
        }
        throw RuntimeException("zeroclaw 启动超时（${HEALTH_TIMEOUT_MS / 1000}s），健康检查未通过")
    }

    /** 收集子进程 stdout/stderr 到 Android Log */
    private fun startLogCollectors() {
        val proc = process ?: return
        val logScope = CoroutineScope(Dispatchers.IO)

        stdoutJob = logScope.launch {
            try {
                proc.inputStream.bufferedReader().forEachLine { line ->
                    Log.d(TAG, "[stdout] $line")
                }
            } catch (_: Exception) { }
        }
        stderrJob = logScope.launch {
            try {
                proc.errorStream.bufferedReader().forEachLine { line ->
                    Log.w(TAG, "[stderr] $line")
                }
            } catch (_: Exception) { }
        }
    }
}
