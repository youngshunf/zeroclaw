# ZeroClaw Android 编译指南

## 前置条件

| 工具 | 最低版本 | 说明 |
|------|---------|------|
| Rust | 1.87+ | `rustc --version` 检查 |
| Android SDK | - | 默认路径 `~/Library/Android/sdk/` |
| Android NDK | r25+ | 通过 SDK Manager 或命令行安装 |
| cargo-ndk | 3.0+ | 简化 NDK 交叉编译 |

## 一、环境准备（首次）

### 1. 安装 Android NDK

```bash
# 方式一：通过 sdkmanager 命令行安装
~/Library/Android/sdk/cmdline-tools/latest/bin/sdkmanager "ndk;25.2.9519653"

# 方式二：通过 Android Studio
# Settings → Languages & Frameworks → Android SDK → SDK Tools → NDK (Side by side) → 勾选安装
```

设置环境变量（加到 `~/.zshrc`）：

```bash
export ANDROID_NDK_HOME=$HOME/Library/Android/sdk/ndk/25.2.9519653
```

### 2. 安装 Rust Android 目标

```bash
# arm64（主流 Android 设备）
rustup target add aarch64-linux-android

# 可选：32 位 ARM（老设备）
rustup target add armv7-linux-androideabi

# 可选：x86_64（模拟器）
rustup target add x86_64-linux-android
```

### 3. 安装 cargo-ndk

```bash
cargo install cargo-ndk
```

## 二、编译 ZeroClaw Binary

```bash
# 进入项目根目录
cd /Users/mac/openclaw-workspace/huanxing/huanxing-project/huanxing-zeroclaw

# 编译 arm64 release 版本（推荐）
cargo ndk -t arm64-v8a build --release --bin zeroclaw \
    --no-default-features \
    --features "skill-creation,huanxing"
```

### Features 说明

| Feature | 是否启用 | 原因 |
|---------|---------|------|
| `skill-creation` | ✅ | 自主技能创建 |
| `huanxing` | ✅ | 唤星多租户扩展 |
| `observability-prometheus` | ❌ | 增加体积，移动端不需要 |
| `channel-nostr` | ❌ | 增加体积，移动端不需要 |
| `sandbox-landlock` | ❌ | Linux 内核特性，Android 不支持 |
| `browser-native` | ❌ | 移动端无需浏览器自动化 |

### 编译产物

```
target/aarch64-linux-android/release/zeroclaw    # ~5-8MB
```

## 三、集成到 Android App

```bash
# 创建 assets 目录（如不存在）
mkdir -p clients/android/app/src/main/assets

# 复制 binary 到 assets
cp target/aarch64-linux-android/release/zeroclaw \
   clients/android/app/src/main/assets/zeroclaw-arm64
```

App 启动时 `BinaryExtractor` 会从 assets 解压到 `filesDir/bin/zeroclaw`，由 `ZeroClawProcessManager` 管理进程生命周期。

## 四、编译 Android APK

```bash
cd clients/android

# Debug 包
./gradlew assembleDebug

# 产物路径
# app/build/outputs/apk/debug/app-debug.apk
```

## 五、一键编译脚本

完整流程（编译 binary + 打包 APK）：

```bash
#!/bin/bash
set -e

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
ANDROID_DIR="$ROOT/clients/android"
ASSETS_DIR="$ANDROID_DIR/app/src/main/assets"

echo "==> 编译 zeroclaw for Android arm64..."
cd "$ROOT"
cargo ndk -t arm64-v8a build --release --bin zeroclaw \
    --no-default-features \
    --features "skill-creation,huanxing"

echo "==> 复制 binary 到 assets..."
mkdir -p "$ASSETS_DIR"
cp target/aarch64-linux-android/release/zeroclaw "$ASSETS_DIR/zeroclaw-arm64"

echo "==> binary 大小: $(du -h "$ASSETS_DIR/zeroclaw-arm64" | cut -f1)"

echo "==> 编译 APK..."
cd "$ANDROID_DIR"
./gradlew assembleDebug

echo "==> 完成！APK 路径:"
ls -lh app/build/outputs/apk/debug/app-debug.apk
```

## 六、常见问题

### Q: 编译报 `ring` 相关错误
`ring` 需要 NDK 中的 C 编译器。确认 `ANDROID_NDK_HOME` 设置正确：
```bash
echo $ANDROID_NDK_HOME
ls $ANDROID_NDK_HOME/toolchains/llvm/prebuilt/
```

### Q: 编译很慢
首次 release 编译约 5-10 分钟（`lto = "fat"` + `codegen-units = 1`）。后续增量编译会快很多。如需加速：
```bash
# 使用 release-fast profile（需 16GB+ 内存）
cargo ndk -t arm64-v8a build --profile release-fast --bin zeroclaw \
    --no-default-features \
    --features "skill-creation,huanxing"
```

### Q: 模拟器测试
模拟器通常是 x86_64 架构，需要额外编译：
```bash
cargo ndk -t x86_64 build --release --bin zeroclaw \
    --no-default-features \
    --features "skill-creation,huanxing"

cp target/x86_64-linux-android/release/zeroclaw \
   clients/android/app/src/main/assets/zeroclaw-x86_64
```
同时需要修改 `BinaryExtractor.kt` 支持多架构选择。
