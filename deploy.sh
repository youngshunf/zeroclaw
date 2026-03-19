#!/bin/bash
# ZeroClaw HuanXing 部署脚本
# 用法: ./deploy.sh [server_ip]

set -e

SERVER="${1:-115.191.47.200}"
SSH_USER="root"
REMOTE_DIR="/opt/huanxing/zeroclaw"
LOCAL_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "=== ZeroClaw HuanXing 部署 ==="
echo "Server: $SERVER"
echo "Local:  $LOCAL_DIR"
echo ""

# 1. 检查 release binary
BINARY="$LOCAL_DIR/target/release/zeroclaw"
if [ ! -f "$BINARY" ]; then
    echo "❌ Release binary not found. Run: cargo build --release"
    exit 1
fi
echo "✅ Binary: $(ls -lh "$BINARY" | awk '{print $5}')"

# 2. 创建远程目录结构
echo ""
echo "📁 Creating remote directories..."
ssh "$SSH_USER@$SERVER" "mkdir -p $REMOTE_DIR/{config,templates/{guardian,finance},workspace/{guardian/memory,agents},data}"

# 3. 传输文件
echo "📦 Uploading files..."

# Binary
scp "$BINARY" "$SSH_USER@$SERVER:$REMOTE_DIR/zeroclaw"
ssh "$SSH_USER@$SERVER" "chmod +x $REMOTE_DIR/zeroclaw"

# Config
scp "$LOCAL_DIR/test-config.toml" "$SSH_USER@$SERVER:$REMOTE_DIR/config/config.toml"

# Templates
scp "$LOCAL_DIR/templates/guardian/SOUL.md" "$SSH_USER@$SERVER:$REMOTE_DIR/templates/guardian/SOUL.md"
scp "$LOCAL_DIR/templates/finance/SOUL.md" "$SSH_USER@$SERVER:$REMOTE_DIR/templates/finance/SOUL.md"

# Guardian workspace
scp "$LOCAL_DIR/workspace/guardian/SOUL.md" "$SSH_USER@$SERVER:$REMOTE_DIR/workspace/guardian/SOUL.md"
scp "$LOCAL_DIR/workspace/guardian/USER.md" "$SSH_USER@$SERVER:$REMOTE_DIR/workspace/guardian/USER.md"

# 4. Copy existing DB (if exists)
echo ""
echo "🔗 Linking existing user database..."
ssh "$SSH_USER@$SERVER" "
    if [ -f /opt/huanxing/data/users.db ]; then
        ln -sf /opt/huanxing/data/users.db $REMOTE_DIR/data/users.db
        echo '  → Linked existing users.db'
    else
        echo '  → No existing DB found, will create new one'
    fi
"

# 5. 创建 systemd service
echo ""
echo "🔧 Creating systemd service..."
ssh "$SSH_USER@$SERVER" "cat > /etc/systemd/system/zeroclaw.service << 'EOF'
[Unit]
Description=ZeroClaw HuanXing Multi-Tenant Agent
After=network.target

[Service]
Type=simple
WorkingDirectory=/opt/huanxing/zeroclaw/workspace/guardian
ExecStart=/opt/huanxing/zeroclaw/zeroclaw --config /opt/huanxing/zeroclaw/config/config.toml
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
EOF
systemctl daemon-reload"

echo ""
echo "=== 部署完成 ==="
echo ""
echo "启动: ssh $SSH_USER@$SERVER 'systemctl start zeroclaw'"
echo "状态: ssh $SSH_USER@$SERVER 'systemctl status zeroclaw'"
echo "日志: ssh $SSH_USER@$SERVER 'journalctl -u zeroclaw -f'"
