#!/bin/bash

# Firecrawl 卸载脚本

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

DEPLOY_DIR="/opt/firecrawl"

echo "═══════════════════════════════════════════════════════════════"
echo "  Firecrawl 卸载脚本"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# 确认卸载
read -p "确定要卸载 Firecrawl 吗？此操作不可恢复！(yes/no): " confirm
if [ "$confirm" != "yes" ]; then
    log_info "取消卸载"
    exit 0
fi

# 停止服务
if [ -d "$DEPLOY_DIR" ]; then
    log_info "停止服务..."
    cd $DEPLOY_DIR
    docker-compose down -v 2>/dev/null || true
fi

# 删除容器和镜像
log_info "删除 Docker 资源..."
docker rm -f firecrawl-api firecrawl-redis firecrawl-playwright 2>/dev/null || true
docker rmi mendableai/firecrawl:latest 2>/dev/null || true
docker volume rm firecrawl_redis-data firecrawl_firecrawl-logs 2>/dev/null || true

# 删除部署目录
log_info "删除部署目录..."
rm -rf $DEPLOY_DIR

# 删除 Nginx 配置
if [ -f /etc/nginx/sites-enabled/firecrawl ]; then
    log_info "删除 Nginx 配置..."
    rm -f /etc/nginx/sites-enabled/firecrawl
    rm -f /etc/nginx/sites-available/firecrawl
    systemctl reload nginx 2>/dev/null || true
fi

log_info "卸载完成！"
