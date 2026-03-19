#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# 唤星 ZeroClaw Release 构建脚本
#
# 用法:
#   ./scripts/release.sh server     # 云端 sidecar（部署到 115 服务器）
#   ./scripts/release.sh desktop    # 桌面端 Tauri .dmg/.app
#   ./scripts/release.sh sidecar    # 仅编译桌面端 sidecar 二进制
#   ./scripts/release.sh all        # 全部构建
#   ./scripts/release.sh info       # 显示构建信息
#
# 环境变量:
#   PROFILE=release-fast    # 编译 profile（默认 release）
#   SKIP_CLIPPY=1           # 跳过 clippy 检查
#   SKIP_TEST=1             # 跳过测试
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DESKTOP_DIR="$PROJECT_DIR/clients/desktop"
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
PROFILE="${PROFILE:-release}"
BUILD_DATE=$(date +%Y%m%d-%H%M)
ARCH=$(uname -m)

# 唤星 features
SERVER_FEATURES="huanxing,channel-lark"
DESKTOP_FEATURES="huanxing"

# 输出目录
DIST_DIR="$PROJECT_DIR/dist"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}[构建]${NC} $*"; }
ok()    { echo -e "${GREEN}[完成]${NC} $*"; }
warn()  { echo -e "${YELLOW}[警告]${NC} $*"; }
err()   { echo -e "${RED}[错误]${NC} $*"; exit 1; }
step()  { echo -e "\n${BOLD}═══ $* ═══${NC}"; }

# ── 构建信息 ──────────────────────────────────────────────────
show_info() {
    echo -e "${BOLD}唤星 ZeroClaw 构建信息${NC}"
    echo "  版本:     $VERSION"
    echo "  Profile:  $PROFILE"
    echo "  架构:     $ARCH"
    echo "  日期:     $BUILD_DATE"
    echo "  项目目录: $PROJECT_DIR"
    echo ""
    echo "  Server features:  $SERVER_FEATURES"
    echo "  Desktop features: $DESKTOP_FEATURES"
    echo ""
    echo "  Cargo profiles:"
    echo "    release      — opt-level=z, fat LTO, codegen-units=1（最小体积，编译慢）"
    echo "    release-fast — fat LTO, codegen-units=8（较快编译，体积略大）"
    echo "    dist         — 同 release（用于正式发布）"
}

# ── 前置检查 ──────────────────────────────────────────────────
preflight() {
    info "前置检查..."

    command -v cargo >/dev/null 2>&1 || err "未找到 cargo"
    command -v rustc >/dev/null 2>&1 || err "未找到 rustc"

    # 检查 Rust 版本
    local rust_ver
    rust_ver=$(rustc --version | awk '{print $2}')
    info "Rust 版本: $rust_ver"

    # 检查工作区是否干净
    cd "$PROJECT_DIR"
    if [[ -n "$(git status --porcelain -- src/ crates/ clients/desktop/src-tauri/ Cargo.toml Cargo.lock 2>/dev/null)" ]]; then
        warn "工作区有未提交的代码变更"
        git status --short -- src/ crates/ clients/desktop/src-tauri/ Cargo.toml Cargo.lock
        echo ""
        read -p "继续构建？(y/N) " -n 1 -r
        echo
        [[ $REPLY =~ ^[Yy]$ ]] || exit 0
    fi

    ok "前置检查通过"
}

# ── 代码质量检查 ──────────────────────────────────────────────
quality_check() {
    local features="$1"
    cd "$PROJECT_DIR"

    if [[ "${SKIP_CLIPPY:-}" != "1" ]]; then
        info "clippy 检查（features: $features）..."
        cargo clippy --all-targets --features "$features" -- -D warnings 2>&1
        ok "clippy 通过"
    else
        warn "跳过 clippy（SKIP_CLIPPY=1）"
    fi

    if [[ "${SKIP_TEST:-}" != "1" ]]; then
        info "运行测试..."
        cargo test --features "$features" 2>&1
        ok "测试通过"
    else
        warn "跳过测试（SKIP_TEST=1）"
    fi
}

# ── 云端 Server 构建 ─────────────────────────────────────────
build_server() {
    step "构建云端 Server（features: $SERVER_FEATURES）"

    preflight
    quality_check "$SERVER_FEATURES"

    cd "$PROJECT_DIR"
    export ZEROCLAW_BUILD_VERSION="$VERSION-$BUILD_DATE"

    info "编译 zeroclaw server（profile: $PROFILE）..."
    cargo build --profile "$PROFILE" --bin zeroclaw --features "$SERVER_FEATURES" 2>&1

    # 确定输出路径（release-fast 等自定义 profile 输出到同名目录）
    local profile_dir="release"
    [[ "$PROFILE" == "release" || "$PROFILE" == "dist" ]] || profile_dir="$PROFILE"
    local binary="$PROJECT_DIR/target/$profile_dir/zeroclaw"

    [[ -f "$binary" ]] || err "编译产物不存在: $binary"

    # 复制到 dist
    mkdir -p "$DIST_DIR"
    local output="$DIST_DIR/zeroclaw-server-${VERSION}-${ARCH}"
    cp "$binary" "$output"

    local size
    size=$(du -h "$output" | awk '{print $1}')

    ok "Server 构建完成"
    echo "  产物: $output"
    echo "  大小: $size"
    echo ""
    echo "  部署命令:"
    echo "    scp $output root@115.191.47.200:/tmp/zeroclaw"
    echo "    ssh root@115.191.47.200 'mv /tmp/zeroclaw /usr/local/bin/zeroclaw && chmod +x /usr/local/bin/zeroclaw'"
}

