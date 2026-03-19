# Firecrawl 自托管部署指南

## 📦 快速部署

### 一键部署脚本

```bash
# 1. 下载部署脚本
wget https://raw.githubusercontent.com/your-repo/deploy_firecrawl.sh

# 或从本地复制
scp deploy_firecrawl.sh root@your-server:/root/

# 2. 执行部署
ssh root@your-server
chmod +x deploy_firecrawl.sh
./deploy_firecrawl.sh
```

**部署时间：** 约 5-10 分钟

---

## 🎯 部署脚本功能

### 自动完成的任务

1. ✅ 检查系统环境
2. ✅ 安装 Docker 和 Docker Compose
3. ✅ 创建部署目录 `/opt/firecrawl`
4. ✅ 生成随机 API Key
5. ✅ 创建配置文件（.env, docker-compose.yml）
6. ✅ 启动服务（Redis, Playwright, Firecrawl）
7. ✅ 创建管理脚本（start.sh, stop.sh, logs.sh 等）
8. ✅ 测试服务可用性
9. ✅ 显示配置信息

---

## 📋 系统要求

### 最低配置

- **CPU:** 2 核
- **内存:** 4GB RAM
- **磁盘:** 20GB
- **系统:** Ubuntu 20.04+ / Debian 11+
- **网络:** 需要访问外网（抓取网页）

### 推荐配置

- **CPU:** 4 核
- **内存:** 8GB RAM
- **磁盘:** 50GB SSD
- **系统:** Ubuntu 22.04 LTS
- **网络:** 稳定的外网连接

---

## 🚀 部署步骤详解

### 步骤 1：准备服务器

```bash
# 连接到服务器
ssh root@your-server

# 更新系统
apt-get update && apt-get upgrade -y

# 安装基础工具
apt-get install -y curl wget git jq
```

### 步骤 2：运行部署脚本

```bash
# 下载脚本
cd /root
wget https://your-repo/deploy_firecrawl.sh

# 添加执行权限
chmod +x deploy_firecrawl.sh

# 执行部署
./deploy_firecrawl.sh
```

**脚本输出示例：**

```
═══════════════════════════════════════════════════════════════
  Firecrawl 自托管部署脚本
═══════════════════════════════════════════════════════════════

[INFO] 检查系统环境...
[INFO] 系统: Ubuntu 22.04
[INFO] 安装 Docker...
[INFO] Docker 安装完成: Docker version 24.0.7
[INFO] 创建部署目录: /opt/firecrawl
[INFO] 创建配置文件...
[INFO] API Key: abc123xyz789...
[WARN] 请妥善保管此 API Key！
[INFO] 启动 Firecrawl 服务...
[INFO] 服务已就绪！
[INFO] 部署完成！
```

### 步骤 3：验证部署

```bash
cd /opt/firecrawl

# 查看服务状态
./status.sh

# 测试 API
./test.sh
```

---

## 🔧 配置文件说明

### .env 配置

```bash
# API Key（自动生成）
API_KEY=your-generated-api-key

# 服务端口
PORT=3002

# Redis 配置
REDIS_URL=redis://redis:6379
REDIS_MAX_MEMORY=2gb

# Playwright 配置
PLAYWRIGHT_MICROSERVICE_URL=http://playwright-service:3000
PLAYWRIGHT_TIMEOUT=30000

# 性能配置
NUM_WORKERS=4                    # 工作进程数
MAX_CONCURRENT_REQUESTS=10       # 并发限制

# 代理配置（可选）
# HTTP_PROXY=http://proxy-server:7890
# HTTPS_PROXY=http://proxy-server:7890
```

### docker-compose.yml 架构

```
┌─────────────────────────────────────┐
│         Firecrawl API               │
│         (Port 3002)                 │
└──────────┬──────────────────────────┘
           │
    ┌──────┴──────┐
    │             │
┌───▼────┐   ┌───▼──────────┐
│ Redis  │   │ Playwright   │
│ Cache  │   │ Browser      │
└────────┘   └──────────────┘
```

