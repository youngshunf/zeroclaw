#!/bin/bash

# Web Search 供应商国内可用性测试脚本
# 测试各供应商 API 端点的连通性

echo "═══════════════════════════════════════════════════════════"
echo "  Web Search 供应商国内可用性测试"
echo "  测试时间: $(date '+%Y-%m-%d %H:%M:%S')"
echo "═══════════════════════════════════════════════════════════"
echo ""

# 测试函数
test_endpoint() {
    local name=$1
    local url=$2
    local method=${3:-GET}
    local data=${4:-}

    echo -n "[$name] 测试中... "

    if [ -n "$data" ]; then
        result=$(curl -s -o /dev/null -w "%{http_code}|%{time_total}" \
            -X "$method" "$url" \
            -H "Content-Type: application/json" \
            -d "$data" \
            --max-time 10 2>&1)
    else
        result=$(curl -s -o /dev/null -w "%{http_code}|%{time_total}" \
            "$url" \
            --max-time 10 2>&1)
    fi

    http_code=$(echo "$result" | cut -d'|' -f1)
    time_total=$(echo "$result" | cut -d'|' -f2)

    if [ "$http_code" = "000" ] || [ -z "$http_code" ]; then
        echo "❌ 不可访问（超时或被墙）"
        return 1
    elif [ "$http_code" = "200" ] || [ "$http_code" = "401" ] || [ "$http_code" = "403" ]; then
        echo "✅ 可访问 (HTTP $http_code, ${time_total}s)"
        return 0
    else
        echo "⚠️  可访问但异常 (HTTP $http_code, ${time_total}s)"
        return 2
    fi
}

# 测试计数
total=0
success=0
failed=0

# 1. Tavily
echo "1. Tavily (推荐，AI 优化搜索)"
test_endpoint "Tavily API" \
    "https://api.tavily.com/search" \
    "POST" \
    '{"api_key":"test","query":"test"}'
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 2. Exa
echo "2. Exa (神经网络搜索)"
test_endpoint "Exa API" \
    "https://api.exa.ai/search"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 3. Jina
echo "3. Jina (免费)"
test_endpoint "Jina Search" \
    "https://s.jina.ai/test"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 4. Brave
echo "4. Brave Search (高质量)"
test_endpoint "Brave API" \
    "https://api.search.brave.com/res/v1/web/search?q=test"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 5. DuckDuckGo
echo "5. DuckDuckGo (免费)"
test_endpoint "DuckDuckGo HTML" \
    "https://html.duckduckgo.com/html/?q=test"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 6. Firecrawl
echo "6. Firecrawl (网页抓取)"
test_endpoint "Firecrawl API" \
    "https://api.firecrawl.dev/v1/search"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 7. Perplexity
echo "7. Perplexity (AI 搜索)"
test_endpoint "Perplexity API" \
    "https://api.perplexity.ai/chat/completions"
result=$?
total=$((total + 1))
[ $result -eq 0 ] && success=$((success + 1)) || failed=$((failed + 1))
echo ""

# 汇总
echo "═══════════════════════════════════════════════════════════"
echo "  测试结果汇总"
echo "═══════════════════════════════════════════════════════════"
echo "总计: $total 个供应商"
echo "✅ 可访问: $success 个"
echo "❌ 不可访问: $failed 个"
echo ""

# 推荐配置
echo "═══════════════════════════════════════════════════════════"
echo "  推荐配置"
echo "═══════════════════════════════════════════════════════════"

if [ $success -ge 3 ]; then
    echo "✅ 国内网络环境良好，推荐配置："
    echo ""
    echo "# ~/.zeroclaw/.env"
    echo "WEB_SEARCH_ENABLED=true"
    echo "WEB_SEARCH_PROVIDER=tavily"
    echo "TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx"
    echo ""
    echo "# 备用供应商"
    echo "WEB_SEARCH_FALLBACK_PROVIDERS=exa,jina"
    echo "EXA_API_KEY=exa-xxxxxxxxxxxxxx"
elif [ $success -ge 1 ]; then
    echo "⚠️  部分供应商可用，建议配置代理"
    echo ""
    echo "# config.toml"
    echo "[proxy]"
    echo "enabled = true"
    echo "http_proxy = \"http://127.0.0.1:7890\""
    echo "https_proxy = \"http://127.0.0.1:7890\""
else
    echo "❌ 网络环境受限，必须配置代理"
    echo ""
    echo "请检查："
    echo "1. 网络连接是否正常"
    echo "2. 是否需要配置代理"
    echo "3. 防火墙设置"
fi

echo ""
echo "详细配置指南: ./API_KEYS_GUIDE.md"
echo "═══════════════════════════════════════════════════════════"