# ── 桌面端 Sidecar 构建 ──────────────────────────────────────
build_sidecar() {
    step "构建桌面端 Sidecar（features: $DESKTOP_FEATURES）"

    cd "$PROJECT_DIR"
    export ZEROCLAW_BUILD_VERSION="$VERSION-$BUILD_DATE"

    info "编译 zeroclaw sidecar（profile: $PROFILE）..."
    cargo build --profile "$PROFILE" --bin zeroclaw --features "$DESKTOP_FEATURES" 2>&1

    local profile_dir="release"
    [[ "$PROFILE" == "release" || "$PROFILE" == "dist" ]] || profile_dir="$PROFILE"
    local binary="$PROJECT_DIR/target/$profile_dir/zeroclaw"

    [[ -f "$binary" ]] || err "编译产物不存在: $binary"

    # Tauri sidecar 需要放到 src-tauri/binaries/ 并按平台命名
    local tauri_bin_dir="$DESKTOP_DIR/src-tauri/binaries"
    mkdir -p "$tauri_bin_dir"

    # Tauri sidecar 命名规范: {name}-{target_triple}
    local target_triple
    case "$ARCH" in
        arm64|aarch64) target_triple="aarch64-apple-darwin" ;;
        x86_64)        target_triple="x86_64-apple-darwin" ;;
        *)             target_triple="$ARCH-unknown-$(uname -s | tr '[:upper:]' '[:lower:]')" ;;
    esac

    local sidecar_path="$tauri_bin_dir/zeroclaw-$target_triple"
    cp "$binary" "$sidecar_path"

    local size
    size=$(du -h "$sidecar_path" | awk '{print $1}')

    ok "Sidecar 构建完成"
    echo "  产物: $sidecar_path"
    echo "  大小: $size"
}

# ── 桌面端 Tauri 构建 ────────────────────────────────────────
build_desktop() {
    step "构建桌面端 Tauri 应用"

    # 先构建 sidecar
    build_sidecar

    cd "$DESKTOP_DIR"

    # 前端依赖
    if [[ ! -d "node_modules" ]]; then
        info "安装前端依赖..."
        pnpm install 2>&1
    fi

    # Tauri 构建
    info "构建 Tauri 应用..."
    pnpm tauri build 2>&1

    # 查找产物
    local bundle_dir="$DESKTOP_DIR/src-tauri/target/release/bundle"
    ok "桌面端构建完成"
    echo "  产物目录: $bundle_dir"

    if [[ -d "$bundle_dir/dmg" ]]; then
        echo "  DMG:"
        ls -lh "$bundle_dir/dmg/"*.dmg 2>/dev/null | awk '{print "    " $NF " (" $5 ")"}'
    fi
    if [[ -d "$bundle_dir/macos" ]]; then
        echo "  APP:"
        ls -d "$bundle_dir/macos/"*.app 2>/dev/null | while read -r app; do
            local app_size
            app_size=$(du -sh "$app" | awk '{print $1}')
            echo "    $app ($app_size)"
        done
    fi

    # 复制 DMG 到 dist
    mkdir -p "$DIST_DIR"
    if ls "$bundle_dir/dmg/"*.dmg 1>/dev/null 2>&1; then
        cp "$bundle_dir/dmg/"*.dmg "$DIST_DIR/"
        ok "DMG 已复制到 $DIST_DIR/"
    fi
}

# ── 全部构建 ──────────────────────────────────────────────────
build_all() {
    step "唤星全量 Release 构建"
    echo "  版本: $VERSION"
    echo "  Profile: $PROFILE"
    echo ""

    preflight
    quality_check "$SERVER_FEATURES"

    build_server
    build_desktop

    step "构建汇总"
    echo ""
    ls -lh "$DIST_DIR/" 2>/dev/null | tail -n +2 | awk '{print "  " $NF " (" $5 ")"}'
    echo ""
    ok "全部构建完成"
}

# ── 主入口 ────────────────────────────────────────────────────
case "${1:-help}" in
    server|s)
        build_server
        ;;
    desktop|d)
        preflight
        quality_check "$DESKTOP_FEATURES"
        build_desktop
        ;;
    sidecar|sc)
        preflight
        build_sidecar
        ;;
    all|a)
        build_all
        ;;
    info|i)
        show_info
        ;;
    *)
        echo "唤星 ZeroClaw Release 构建脚本 v$VERSION"
        echo ""
        echo "用法: $0 {server|desktop|sidecar|all|info}"
        echo ""
        echo "  server   (s)   云端 sidecar — 部署到 115 服务器"
        echo "  desktop  (d)   桌面端 Tauri .dmg/.app"
        echo "  sidecar  (sc)  仅编译桌面端 sidecar 二进制"
        echo "  all      (a)   全部构建"
        echo "  info     (i)   显示构建信息"
        echo ""
        echo "环境变量:"
        echo "  PROFILE=release-fast  使用快速编译 profile"
        echo "  SKIP_CLIPPY=1         跳过 clippy 检查"
        echo "  SKIP_TEST=1           跳过测试"
        echo ""
        echo "示例:"
        echo "  ./scripts/release.sh server                    # 标准 release 构建"
        echo "  PROFILE=release-fast ./scripts/release.sh all  # 快速构建全部"
        echo "  SKIP_TEST=1 ./scripts/release.sh desktop       # 跳过测试构建桌面端"
        ;;
esac
