#!/bin/bash

# ═══════════════════════════════════════════════════════════════
# Firecrawl 自托管部署脚本
# 适用于：Ubuntu 20.04+ / Debian 11+
# 用途：一键部署 Firecrawl 搜索服务
# ═══════════════════════════════════════════════════════════════

set -e  # 遇到错误立即退出

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查是否为 root
check_root() {
    if [ "$EUID" -ne 0 ]; then
        log_error "请使用 root 用户运行此脚本"
        exit 1
    fi
}

# 检查系统
check_system() {
    log_info "检查系统环境..."

    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS=$NAME
        VER=$VERSION_ID
        log_info "系统: $OS $VER"
    else
        log_error "无法识别的操作系统"
        exit 1
    fi
}

# 安装 Docker
install_docker() {
    if command -v docker &> /dev/null; then
        log_info "Docker 已安装: $(docker --version)"
        return
    fi

    log_info "安装 Docker..."

    # 更新包索引
    apt-get update

    # 安装依赖
    apt-get install -y \
        ca-certificates \
        curl \
        gnupg \
        lsb-release

    # 添加 Docker 官方 GPG key
    mkdir -p /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg | gpg --dearmor -o /etc/apt/keyrings/docker.gpg

    # 设置仓库
    echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] https://download.docker.com/linux/ubuntu \
      $(lsb_release -cs) stable" | tee /etc/apt/sources.list.d/docker.list > /dev/null

    # 安装 Docker Engine
    apt-get update
    apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

    # 启动 Docker
    systemctl start docker
    systemctl enable docker

    log_info "Docker 安装完成: $(docker --version)"
}

# 安装 Docker Compose
install_docker_compose() {
    if command -v docker-compose &> /dev/null; then
        log_info "Docker Compose 已安装: $(docker-compose --version)"
        return
    fi

    log_info "安装 Docker Compose..."

    # 下载最新版本
    COMPOSE_VERSION=$(curl -s https://api.github.com/repos/docker/compose/releases/latest | grep 'tag_name' | cut -d\" -f4)
    curl -L "https://github.com/docker/compose/releases/download/${COMPOSE_VERSION}/docker-compose-$(uname -s)-$(uname -m)" -o /usr/local/bin/docker-compose

    # 添加执行权限
    chmod +x /usr/local/bin/docker-compose

    log_info "Docker Compose 安装完成: $(docker-compose --version)"
}

# 创建部署目录
create_deploy_dir() {
    DEPLOY_DIR="/opt/firecrawl"

    log_info "创建部署目录: $DEPLOY_DIR"
    mkdir -p $DEPLOY_DIR
    cd $DEPLOY_DIR
}

# 生成随机 API Key
generate_api_key() {
    openssl rand -base64 32 | tr -d "=+/" | cut -c1-32
}

# 创建配置文件
create_config() {
    log_info "创建配置文件..."

    # 生成 API Key
    API_KEY=$(generate_api_key)

    # 创建 .env 文件
    cat > .env << EOF
# ═══════════════════════════════════════════════════════════════
# Firecrawl 自托管配置
# 生成时间: $(date '+%Y-%m-%d %H:%M:%S')
# ═══════════════════════════════════════════════════════════════

# ── API 配置 ──────────────────────────────────────────────────
# API Key（用于认证）
API_KEY=${API_KEY}

# 服务端口
PORT=3002

# ── Redis 配置 ──────────────────────────────────────────────────
REDIS_URL=redis://redis:6379
REDIS_MAX_MEMORY=2gb

# ── Playwright 配置 ────────────────────────────────────────────
PLAYWRIGHT_MICROSERVICE_URL=http://playwright-service:3000
PLAYWRIGHT_TIMEOUT=30000

# ── 性能配置 ────────────────────────────────────────────────────
# 工作进程数（根据 CPU 核心数调整）
NUM_WORKERS=4

# 并发限制
MAX_CONCURRENT_REQUESTS=10

# ── 代理配置（可选）────────────────────────────────────────────
# 如果需要访问国外网站，配置代理
# HTTP_PROXY=http://proxy-server:7890
# HTTPS_PROXY=http://proxy-server:7890
# NO_PROXY=localhost,127.0.0.1

# ── 日志配置 ────────────────────────────────────────────────────
LOG_LEVEL=info

# ═══════════════════════════════════════════════════════════════
EOF

    log_info "API Key: ${GREEN}${API_KEY}${NC}"
    log_warn "请妥善保管此 API Key！"

    # 保存 API Key 到单独文件
    echo "$API_KEY" > api_key.txt
    chmod 600 api_key.txt
}

# 创建 Docker Compose 文件
create_docker_compose() {
    log_info "创建 Docker Compose 配置..."

    cat > docker-compose.yml << 'EOF'
version: '3.8'

services:
  # Redis 缓存
  redis:
    image: redis:7-alpine
    container_name: firecrawl-redis
    restart: unless-stopped
    command: redis-server --maxmemory 2gb --maxmemory-policy allkeys-lru
    volumes:
      - redis-data:/data
    networks:
      - firecrawl-network
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 3

  # Playwright 浏览器服务
  playwright-service:
    image: mcr.microsoft.com/playwright:v1.40.0-focal
    container_name: firecrawl-playwright
    restart: unless-stopped
    command: npx playwright run-server --port 3000
    networks:
      - firecrawl-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Firecrawl 主服务
  firecrawl:
    image: mendableai/firecrawl:latest
    container_name: firecrawl-api
    restart: unless-stopped
    ports:
      - "3002:3002"
    env_file:
      - .env
    depends_on:
      redis:
        condition: service_healthy
      playwright-service:
        condition: service_healthy
    networks:
      - firecrawl-network
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3002/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
    volumes:
      - firecrawl-logs:/app/logs

networks:
  firecrawl-network:
    driver: bridge

volumes:
  redis-data:
  firecrawl-logs:
EOF
}

# 创建 Nginx 配置（可选）
create_nginx_config() {
    log_info "创建 Nginx 反向代理配置..."

    cat > nginx.conf << 'EOF'
server {
    listen 80;
    server_name firecrawl.yourdomain.com;  # 修改为你的域名

    # 日志
    access_log /var/log/nginx/firecrawl-access.log;
    error_log /var/log/nginx/firecrawl-error.log;

    # 限流配置
    limit_req_zone $binary_remote_addr zone=firecrawl_limit:10m rate=10r/s;
    limit_req zone=firecrawl_limit burst=20 nodelay;

    location / {
        proxy_pass http://localhost:3002;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;

        # 超时配置
        proxy_connect_timeout 60s;
        proxy_send_timeout 60s;
        proxy_read_timeout 60s;

        # 缓冲配置
        proxy_buffering on;
        proxy_buffer_size 4k;
        proxy_buffers 8 4k;
    }

    # 健康检查端点
    location /health {
        proxy_pass http://localhost:3002/health;
        access_log off;
    }
}
EOF

    log_info "Nginx 配置已创建: nginx.conf"
    log_warn "请手动复制到 /etc/nginx/sites-available/ 并启用"
}

# 创建管理脚本
create_management_scripts() {
    log_info "创建管理脚本..."

    # 启动脚本
    cat > start.sh << 'EOF'
#!/bin/bash
echo "启动 Firecrawl 服务..."
docker-compose up -d
echo "服务已启动！"
docker-compose ps
EOF

    # 停止脚本
    cat > stop.sh << 'EOF'
#!/bin/bash
echo "停止 Firecrawl 服务..."
docker-compose down
echo "服务已停止！"
EOF

    # 重启脚本
    cat > restart.sh << 'EOF'
#!/bin/bash
echo "重启 Firecrawl 服务..."
docker-compose restart
echo "服务已重启！"
docker-compose ps
EOF

    # 查看日志脚本
    cat > logs.sh << 'EOF'
#!/bin/bash
docker-compose logs -f --tail=100
EOF

    # 状态检查脚本
    cat > status.sh << 'EOF'
#!/bin/bash
echo "=== Firecrawl 服务状态 ==="
docker-compose ps
echo ""
echo "=== 健康检查 ==="
curl -s http://localhost:3002/health | jq . || echo "服务未响应"
EOF

    # 测试脚本
    cat > test.sh << 'EOF'
#!/bin/bash
API_KEY=$(cat api_key.txt)
echo "测试 Firecrawl API..."
echo ""
curl -X POST http://localhost:3002/v1/search \
  -H "Authorization: Bearer $API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "query": "AI news",
    "limit": 3
  }' | jq .
EOF

    # 添加执行权限
    chmod +x *.sh

    log_info "管理脚本已创建"
}

