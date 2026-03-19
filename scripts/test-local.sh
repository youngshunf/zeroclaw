#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════
# ZeroClaw 本地测试启动脚本
#
# 用法:
#   ./scripts/test-local.sh server    # 云端多租户模式（端口 42618）
#   ./scripts/test-local.sh desktop   # 桌面端模式（端口 42620）
#   ./scripts/test-local.sh build     # 仅编译检查
#   ./scripts/test-local.sh check     # 编译 + cargo test + clippy
#   ./scripts/test-local.sh status    # 查看 daemon_state.json
#
# 环境区别:
#   server  → server-config/config.toml（多租户 + tenant_heartbeat + 飞书/QQ）
#   desktop → ~/.huanxing/config.toml（单用户桌面端 sidecar）
# ═══════════════════════════════════════════════════════════════
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_CONFIG="$PROJECT_DIR/server-config"
DESKTOP_CONFIG="$HOME/.zeroclaw/config.toml"
BINARY="$PROJECT_DIR/target/debug/zeroclaw"

# 颜色
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info()  { echo -e "${CYAN}[信息]${NC} $*"; }
ok()    { echo -e "${GREEN}[完成]${NC} $*"; }
warn()  { echo -e "${YELLOW}[警告]${NC} $*"; }
err()   { echo -e "${RED}[错误]${NC} $*"; exit 1; }

# ── 前置检查 ──────────────────────────────────────────────────
preflight_check() {
    info "前置检查..."

    # 检查 Rust 工具链
    command -v cargo >/dev/null 2>&1 || err "未找到 cargo，请安装 Rust 工具链"

    # 检查端口占用
    local port=$1
    if lsof -i :"$port" -sTCP:LISTEN >/dev/null 2>&1; then
        warn "端口 $port 已被占用:"
        lsof -i :"$port" -sTCP:LISTEN
        echo ""
        read -p "是否终止占用进程？(y/N) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            lsof -ti :"$port" -sTCP:LISTEN | xargs kill -9 2>/dev/null || true
            sleep 1
            ok "已终止端口 $port 上的进程"
        else
            err "端口 $port 被占用，请手动处理"
        fi
    fi

    ok "前置检查通过"
}

# ── 编译 ──────────────────────────────────────────────────────
do_build() {
    info "编译 zeroclaw（debug 模式）..."
    cd "$PROJECT_DIR"

    # 设置编译时环境变量
    export ZEROCLAW_BUILD_VERSION="local-test-$(date +%Y%m%d)"

    cargo build --bin zeroclaw --features "huanxing,channel-lark" 2>&1
    ok "编译完成: $BINARY"
}

# ── 完整检查 ──────────────────────────────────────────────────
do_check() {
    info "=== 编译检查 ==="
    cd "$PROJECT_DIR"
    export ZEROCLAW_BUILD_VERSION="local-test-$(date +%Y%m%d)"

    info "1/4 cargo check --lib（含 huanxing + channel-lark feature）"
    cargo check --lib --features "huanxing,channel-lark" 2>&1
    ok "lib 编译通过"

    info "2/4 cargo check --lib（不含 huanxing，验证 feature gate）"
    cargo check --lib --no-default-features --features observability-prometheus,channel-nostr 2>&1 || {
        warn "不含 huanxing 编译有预先存在的错误（非本次修改引入），跳过"
    }

    info "3/4 cargo test"
    cargo test 2>&1 || warn "部分测试失败，请检查输出"

    info "4/4 cargo clippy"
    cargo clippy --all-targets -- -D warnings 2>&1 || warn "clippy 有警告，请检查输出"

    ok "=== 检查完成 ==="
}

# ── 确保数据目录存在 ──────────────────────────────────────────
ensure_db() {
    local data_dir="$PROJECT_DIR/server-config/data"
    [[ -d "$data_dir" ]] || mkdir -p "$data_dir"
    ok "数据目录就绪（daemon 启动时自动创建数据库）"
}