---

## 📝 管理命令

### 服务管理

```bash
cd /opt/firecrawl

# 启动服务
./start.sh

# 停止服务
./stop.sh

# 重启服务
./restart.sh

# 查看状态
./status.sh

# 查看日志
./logs.sh

# 测试 API
./test.sh
```

### Docker Compose 命令

```bash
cd /opt/firecrawl

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f

# 重启单个服务
docker-compose restart firecrawl

# 查看资源使用
docker stats

# 进入容器
docker-compose exec firecrawl bash
```

---

## 🔌 集成到 ZeroClaw

### 配置 ZeroClaw

```bash
# 在 ZeroClaw 服务器上配置
ssh root@115.191.47.200

# 编辑 .env
nano /root/.zeroclaw/.env

# 添加配置
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=firecrawl
WEB_SEARCH_API_KEY=your-firecrawl-api-key
WEB_SEARCH_API_URL=http://your-firecrawl-server:3002

# 重启 ZeroClaw
supervisorctl restart zeroclaw
```

### 测试集成

```bash
# 通过 ZeroClaw Agent 测试
# 让 Agent 执行搜索任务：
# "帮我搜索一下最新的 AI 新闻"
```

---

## 🌐 配置 Nginx 反向代理（可选）

### 安装 Nginx

```bash
apt-get install -y nginx
```

### 配置反向代理

```bash
# 复制配置文件
cp /opt/firecrawl/nginx.conf /etc/nginx/sites-available/firecrawl

# 修改域名
nano /etc/nginx/sites-available/firecrawl
# 将 firecrawl.yourdomain.com 改为你的域名

# 启用配置
ln -s /etc/nginx/sites-available/firecrawl /etc/nginx/sites-enabled/

# 测试配置
nginx -t

# 重载 Nginx
systemctl reload nginx
```

### 配置 SSL（推荐）

```bash
# 安装 Certbot
apt-get install -y certbot python3-certbot-nginx

# 获取 SSL 证书
certbot --nginx -d firecrawl.yourdomain.com

# 自动续期
certbot renew --dry-run
```

---

## 🔍 监控和维护

### 健康检查

```bash
# 检查服务健康状态
curl http://localhost:3002/health

# 预期输出
{"status":"ok","timestamp":"2026-03-18T10:00:00Z"}
```

### 日志管理

```bash
# 查看实时日志
cd /opt/firecrawl
./logs.sh

# 查看特定服务日志
docker-compose logs firecrawl
docker-compose logs redis
docker-compose logs playwright-service

# 清理旧日志
docker-compose logs --tail=0 -f
```

### 性能监控

```bash
# 查看资源使用
docker stats

# 查看 Redis 状态
docker-compose exec redis redis-cli info stats

# 查看 Firecrawl 指标
curl http://localhost:3002/metrics
```

---

## 🔧 性能优化

### 1. 调整工作进程数

```bash
# 编辑 .env
nano /opt/firecrawl/.env

# 根据 CPU 核心数调整
NUM_WORKERS=8  # 推荐设置为 CPU 核心数

# 重启服务
./restart.sh
```

### 2. 增加 Redis 内存

```bash
# 编辑 .env
nano /opt/firecrawl/.env

# 增加内存限制
REDIS_MAX_MEMORY=4gb

# 重启服务
./restart.sh
```

### 3. 配置代理（访问国外网站）

```bash
# 编辑 .env
nano /opt/firecrawl/.env

# 添加代理配置
HTTP_PROXY=http://proxy-server:7890
HTTPS_PROXY=http://proxy-server:7890
NO_PROXY=localhost,127.0.0.1

# 重启服务
./restart.sh
```

---

## 🐛 故障排查

### 问题 1：服务无法启动

**症状：** `docker-compose up -d` 失败

