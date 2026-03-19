# Web Search API Keys 获取指南

## 📋 供应商对比表

| 供应商 | 国内可用 | 价格 | 获取难度 | 推荐指数 | 特点 |
|--------|---------|------|---------|---------|------|
| **Tavily** | ✅ 是 | 免费1K/月 | ⭐ 简单 | ⭐⭐⭐⭐⭐ | AI优化，国内可用 |
| **Exa** | ✅ 是 | 免费1K/月 | ⭐ 简单 | ⭐⭐⭐⭐ | 神经搜索，国内可用 |
| **Jina** | ✅ 是 | 免费 | ⭐ 简单 | ⭐⭐⭐ | 完全免费，国内可用 |
| **Brave** | ⚠️ 需代理 | $5/月 | ⭐⭐ 中等 | ⭐⭐⭐⭐ | 高质量，需代理注册 |
| **DuckDuckGo** | ⚠️ 需代理 | 免费 | ⭐ 简单 | ⭐⭐ | 免费但被墙 |
| **Perplexity** | ⚠️ 需代理 | 按量计费 | ⭐⭐⭐ 困难 | ⭐⭐⭐ | AI搜索，需代理 |
| **Firecrawl** | ⚠️ 需代理 | $20/月 | ⭐⭐ 中等 | ⭐⭐⭐ | 抓取+搜索，需代理 |

---

## 🎯 推荐配置方案

### 方案 A：国内最佳（推荐）

**适用场景：** 国内服务器，无代理或代理不稳定

```bash
# ~/.zeroclaw/.env
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx

# 备用供应商
WEB_SEARCH_FALLBACK_PROVIDERS=exa,jina
EXA_API_KEY=exa-xxxxxxxxxxxxxx
```

**优点：**
- ✅ 完全国内可用，无需代理
- ✅ Tavily 专为 AI 优化，结果质量高
- ✅ 有免费额度，成本低
- ✅ 三层容错（Tavily → Exa → Jina）

### 方案 B：高质量（需代理）

**适用场景：** 有稳定代理，追求最佳搜索质量

```bash
# ~/.zeroclaw/.env
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=brave
BRAVE_API_KEY=BSA-xxxxxxxxxxxxxx

# 备用供应商
WEB_SEARCH_FALLBACK_PROVIDERS=tavily,perplexity,duckduckgo
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
PERPLEXITY_API_KEY=pplx-xxxxxxxxxxxxxx
```

**优点：**
- ✅ Brave 搜索质量最高
- ✅ 多层容错机制
- ⚠️ 需要稳定代理

### 方案 C：完全免费

**适用场景：** 测试环境，预算有限

```bash
# ~/.zeroclaw/.env
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=jina

# 备用供应商
WEB_SEARCH_FALLBACK_PROVIDERS=duckduckgo
```

**优点：**
- ✅ 完全免费
- ✅ Jina 国内可用
- ⚠️ 有限额限制

---

## 📝 详细获取步骤

### 1. Tavily（推荐，国内可用）⭐⭐⭐⭐⭐

**获取步骤：**

1. 访问 https://tavily.com
2. 点击 "Sign Up" 注册账号（支持 Google/GitHub 登录）
3. 登录后进入 Dashboard
4. 点击 "API Keys" 标签
5. 复制 API Key（格式：`tvly-xxxxxxxxxxxxxx`）

**价格：**
- 免费：1,000 次/月
- Starter：$20/月（10,000 次）
- Pro：$100/月（100,000 次）

**国内可用性测试：**
```bash
# 测试 API 是否可访问
curl -X POST https://api.tavily.com/search \
  -H "Content-Type: application/json" \
  -d '{"api_key":"your-key","query":"test"}'
```

**配置：**
```bash
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
```

---

### 2. Exa（国内可用）⭐⭐⭐⭐

**获取步骤：**

1. 访问 https://exa.ai
2. 点击 "Get Started" 注册
3. 进入 Dashboard → API Keys
4. 创建新的 API Key
5. 复制 Key（格式：`exa-xxxxxxxxxxxxxx`）

**价格：**
- 免费：1,000 次/月
- Basic：$20/月（10,000 次）
- Pro：$100/月（100,000 次）

**特点：**
- 神经网络搜索，语义理解强
- 支持关键词和神经搜索模式

**配置：**
```bash
EXA_API_KEY=exa-xxxxxxxxxxxxxx
WEB_SEARCH_EXA_SEARCH_TYPE=neural  # auto/keyword/neural
```

---

### 3. Jina（国内可用，免费）⭐⭐⭐

**获取步骤：**

1. 访问 https://jina.ai
2. 注册账号（可选，无 Key 也能用）
3. 如需提升限额，进入 Dashboard → API Keys
4. 创建 API Key

**价格：**
- 完全免费（有限额）
- API Key 可提升限额

**特点：**
- 无需 API Key 即可使用（有限额）
- 国内访问速度快

**配置：**
```bash
# 可选，提升限额
JINA_API_KEY=jina-xxxxxxxxxxxxxx
```

---

### 4. Brave Search（需代理）⭐⭐⭐⭐

**获取步骤：**

1. 访问 https://brave.com/search/api（需代理）
2. 点击 "Get Started"
3. 注册 Brave 账号
4. 选择订阅计划
5. 进入 Dashboard → API Keys
6. 创建 API Key（格式：`BSA-xxxxxxxxxxxxxx`）