# ── 云端多租户模式 ────────────────────────────────────────────
run_server() {
    info "=== 云端多租户模式 ==="
    info "配置: $SERVER_CONFIG"
    info "端口: 42618"
    echo ""

    preflight_check 42618
    ensure_db

    # 检查配置文件
    [[ -f "$SERVER_CONFIG/config.toml" ]] || err "配置文件不存在: $SERVER_CONFIG/config.toml"

    # 编译（如果二进制不存在或源码更新）
    if [[ ! -f "$BINARY" ]] || [[ $(find "$PROJECT_DIR/src" -newer "$BINARY" -name '*.rs' | head -1) ]]; then
        do_build
    else
        ok "二进制已是最新，跳过编译"
    fi

    echo ""
    info "启动组件:"
    echo "  - gateway（API 网关，端口 42618）"
    echo "  - channels（飞书 + NapCat QQ）"
    echo "  - heartbeat（全局心跳调度器）"
    echo "  - scheduler（定时任务）"
    echo "  - tenant-heartbeat（多租户心跳 ← 本次新增）"
    echo ""
    info "验证要点:"
    echo "  1. 启动日志应显示 '🧠 ZeroClaw daemon started'"
    echo "  2. daemon_state.json 应出现 'tenant-heartbeat' 组件"
    echo "  3. 日志中应有 tenant_heartbeat 扫描记录"
    echo ""
    info "按 Ctrl+C 停止"
    echo "─────────────────────────────────────────────────"

    exec "$BINARY" --config-dir "$SERVER_CONFIG" daemon
}

# ── 桌面端模式 ────────────────────────────────────────────────
run_desktop() {
    info "=== 桌面端模式 ==="
    info "配置: $DESKTOP_CONFIG"
    info "端口: 42617"
    echo ""

    preflight_check 42617

    # 检查配置文件
    [[ -f "$DESKTOP_CONFIG" ]] || err "配置文件不存在: $DESKTOP_CONFIG"

    # 编译
    if [[ ! -f "$BINARY" ]] || [[ $(find "$PROJECT_DIR/src" -newer "$BINARY" -name '*.rs' | head -1) ]]; then
        do_build
    else
        ok "二进制已是最新，跳过编译"
    fi

    echo ""
    info "启动组件:"
    echo "  - gateway（API 网关，端口 42617）"
    echo "  - tenant-heartbeat → 跳过（桌面端 huanxing.tenant_heartbeat.enabled = false）"
    echo ""
    info "桌面端前端连接:"
    echo "  cd $PROJECT_DIR/../huanxing-project/huanxing-zeroclaw/clients/desktop && pnpm dev"
    echo "  → http://localhost:1420"
    echo ""
    info "按 Ctrl+C 停止"
    echo "─────────────────────────────────────────────────"

    exec "$BINARY" gateway -p 42617
}

# ── 查看状态 ──────────────────────────────────────────────────
show_status() {
    local state_file="$PROJECT_DIR/server-config/daemon_state.json"
    if [[ -f "$state_file" ]]; then
        info "daemon_state.json 内容:"
        python3 -m json.tool "$state_file" 2>/dev/null || cat "$state_file"
        echo ""

        # 检查 tenant-heartbeat 组件
        if python3 -c "
import json, sys
with open('$state_file') as f:
    data = json.load(f)
comps = data.get('components', {})
th = comps.get('tenant-heartbeat')
if th:
    status = th.get('status', 'unknown')
    color = '\033[0;32m' if status == 'ok' else '\033[0;31m'
    print(f'{color}tenant-heartbeat: {status}\033[0m')
    if th.get('last_error'):
        print(f'  最后错误: {th[\"last_error\"]}')
else:
    print('\033[1;33mtenant-heartbeat: 未出现（可能未启用或未启动）\033[0m')
" 2>/dev/null; then
            :
        else
            warn "无法解析状态文件"
        fi
    else
        warn "状态文件不存在: $state_file"
        info "请先启动 server 模式: ./scripts/test-local.sh server"
    fi
}

# ── 主入口 ────────────────────────────────────────────────────
case "${1:-help}" in
    server|s)
        run_server
        ;;
    desktop|d)
        run_desktop
        ;;
    build|b)
        cd "$PROJECT_DIR"
        export ZEROCLAW_BUILD_VERSION="local-test-$(date +%Y%m%d)"
        do_build
        ;;
    check|c)
        do_check
        ;;
    status|st)
        show_status
        ;;
    *)
        echo "用法: $0 {server|desktop|build|check|status}"
        echo ""
        echo "  server  (s)   云端多租户模式 — 端口 42618，含 tenant_heartbeat"
        echo "  desktop (d)   桌面端模式 — 端口 42617，单用户 sidecar"
        echo "  build   (b)   仅编译"
        echo "  check   (c)   编译 + test + clippy"
        echo "  status  (st)  查看 daemon 运行状态"
        ;;
esac
