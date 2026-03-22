package ai.zeroclaw.android.worker

import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import androidx.work.*
import ai.zeroclaw.android.process.ZeroClawProcessManager
import ai.zeroclaw.android.service.ZeroClawService
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.concurrent.TimeUnit

/**
 * WorkManager worker：周期性健康检查 + 自动重启。
 *
 * zeroclaw 意外退出时，通过 startForegroundService 重启 ZeroClawService。
 * WorkManager 最小间隔 15 分钟（Android 限制）。
 */
class HeartbeatWorker(
    context: Context,
    params: WorkerParameters
) : CoroutineWorker(context, params) {

    override suspend fun doWork(): Result = withContext(Dispatchers.IO) {
        try {
            val taskType = inputData.getString(KEY_TASK_TYPE) ?: TASK_HEARTBEAT

            when (taskType) {
                TASK_HEARTBEAT -> runHeartbeat()
                TASK_CRON -> runCronJob()
                TASK_HEALTH_CHECK -> runHealthCheck()
                else -> runHeartbeat()
            }

            Result.success()
        } catch (e: Exception) {
            Log.w(TAG, "Worker 执行失败: ${e.message}")
            if (runAttemptCount < 3) {
                Result.retry()
            } else {
                Result.failure(workDataOf(KEY_ERROR to e.message))
            }
        }
    }

    private suspend fun runHeartbeat() {
        val manager = ZeroClawProcessManager(applicationContext)
        val healthy = manager.isHealthy()
        Log.d(TAG, "Heartbeat: ${if (healthy) "OK" else "FAIL"}")

        if (!healthy) {
            Log.w(TAG, "zeroclaw 不健康，请求重启 Service")
            restartService()
        }
    }

    private suspend fun runCronJob() {
        val jobId = inputData.getString(KEY_JOB_ID)
        val prompt = inputData.getString(KEY_PROMPT)
        // TODO: Phase 2 实现 cron 任务执行
        Log.d(TAG, "Cron job: $jobId (未实现)")
    }

    private suspend fun runHealthCheck() {
        val manager = ZeroClawProcessManager(applicationContext)
        val healthy = manager.isHealthy()
        Log.d(TAG, "Health check: ${if (healthy) "OK" else "FAIL"}")

        if (!healthy) {
            restartService()
            throw RuntimeException("健康检查失败，已请求重启")
        }
    }

    private fun restartService() {
        val intent = Intent(applicationContext, ZeroClawService::class.java)
            .setAction(ZeroClawService.ACTION_START)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            applicationContext.startForegroundService(intent)
        } else {
            applicationContext.startService(intent)
        }
    }

    companion object {
        private const val TAG = "HeartbeatWorker"

        const val KEY_TASK_TYPE = "task_type"
        const val KEY_JOB_ID = "job_id"
        const val KEY_PROMPT = "prompt"
        const val KEY_ERROR = "error"

        const val TASK_HEARTBEAT = "heartbeat"
        const val TASK_CRON = "cron"
        const val TASK_HEALTH_CHECK = "health_check"

        const val WORK_NAME_HEARTBEAT = "zeroclaw_heartbeat"

        /** 调度周期性心跳（最小 15 分钟） */
        fun scheduleHeartbeat(context: Context, intervalMinutes: Long = 15) {
            val effectiveInterval = maxOf(intervalMinutes, 15L)

            val constraints = Constraints.Builder()
                .setRequiredNetworkType(NetworkType.CONNECTED)
                .build()

            val request = PeriodicWorkRequestBuilder<HeartbeatWorker>(
                effectiveInterval, TimeUnit.MINUTES
            )
                .setConstraints(constraints)
                .setInputData(workDataOf(KEY_TASK_TYPE to TASK_HEARTBEAT))
                .setBackoffCriteria(BackoffPolicy.EXPONENTIAL, 1, TimeUnit.MINUTES)
                .build()

            WorkManager.getInstance(context).enqueueUniquePeriodicWork(
                WORK_NAME_HEARTBEAT,
                ExistingPeriodicWorkPolicy.UPDATE,
                request
            )
        }

        /** 调度一次性 cron 任务 */
        fun scheduleCronJob(
            context: Context,
            jobId: String,
            prompt: String,
            delayMs: Long
        ) {
            val request = OneTimeWorkRequestBuilder<HeartbeatWorker>()
                .setInputData(workDataOf(
                    KEY_TASK_TYPE to TASK_CRON,
                    KEY_JOB_ID to jobId,
                    KEY_PROMPT to prompt
                ))
                .setInitialDelay(delayMs, TimeUnit.MILLISECONDS)
                .build()

            WorkManager.getInstance(context).enqueue(request)
        }

        /** 取消心跳 */
        fun cancelHeartbeat(context: Context) {
            WorkManager.getInstance(context).cancelUniqueWork(WORK_NAME_HEARTBEAT)
        }
    }
}
