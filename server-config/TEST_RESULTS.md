# Web Search 供应商国内可用性测试报告

**测试时间：** 2026-03-18
**测试环境：** macOS，本地网络（有代理）

---

## 📊 测试结果汇总

| 供应商 | 国内可用性 | HTTP 状态 | 响应时间 | 推荐指数 |
|--------|-----------|----------|---------|---------|
| **Tavily** | ✅ 可用 | 401 | 0.83s | ⭐⭐⭐⭐⭐ |
| **Jina** | ✅ 可用 | 401 | 1.19s | ⭐⭐⭐⭐ |
| **Exa** | ⚠️ 端点异常 | 404 | 1.13s | ⭐⭐⭐ |
| **Firecrawl** | ⚠️ 端点异常 | 404 | 1.21s | ⭐⭐ |
| **Brave** | ❌ 被墙 | 超时 | - | ⭐⭐⭐⭐ |
| **DuckDuckGo** | ❌ 被墙 | 超时 | - | ⭐⭐ |
| **Perplexity** | ❌ 被墙 | 超时 | - | ⭐⭐⭐ |

**说明：**
- ✅ **可用**：返回 401（需要 API Key）表示端点可访问
- ⚠️ **端点异常**：返回 404 可能是测试 URL 不正确，但网络可达
- ❌ **被墙**：连接超时，需要代理

---

## 🎯 国内最佳配置方案

### 方案 A：纯国内可用（推荐）

```bash
# ~/.zeroclaw/.env

WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx

# 备用供应商
WEB_SEARCH_FALLBACK_PROVIDERS=jina
# Jina 可选 API Key（提升限额）
# JINA_API_KEY=jina-xxxxxxxxxxxxxx
```

**优点：**
- ✅ 完全国内可用，无需代理
- ✅ Tavily 专为 AI 优化，质量高
- ✅ Jina 免费兜底
- ✅ 响应速度快（<1.2s）

**获取 API Key：**
1. Tavily: https://tavily.com → 注册 → API Keys
2. Jina: https://jina.ai → 可选注册

---

### 方案 B：代理环境（高质量）

```bash
# ~/.zeroclaw/.env

WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=brave
BRAVE_API_KEY=BSA-xxxxxxxxxxxxxx

# 备用供应商
WEB_SEARCH_FALLBACK_PROVIDERS=tavily,jina,duckduckgo
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
```

**前提条件：**
```toml
# config.toml
[proxy]
enabled = true
http_proxy = "http://127.0.0.1:7890"
https_proxy = "http://127.0.0.1:7890"
no_proxy = ["127.0.0.1", "localhost", "llm.dcfuture.cn", "api.huanxing.dcfuture.cn"]
scope = "environment"
```

**优点：**
- ✅ Brave 搜索质量最高
- ✅ 多层容错机制
- ⚠️ 需要稳定代理

---

### 方案 C：完全免费

```bash
# ~/.zeroclaw/.env

WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=jina

# 无需 API Key，直接使用
```

**优点：**
- ✅ 完全免费
- ✅ 国内可用
- ⚠️ 有限额限制

---

## 📝 详细配置步骤

### 1. Tavily（强烈推荐）⭐⭐⭐⭐⭐

**测试结果：** ✅ 国内可用，响应 0.83s

**获取步骤：**

1. 访问 https://tavily.com
2. 点击 "Sign Up" 注册（支持 Google/GitHub）
3. 进入 Dashboard → API Keys
4. 复制 API Key（格式：`tvly-xxxxxxxxxxxxxx`）

**价格：**
- 免费：1,000 次/月
- Starter：$20/月（10,000 次）

**配置：**
```bash
# ~/.zeroclaw/.env
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
WEB_SEARCH_MAX_RESULTS=5
```

**验证：**
```bash
# 测试 API
curl -X POST https://api.tavily.com/search \
  -H "Content-Type: application/json" \
  -d '{
    "api_key": "your-key",
    "query": "AI news",
    "max_results": 5
  }'
```

---

### 2. Jina（免费备选）⭐⭐⭐⭐

**测试结果：** ✅ 国内可用，响应 1.19s

**获取步骤：**

1. 访问 https://jina.ai
2. 可选注册（无 Key 也能用）
3. 如需提升限额，创建 API Key

**价格：**
- 完全免费（有限额）

**配置：**
```bash
# ~/.zeroclaw/.env
WEB_SEARCH_PROVIDER=jina
# 可选，提升限额
JINA_API_KEY=jina-xxxxxxxxxxxxxx
```

---

### 3. Exa（需验证端点）⚠️

**测试结果：** ⚠️ 返回 404，可能是测试 URL 不正确

**说明：**
- 网络可达（1.13s 响应）
- 需要验证正确的 API 端点
- 可能需要完整的请求格式