**价格：**
- Free：1,000 次/月（免费）
- Basic：$5/月（10,000 次）
- Pro：$20/月（100,000 次）

**国内可用性：**
- ⚠️ 官网需要代理访问
- ✅ API 端点可能国内可访问（需测试）

**配置：**
```bash
BRAVE_API_KEY=BSA-xxxxxxxxxxxxxx
```

---

### 5. Perplexity（需代理）⭐⭐⭐

**获取步骤：**

1. 访问 https://www.perplexity.ai（需代理）
2. 注册账号
3. 进入 Settings → API
4. 开通 API 访问权限
5. 创建 API Key

**价格：**
- 按使用量计费
- 需要信用卡

**配置：**
```bash
PERPLEXITY_API_KEY=pplx-xxxxxxxxxxxxxx
```

---

### 6. Firecrawl（需代理）⭐⭐⭐

**获取步骤：**

1. 访问 https://firecrawl.dev（需代理）
2. 注册账号
3. 进入 Dashboard → API Keys
4. 创建 API Key

**价格：**
- 免费：500 credits/月
- Starter：$20/月
- Pro：$100/月

**特点：**
- 支持自托管（开源）
- 网页抓取 + 搜索

**配置：**
```bash
WEB_SEARCH_API_KEY=fc-xxxxxxxxxxxxxx
```

---

### 7. DuckDuckGo（免费，需代理）⭐⭐

**获取步骤：**
- 无需注册，直接使用

**国内可用性：**
- ⚠️ 被墙，需要代理

**特点：**
- 完全免费
- 可能被限流

**配置：**
```bash
WEB_SEARCH_PROVIDER=duckduckgo
# 无需 API Key
```

---

## 🧪 国内可用性测试

### 测试脚本

创建测试脚本 `test_search_providers.sh`：

```bash
#!/bin/bash

echo "=== Web Search 供应商国内可用性测试 ==="
echo ""

# 1. Tavily
echo "1. 测试 Tavily..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  -X POST https://api.tavily.com/search \
  -H "Content-Type: application/json" \
  -d '{"api_key":"test","query":"test"}' \
  --max-time 10

# 2. Exa
echo "2. 测试 Exa..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://api.exa.ai/search \
  --max-time 10

# 3. Jina
echo "3. 测试 Jina..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://s.jina.ai/test \
  --max-time 10

# 4. Brave
echo "4. 测试 Brave..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://api.search.brave.com/res/v1/web/search?q=test \
  --max-time 10

# 5. DuckDuckGo
echo "5. 测试 DuckDuckGo..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://html.duckduckgo.com/html/?q=test \
  --max-time 10

# 6. Firecrawl
echo "6. 测试 Firecrawl..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://api.firecrawl.dev/v1/search \
  --max-time 10

# 7. Perplexity
echo "7. 测试 Perplexity..."
curl -s -o /dev/null -w "HTTP %{http_code} - 耗时 %{time_total}s\n" \
  https://api.perplexity.ai/chat/completions \
  --max-time 10

echo ""
echo "=== 测试完成 ==="
echo "✅ HTTP 200/401/403 = 可访问"
echo "❌ HTTP 000/超时 = 不可访问（被墙）"
```

### 运行测试

```bash
chmod +x test_search_providers.sh
./test_search_providers.sh
```

---

## 💡 最佳实践

### 1. 多层容错配置

```bash
# 主供应商
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx

# 备用供应商（按优先级）
WEB_SEARCH_FALLBACK_PROVIDERS=exa,jina,duckduckgo
EXA_API_KEY=exa-xxxxxxxxxxxxxx

# 重试配置
WEB_SEARCH_RETRIES_PER_PROVIDER=2
WEB_SEARCH_RETRY_BACKOFF_MS=250
```

### 2. 负载均衡（多 Key）

```bash
# 多个 Key 轮询，避免单 Key 限流
TAVILY_API_KEY=tvly-key1,tvly-key2,tvly-key3
EXA_API_KEY=exa-key1,exa-key2
```

### 3. 监控配额

定期检查 API 使用量，避免超额：

```bash
# Tavily Dashboard: https://tavily.com/dashboard
# Exa Dashboard: https://exa.ai/dashboard
```

---

## 🔧 故障排查

### 问题 1：API Key 无效

**症状：** `401 Unauthorized` 或 `403 Forbidden`

**解决：**
1. 检查 Key 是否正确复制（无多余空格）
2. 确认 Key 未过期
3. 检查账户是否有余额/配额

### 问题 2：国内无法访问

**症状：** 连接超时或 `000` 错误

**解决：**
1. 确认是否需要代理
2. 检查 proxy 配置是否正确
3. 尝试切换到国内可用的供应商（Tavily/Exa/Jina）

### 问题 3：限流

**症状：** `429 Too Many Requests`

**解决：**
1. 配置多个 API Key 负载均衡
2. 增加重试间隔 `WEB_SEARCH_RETRY_BACKOFF_MS`
3. 升级付费计划

---

## 📞 技术支持

- Tavily: support@tavily.com
- Exa: support@exa.ai
- Jina: support@jina.ai
- Brave: https://brave.com/search/api/support

---

**最后更新：** 2026-03-18