**解决：**

```bash
# 查看详细日志
docker-compose logs

# 检查端口占用
netstat -tlnp | grep 3002

# 清理并重启
docker-compose down
docker-compose up -d
```

### 问题 2：API 返回 500 错误

**症状：** 搜索请求返回 500 Internal Server Error

**解决：**

```bash
# 查看 Firecrawl 日志
docker-compose logs firecrawl

# 检查 Redis 连接
docker-compose exec redis redis-cli ping

# 检查 Playwright 服务
curl http://localhost:3000/health
```

### 问题 3：搜索超时

**症状：** 搜索请求超时

**解决：**

```bash
# 增加超时时间
nano /opt/firecrawl/.env

# 修改配置
PLAYWRIGHT_TIMEOUT=60000  # 增加到 60 秒

# 重启服务
./restart.sh
```

### 问题 4：内存不足

**症状：** 容器频繁重启，OOM 错误

**解决：**

```bash
# 检查内存使用
docker stats

# 减少工作进程
nano /opt/firecrawl/.env
NUM_WORKERS=2

# 限制 Redis 内存
REDIS_MAX_MEMORY=1gb

# 重启服务
./restart.sh
```

---

## 🔄 更新和备份

### 更新 Firecrawl

```bash
cd /opt/firecrawl

# 拉取最新镜像
docker-compose pull

# 重启服务
docker-compose up -d

# 清理旧镜像
docker image prune -f
```

### 备份配置

```bash
# 备份配置文件
cd /opt/firecrawl
tar -czf firecrawl-backup-$(date +%Y%m%d).tar.gz \
    .env \
    docker-compose.yml \
    api_key.txt

# 下载到本地
scp root@your-server:/opt/firecrawl/firecrawl-backup-*.tar.gz ./
```

### 恢复配置

```bash
# 上传备份文件
scp firecrawl-backup-*.tar.gz root@your-server:/opt/firecrawl/

# 解压恢复
cd /opt/firecrawl
tar -xzf firecrawl-backup-*.tar.gz

# 重启服务
./restart.sh
```

---

## 📊 成本估算

### 服务器成本（月）

| 配置 | 云服务商 | 价格 | 适用场景 |
|------|---------|------|---------|
| 2核4GB | 阿里云 | ¥50-80 | 测试环境 |
| 4核8GB | 腾讯云 | ¥150-200 | 小规模生产 |
| 8核16GB | AWS | $100-150 | 大规模生产 |

### 对比云服务

| 方案 | 月成本 | 请求限制 | 维护成本 |
|------|--------|---------|---------|
| Firecrawl 云服务 | $20-100 | 有限制 | 无 |
| 自托管（2核4GB） | ¥50-80 | 无限制 | 低 |
| 自托管（4核8GB） | ¥150-200 | 无限制 | 低 |

**结论：** 月请求量 > 10,000 次时，自托管更经济。

---

## 🎯 最佳实践

### 1. 安全配置

```bash
# 修改默认端口
nano /opt/firecrawl/.env
PORT=8888

# 配置防火墙
ufw allow 8888/tcp
ufw enable

# 定期更新 API Key
./regenerate_api_key.sh
```

### 2. 监控告警

```bash
# 安装监控工具
apt-get install -y prometheus grafana

# 配置 Prometheus 抓取 Firecrawl 指标
# 配置 Grafana 仪表板
```

### 3. 定期维护

```bash
# 每周清理 Docker
docker system prune -f

# 每月更新镜像
docker-compose pull && docker-compose up -d

# 每季度备份配置
tar -czf backup-$(date +%Y%m%d).tar.gz /opt/firecrawl
```

---

## 📞 技术支持

- **Firecrawl GitHub:** https://github.com/mendableai/firecrawl
- **Docker 文档:** https://docs.docker.com
- **问题反馈:** 提交 Issue 到项目仓库

---

**最后更新：** 2026-03-18
