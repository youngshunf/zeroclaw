#!/bin/bash
# ──────────────────────────────────────────────────────
# 唤星桌面端开发启动脚本
# 
# 用法: ./dev.sh [--skip-build] [--debug]
#
# 步骤:
#   1. 编译 ZeroClaw sidecar (release, huanxing feature)
#   2. 安装前端依赖 (如果 node_modules 不存在)
#   3. 启动 Tauri 开发模式
# ──────────────────────────────────────────────────────

set -e

# 颜色
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
NC='\033[0m'

# 项目根目录
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ZEROCLAW_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DESKTOP_DIR="$SCRIPT_DIR"

echo -e "${CYAN}╔════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║       唤星桌面端 · 开发启动脚本       ║${NC}"
echo -e "${CYAN}╚════════════════════════════════════════╝${NC}"
echo ""

# 解析参数
SKIP_BUILD=false
BUILD_MODE="release"
CARGO_PROFILE="--release"

for arg in "$@"; do
    case $arg in
        --skip-build)
            SKIP_BUILD=true
            ;;
        --debug)
            BUILD_MODE="debug"
            CARGO_PROFILE=""
            ;;
    esac
done

# ─── Step 1: 编译 ZeroClaw Sidecar ───
if [ "$SKIP_BUILD" = true ]; then
    echo -e "${YELLOW}⏭  跳过 Sidecar 编译 (--skip-build)${NC}"
    
    BINARY="$ZEROCLAW_ROOT/target/$BUILD_MODE/zeroclaw"
    if [ ! -f "$BINARY" ]; then
        echo -e "${RED}✘ 未找到 Sidecar 二进制: $BINARY${NC}"
        echo -e "${RED}  请先不带 --skip-build 运行一次${NC}"
        exit 1
    fi
else
    echo -e "${GREEN}🔨 Step 1/3: 编译 ZeroClaw Sidecar ($BUILD_MODE + huanxing)${NC}"
    echo -e "   目录: $ZEROCLAW_ROOT"
    echo ""
    
    cd "$ZEROCLAW_ROOT"
    cargo build $CARGO_PROFILE --features huanxing
    
    BINARY="$ZEROCLAW_ROOT/target/$BUILD_MODE/zeroclaw"
    if [ ! -f "$BINARY" ]; then
        echo -e "${RED}✘ 编译产物不存在: $BINARY${NC}"
        exit 1
    fi
    
    echo ""
    echo -e "${GREEN}✔ Sidecar 编译完成: $(du -h "$BINARY" | cut -f1) ${NC}"
fi

echo ""

# ─── Step 2: 安装前端依赖 ───
echo -e "${GREEN}📦 Step 2/3: 检查前端依赖${NC}"
cd "$DESKTOP_DIR"

if [ ! -d "node_modules" ]; then
    echo "   安装 npm 依赖..."
    npm install
    echo -e "${GREEN}✔ 依赖安装完成${NC}"
else
    echo -e "   ${YELLOW}node_modules 已存在，跳过安装${NC}"
fi

echo ""

# ─── Step 3: 启动 Tauri Dev ───
echo -e "${GREEN}🚀 Step 3/3: 启动 Tauri 开发模式${NC}"
echo -e "   前端: http://localhost:1420"
echo -e "   Sidecar: $BINARY"
echo ""
echo -e "${CYAN}──────────────────────────────────────────${NC}"
echo ""

# 设置 ZEROCLAW_BIN 环境变量指向刚编译的二进制
export ZEROCLAW_BIN="$BINARY"

# 启动 Tauri dev
npx tauri dev
