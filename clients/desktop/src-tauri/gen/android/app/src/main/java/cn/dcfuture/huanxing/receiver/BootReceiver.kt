package cn.dcfuture.huanxing.receiver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import cn.dcfuture.huanxing.service.EngineService

/**
 * 开机自启广播接收器
 *
 * 设备重启后自动重新启动 ZeroClaw 引擎前台服务。
 * 确保 AI Agent 的 Heartbeat 和 HASN 连接在重启后恢复。
 *
 * 监听：
 * - BOOT_COMPLETED — 标准开机完成
 * - QUICKBOOT_POWERON — 部分 OEM 的快速启动
 */
class BootReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED ||
            intent.action == "android.intent.action.QUICKBOOT_POWERON"
        ) {
            // 检查用户是否已登录（config.toml 存在）
            val configDir = context.filesDir.resolve(".huanxing")
            val configFile = configDir.resolve("config.toml")

            if (configFile.exists()) {
                android.util.Log.i(
                    "HuanXing",
                    "Boot completed: restarting engine service"
                )
                EngineService.start(context)
            } else {
                android.util.Log.i(
                    "HuanXing",
                    "Boot completed: no config found, skipping engine start"
                )
            }
        }
    }
}
