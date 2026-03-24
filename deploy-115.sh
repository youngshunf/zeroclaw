#!/bin/bash
# ═══════════════════════════════════════════════════════════════
# deploy-115.sh — ZeroClaw HuanXing 编译部署到 115 服务器
#
# 用法:
#   ./deploy-115.sh              # 编译 + 部署（完整流程）
#   ./deploy-115.sh --build-only # 仅交叉编译，不部署
#   ./deploy-115.sh --deploy-only # 仅部署已编译的二进制
#   ./deploy-115.sh --config-only # 仅同步配置文件
#   ./deploy-115.sh --restart     # 仅重启远程服务
# ═══════════════════════════════════════════════════════════════

set -euo pipefail

# 确保能找到 Homebrew 里的交叉编译工具链 (x86_64-linux-musl-gcc)
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"

# ── 配置 ──────────────────────────────────────────────────────
SERVER_HOST="huanxing-server"              # SSH config 中的别名
SERVER_IP="115.191.47.200"
SSH_USER="root"
SSH_TARGET="${SSH_USER}@${SERVER_IP}"

REMOTE_BIN="/opt/huanxing/bin"
REMOTE_CONFIG="/opt/huanxing/config"
REMOTE_SERVICE="huanxing"

LOCAL_DIR="$(cd "$(dirname "$0")" && pwd)"
TARGET="x86_64-unknown-linux-musl"
BINARY="$LOCAL_DIR/target/${TARGET}/release/zeroclaw"
CONFIG_DIR="$LOCAL_DIR/server-config"

# ── 颜色 ──────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

log()  { echo -e "${BLUE}[$(date +%H:%M:%S)]${NC} $*"; }
ok()   { echo -e "${GREEN}✅ $*${NC}"; }
warn() { echo -e "${YELLOW}⚠️  $*${NC}"; }
err()  { echo -e "${RED}❌ $*${NC}"; exit 1; }

# ── 函数 ──────────────────────────────────────────────────────

do_build() {
    log "交叉编译 zeroclaw (target: ${TARGET}, features: huanxing,channel-lark)..."
    cargo build --release --target "$TARGET" --features huanxing,channel-lark
    local size
    size=$(ls -lh "$BINARY" | awk '{print $5}')
    ok "编译完成: $BINARY ($size)"
}

do_deploy_binary() {
    if [ ! -f "$BINARY" ]; then
        err "二进制文件不存在: $BINARY\n   请先运行: ./deploy-115.sh --build-only"
    fi

    local size
    size=$(ls -lh "$BINARY" | awk '{print $5}')
    log "部署二进制文件 ($size)..."

    # 备份旧版本
    log "  备份旧二进制..."
    ssh "$SERVER_HOST" "
        if [ -f ${REMOTE_BIN}/zeroclaw ]; then
            cp ${REMOTE_BIN}/zeroclaw ${REMOTE_BIN}/zeroclaw.bak.\$(date +%Y%m%d%H%M)
        fi
    "

    # 上传新二进制
    log "  上传新二进制..."
    scp "$BINARY" "${SERVER_HOST}:${REMOTE_BIN}/zeroclaw.new"
    ssh "$SERVER_HOST" "chmod +x ${REMOTE_BIN}/zeroclaw.new"

    # 原子替换 + 重启服务
    log "  停止服务 → 替换二进制 → 启动服务..."
    ssh "$SERVER_HOST" "
        systemctl stop ${REMOTE_SERVICE} 2>/dev/null || true
        sleep 1
        mv ${REMOTE_BIN}/zeroclaw.new ${REMOTE_BIN}/zeroclaw
        systemctl start ${REMOTE_SERVICE}
    "

    # 等待启动并验证
    sleep 3
    local status
    status=$(ssh "$SERVER_HOST" "systemctl is-active ${REMOTE_SERVICE} 2>/dev/null || echo 'failed'")
    if [ "$status" = "active" ]; then
        ok "服务已启动 (${REMOTE_SERVICE}.service)"
    else
        warn "服务状态: $status — 请检查日志: ssh ${SERVER_HOST} 'journalctl -u ${REMOTE_SERVICE} -n 30 --no-pager'"
    fi
}

