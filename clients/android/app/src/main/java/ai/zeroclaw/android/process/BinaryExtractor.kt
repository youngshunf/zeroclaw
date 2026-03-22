package ai.zeroclaw.android.process

import android.content.Context
import android.os.Build
import android.util.Log
import ai.zeroclaw.android.BuildConfig
import java.io.File

/**
 * 从 assets 解压 zeroclaw binary 到 filesDir/bin/。
 * 版本检查避免重复解压。
 */
class BinaryExtractor(private val context: Context) {

    companion object {
        private const val TAG = "BinaryExtractor"
        private const val BIN_DIR = "bin"
        private const val VERSION_FILE = "version"
    }

    /** 根据 CPU ABI 选择 asset 文件名 */
    private val binaryAssetName: String
        get() = when {
            Build.SUPPORTED_ABIS.contains("arm64-v8a") -> "zeroclaw-arm64"
            Build.SUPPORTED_ABIS.contains("armeabi-v7a") -> "zeroclaw-arm32"
            Build.SUPPORTED_ABIS.contains("x86_64") -> "zeroclaw-x86_64"
            else -> "zeroclaw-arm64"
        }

    /**
     * 解压 binary（首次运行或版本升级时）。
     * @return binary 文件的 File 对象
     */
    fun extractIfNeeded(): File {
        val binDir = File(context.filesDir, BIN_DIR)
        val binFile = File(binDir, "zeroclaw")
        val versionFile = File(binDir, VERSION_FILE)
        val currentVersion = BuildConfig.VERSION_CODE.toString()

        if (binFile.exists() && versionFile.exists()
            && versionFile.readText().trim() == currentVersion
        ) {
            return binFile
        }

        binDir.mkdirs()
        context.assets.open(binaryAssetName).use { input ->
            binFile.outputStream().use { output -> input.copyTo(output) }
        }
        binFile.setExecutable(true, false)
        versionFile.writeText(currentVersion)

        Log.i(TAG, "Binary extracted: ${binFile.absolutePath} (v$currentVersion)")
        return binFile
    }

    /** binary 文件路径（不触发解压） */
    fun binaryPath(): File = File(context.filesDir, "$BIN_DIR/zeroclaw")
}
