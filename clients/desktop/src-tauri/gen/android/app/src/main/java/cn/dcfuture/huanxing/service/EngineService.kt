package cn.dcfuture.huanxing.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import cn.dcfuture.huanxing.MainActivity
import cn.dcfuture.huanxing.R

/**
 * 唤星 AI 引擎前台服务
 *
 * ZeroClaw 引擎需要在后台持续运行以维持：
 * - WebSocket 长连接（HASN 社交网络）
 * - Heartbeat 定时任务（AI Agent 自主行为）
 * - Gateway HTTP 服务（本地 API 端点）
 *
 * 使用 START_STICKY 确保系统回收后自动重启。
 * 持久通知让用户随时知道引擎在后台运行。
 */
class EngineService : Service() {

    companion object {
        private const val CHANNEL_ID = "huanxing_engine_channel"
        private const val NOTIFICATION_ID = 42620 // 与引擎端口一致作为标识
        private const val WAKELOCK_TAG = "huanxing:engine-wakelock"

        /** 从任意 Context 启动引擎服务 */
        fun start(context: Context) {
            val intent = Intent(context, EngineService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }

        /** 停止引擎服务 */
        fun stop(context: Context) {
            context.stopService(Intent(context, EngineService::class.java))
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private var startTimeMs: Long = 0L

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
        startTimeMs = System.currentTimeMillis()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        // 构建持久通知
        val notification = buildNotification("唤星AI 守护中")
        startForeground(NOTIFICATION_ID, notification)

        // 获取 partial wakelock 防止 CPU 休眠
        acquireWakeLock()

        // 启动运行时长更新
        startUptimeUpdater()

        return START_STICKY // 被杀后系统自动重启
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onDestroy() {
        releaseWakeLock()
        super.onDestroy()
    }

    // ── 通知管理 ──

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "唤星AI 引擎服务",
                NotificationManager.IMPORTANCE_LOW // 不打扰用户
            ).apply {
                description = "ZeroClaw AI 引擎后台运行状态"
                setShowBadge(false)
                lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            }
            val manager = getSystemService(NotificationManager::class.java)
            manager.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(text: String): Notification {
        // 点击通知回到主界面
        val tapIntent = Intent(this, MainActivity::class.java).apply {
            flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
        }
        val pendingTap = PendingIntent.getActivity(
            this, 0, tapIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        // 停止按钮
        val stopIntent = Intent(this, EngineService::class.java).apply {
            action = "STOP_ENGINE"
        }
        val pendingStop = PendingIntent.getService(
            this, 1, stopIntent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )

        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("唤星AI")
            .setContentText(text)
            .setSmallIcon(R.mipmap.ic_launcher) // TODO: 替换为专属通知图标
            .setOngoing(true)
            .setSilent(true)
            .setContentIntent(pendingTap)
            .addAction(0, "停止", pendingStop)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    /** 定时更新通知显示运行时长 */
    private fun startUptimeUpdater() {
        Thread {
            while (true) {
                try {
                    Thread.sleep(60_000) // 每分钟更新一次
                } catch (_: InterruptedException) {
                    break
                }
                val uptimeMs = System.currentTimeMillis() - startTimeMs
                val hours = uptimeMs / 3_600_000
                val minutes = (uptimeMs % 3_600_000) / 60_000
                val text = if (hours > 0) {
                    "唤星AI 守护中 · 已运行 ${hours}h${minutes}m"
                } else {
                    "唤星AI 守护中 · 已运行 ${minutes}m"
                }
                val notification = buildNotification(text)
                val manager = getSystemService(NotificationManager::class.java)
                manager.notify(NOTIFICATION_ID, notification)
            }
        }.start()
    }

    // ── WakeLock 管理 ──

    private fun acquireWakeLock() {
        val powerManager = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = powerManager.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            WAKELOCK_TAG
        ).apply {
            acquire() // 无超时 — 引擎需要持续运行
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let {
            if (it.isHeld) it.release()
        }
        wakeLock = null
    }
}