do_sync_config() {
    log "同步配置文件..."

    # ── 本地路径 → 远程路径映射 ──
    # 部署时自动将本地开发路径替换为服务器路径
    LOCAL_PROJECT_ROOT="/Users/mac/openclaw-workspace/huanxing/huanxing-project"
    REMOTE_ZEROCLAW_CONFIG="/opt/huanxing/config"
    REMOTE_HUB="/opt/huanxing/hub"

    # 同步 config.toml（替换路径后上传）
    if [ -f "$CONFIG_DIR/config.toml" ]; then
        log "  处理 config.toml（替换本地路径 → 远程路径）..."
        local tmp_config
        tmp_config=$(mktemp /tmp/zeroclaw-config.XXXXXX.toml)

        # 复制并替换路径，同时替换 server_id 为 115 服务器的环境标识
        sed \
            -e "s|${LOCAL_PROJECT_ROOT}/huanxing-zeroclaw/server-config|${REMOTE_ZEROCLAW_CONFIG}|g" \
            -e "s|${LOCAL_PROJECT_ROOT}/huanxing-hub/templates|${REMOTE_HUB}/templates|g" \
            -e "s|${LOCAL_PROJECT_ROOT}/huanxing-hub|${REMOTE_HUB}|g" \
            -e 's|server_id = "[^"]*"|server_id = "huanxing-115-test"|g' \
            "$CONFIG_DIR/config.toml" > "$tmp_config"

        # 验证替换结果（不应残留本地路径）
        if grep -q "/Users/mac" "$tmp_config"; then
            warn "  config.toml 中仍有未替换的本地路径:"
            grep "/Users/mac" "$tmp_config" | head -5
            echo ""
            read -p "  是否继续上传？(y/N) " -n 1 -r
            echo ""
            [[ $REPLY =~ ^[Yy]$ ]] || { rm -f "$tmp_config"; err "已取消"; }
        fi

        scp "$tmp_config" "${SERVER_HOST}:${REMOTE_CONFIG}/config.toml"
        rm -f "$tmp_config"
        ok "  config.toml 已更新（路径已替换为远程路径）"
    fi

    # 同步 .env
    if [ -f "$CONFIG_DIR/.env" ]; then
        log "  同步 .env..."
        scp "$CONFIG_DIR/.env" "${SERVER_HOST}:${REMOTE_CONFIG}/.env"
        ok "  .env 已更新"
    fi

    # 同步 guardian 工作区（仅 .md 和 config.toml）
    if [ -d "$CONFIG_DIR/guardian" ]; then
        log "  同步 guardian 配置..."
        local guardian_files=()
        for f in "$CONFIG_DIR/guardian"/*.md "$CONFIG_DIR/guardian"/config.toml; do
            [ -f "$f" ] && guardian_files+=("$f")
        done
        if [ ${#guardian_files[@]} -gt 0 ]; then
            scp "${guardian_files[@]}" "${SERVER_HOST}:${REMOTE_CONFIG}/guardian/"
            ok "  guardian 配置已更新 (${#guardian_files[@]} 个文件)"
        fi
    fi

    # 同步 admin 工作区（仅 .md 和 config.toml）
    if [ -d "$CONFIG_DIR/admin" ]; then
        log "  同步 admin 配置..."
        local admin_files=()
        for f in "$CONFIG_DIR/admin"/*.md "$CONFIG_DIR/admin"/config.toml; do
            [ -f "$f" ] && admin_files+=("$f")
        done
        if [ ${#admin_files[@]} -gt 0 ]; then
            scp "${admin_files[@]}" "${SERVER_HOST}:${REMOTE_CONFIG}/admin/"
            ok "  admin 配置已更新 (${#admin_files[@]} 个文件)"
        fi
    fi

    ok "配置同步完成"
}

do_restart() {
    log "重启远程服务..."
    ssh "$SERVER_HOST" "systemctl restart ${REMOTE_SERVICE}"
    sleep 3
    local status
    status=$(ssh "$SERVER_HOST" "systemctl is-active ${REMOTE_SERVICE}")
    if [ "$status" = "active" ]; then
        ok "服务已重启"
        ssh "$SERVER_HOST" "journalctl -u ${REMOTE_SERVICE} -n 5 --no-pager" 2>/dev/null || true
    else
        warn "服务状态: $status"
        ssh "$SERVER_HOST" "journalctl -u ${REMOTE_SERVICE} -n 15 --no-pager" 2>/dev/null || true
    fi
}

show_status() {
    echo ""
    log "远程服务状态:"
    ssh "$SERVER_HOST" "
        echo '  服务:  '$(systemctl is-active ${REMOTE_SERVICE} 2>/dev/null || echo 'unknown')
        echo '  PID:   '$(pgrep -f 'zeroclaw daemon' || echo 'N/A')
        echo '  版本:  '$(${REMOTE_BIN}/zeroclaw --version 2>/dev/null || echo 'unknown')
        echo '  端口:  42618'
        echo '  二进制: '$(ls -lh ${REMOTE_BIN}/zeroclaw 2>/dev/null | awk '{print \$5, \$6, \$7, \$8}')
    "
}

# ── 主流程 ────────────────────────────────────────────────────

echo ""
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo -e "${BLUE}  ZeroClaw HuanXing → 115 服务器部署${NC}"
echo -e "${BLUE}═══════════════════════════════════════════════${NC}"
echo ""

case "${1:-full}" in
    --build-only)
        do_build
        ;;
    --deploy-only)
        do_deploy_binary
        show_status
        ;;
    --config-only)
        do_sync_config
        echo ""
        read -p "是否重启服务使配置生效？(y/N) " -n 1 -r
        echo ""
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            do_restart
        fi
        ;;
    --restart)
        do_restart
        show_status
        ;;
    full|--full)
        do_build
        echo ""
        do_sync_config
        echo ""
        do_deploy_binary
        echo ""
        show_status
        ;;
    --status)
        show_status
        ;;
    *)
        echo "用法: $0 [选项]"
        echo ""
        echo "选项:"
        echo "  (无参数)       完整流程: 编译 → 同步配置 → 部署"
        echo "  --build-only   仅交叉编译"
        echo "  --deploy-only  仅部署已编译的二进制（停服→替换→启动）"
        echo "  --config-only  仅同步配置文件（可选重启）"
        echo "  --restart      仅重启远程服务"
        echo "  --status       查看远程服务状态"
        echo ""
        ;;
esac

echo ""
echo -e "${GREEN}完成!${NC}"