# 启动服务
start_services() {
    log_info "启动 Firecrawl 服务..."

    docker-compose up -d

    log_info "等待服务启动..."
    sleep 10

    # 检查服务状态
    docker-compose ps
}

# 测试服务
test_service() {
    log_info "测试服务..."

    API_KEY=$(cat api_key.txt)

    # 等待服务完全启动
    for i in {1..30}; do
        if curl -s http://localhost:3002/health > /dev/null; then
            log_info "服务已就绪！"
            break
        fi
        echo -n "."
        sleep 2
    done
    echo ""

    # 测试搜索 API
    log_info "测试搜索 API..."
    curl -X POST http://localhost:3002/v1/search \
      -H "Authorization: Bearer $API_KEY" \
      -H "Content-Type: application/json" \
      -d '{"query":"test","limit":1}' \
      2>/dev/null | head -n 5
}

# 显示部署信息
show_info() {
    API_KEY=$(cat api_key.txt)

    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Firecrawl 部署完成！"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    echo "📍 部署目录: $DEPLOY_DIR"
    echo "🔑 API Key: $API_KEY"
    echo "🌐 API 地址: http://$(hostname -I | awk '{print $1}'):3002"
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  管理命令"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    echo "启动服务:   ./start.sh"
    echo "停止服务:   ./stop.sh"
    echo "重启服务:   ./restart.sh"
    echo "查看日志:   ./logs.sh"
    echo "查看状态:   ./status.sh"
    echo "测试 API:   ./test.sh"
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo "  ZeroClaw 配置"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    echo "# ~/.zeroclaw/.env"
    echo "WEB_SEARCH_ENABLED=true"
    echo "WEB_SEARCH_PROVIDER=firecrawl"
    echo "WEB_SEARCH_API_KEY=$API_KEY"
    echo "WEB_SEARCH_API_URL=http://$(hostname -I | awk '{print $1}'):3002"
    echo ""
    echo "═══════════════════════════════════════════════════════════════"
    echo ""
    log_warn "请妥善保管 API Key！已保存到: $DEPLOY_DIR/api_key.txt"
    echo ""
}

# 主函数
main() {
    echo "═══════════════════════════════════════════════════════════════"
    echo "  Firecrawl 自托管部署脚本"
    echo "═══════════════════════════════════════════════════════════════"
    echo ""

    check_root
    check_system
    install_docker
    install_docker_compose
    create_deploy_dir
    create_config
    create_docker_compose
    create_nginx_config
    create_management_scripts
    start_services
    test_service
    show_info

    log_info "部署完成！"
}

# 运行主函数
main "$@"