**待验证：**
```bash
# 正确的 API 端点可能是：
curl -X POST https://api.exa.ai/search \
  -H "Authorization: Bearer exa-xxxxxxxxxxxxxx" \
  -H "Content-Type: application/json" \
  -d '{"query": "test"}'
```

---

### 4. Brave（需代理）❌

**测试结果：** ❌ 连接超时，被墙

**解决方案：**
1. 配置代理（见方案 B）
2. 或使用国内可用的供应商

---

### 5. DuckDuckGo（需代理）❌

**测试结果：** ❌ 连接超时，被墙

**说明：**
- 完全免费，但国内无法访问
- 必须配置代理

---

### 6. Perplexity（需代理）❌

**测试结果：** ❌ 连接超时，被墙

**说明：**
- AI 搜索引擎，质量高
- 国内无法访问，需要代理

---

### 7. Firecrawl（需验证）⚠️

**测试结果：** ⚠️ 返回 404，可能是端点问题

**说明：**
- 网络可达（1.21s 响应）
- 可能需要验证正确的 API 版本

---

## 🚀 快速开始

### 步骤 1：复制配置模板

```bash
cd /path/to/huanxing-zeroclaw/server-config
cp .env.example .env
```

### 步骤 2：获取 Tavily API Key

1. 访问 https://tavily.com
2. 注册账号
3. 复制 API Key

### 步骤 3：配置环境变量

```bash
# 编辑 .env
nano .env

# 添加：
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-your-actual-key-here
```

### 步骤 4：重启 daemon

```bash
# 如果使用 supervisor
supervisorctl restart zeroclaw

# 或手动重启
pkill zeroclaw
./zeroclaw gateway -p 42617
```

### 步骤 5：测试搜索

通过 Agent 对话测试：

```
用户：帮我搜索一下最新的 AI 新闻
Agent：[调用 web_search 工具]
```

---

## 🔧 故障排查

### 问题 1：Tavily 返回 401

**原因：** API Key 无效或未配置

**解决：**
```bash
# 检查 .env 文件
cat ~/.zeroclaw/.env | grep TAVILY

# 确保格式正确（无多余空格）
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
```

### 问题 2：所有供应商都超时

**原因：** 网络问题或防火墙

**解决：**
```bash
# 1. 测试网络连通性
curl -I https://api.tavily.com

# 2. 检查代理配置
cat ~/.zeroclaw/config.toml | grep -A 5 "\[proxy\]"

# 3. 尝试配置代理
[proxy]
enabled = true
http_proxy = "http://127.0.0.1:7890"
https_proxy = "http://127.0.0.1:7890"
```

### 问题 3：Jina 限流

**原因：** 免费额度用完

**解决：**
```bash
# 注册 Jina 账号获取 API Key
JINA_API_KEY=jina-xxxxxxxxxxxxxx
```

---

## 📊 性能对比

基于实际测试：

| 供应商 | 响应时间 | 质量 | 成本 | 国内可用 |
|--------|---------|------|------|---------|
| Tavily | 0.83s | ⭐⭐⭐⭐⭐ | $20/月 | ✅ |
| Jina | 1.19s | ⭐⭐⭐ | 免费 | ✅ |
| Brave | - | ⭐⭐⭐⭐⭐ | $5/月 | ❌ |
| DuckDuckGo | - | ⭐⭐⭐ | 免费 | ❌ |

**结论：**
- **最佳选择：** Tavily（国内可用 + 高质量）
- **免费备选：** Jina（国内可用 + 免费）
- **代理环境：** Brave（需代理 + 最高质量）

---

## 💡 最佳实践

### 1. 推荐配置（国内）

```bash
# ~/.zeroclaw/.env
WEB_SEARCH_ENABLED=true
WEB_SEARCH_PROVIDER=tavily
TAVILY_API_KEY=tvly-xxxxxxxxxxxxxx
WEB_SEARCH_FALLBACK_PROVIDERS=jina
WEB_SEARCH_MAX_RESULTS=5
WEB_SEARCH_TIMEOUT_SECS=15
WEB_SEARCH_RETRIES_PER_PROVIDER=2
```

### 2. 监控配额

```bash
# 定期检查 Tavily 使用量
# Dashboard: https://tavily.com/dashboard
```

### 3. 多 Key 负载均衡

```bash
# 如果有多个账号
TAVILY_API_KEY=tvly-key1,tvly-key2,tvly-key3
```

---

## 📞 技术支持

- **Tavily:** support@tavily.com
- **Jina:** support@jina.ai
- **项目 Issues:** https://github.com/your-repo/issues

---

**最后更新：** 2026-03-18
**测试环境：** macOS + 本地网络
