package cn.dcfuture.huanxing

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import cn.dcfuture.huanxing.service.EngineService

class MainActivity : TauriActivity() {

    companion object {
        private const val REQUEST_NOTIFICATION_PERMISSION = 1001
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        // Android 13+ (API 33) 需要运行时请求 POST_NOTIFICATIONS
        requestNotificationPermission()

        // 启动引擎前台服务
        EngineService.start(this)
    }

    override fun onDestroy() {
        // 注意：不在这里停止引擎！
        // 引擎前台服务需要在 Activity 销毁后继续运行。
        // 用户可通过通知栏的"停止"按钮手动停止。
        super.onDestroy()
    }

    /** Android 13+ 需要运行时通知权限 */
    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (ContextCompat.checkSelfPermission(
                    this,
                    Manifest.permission.POST_NOTIFICATIONS
                ) != PackageManager.PERMISSION_GRANTED
            ) {
                ActivityCompat.requestPermissions(
                    this,
                    arrayOf(Manifest.permission.POST_NOTIFICATIONS),
                    REQUEST_NOTIFICATION_PERMISSION
                )
            }
        }
    }
}
