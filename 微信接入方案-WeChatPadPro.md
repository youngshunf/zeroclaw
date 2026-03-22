# 微信接入方案：WeChatPadPro（iPad协议）

> 调研日期：2026-03-22
> 状态：调研完成，待决策

---

## 一、方案概述

通过 WeChatPadPro（基于微信 iPad 协议）接入个人微信，使唤星AI能够通过微信渠道为用户提供服务。

```
微信用户 ←→ 微信服务器 ←→ WeChatPadPro (iPad协议)
                                  ↕ HTTP API + WebSocket
                           ZeroClaw wechat channel (新开发)
                                  ↕
                              唤星AI (多租户)
```

---

## 二、WeChatPadPro 简介

- **项目地址**：`github.com/WeChatPadPro/WeChatPadPro`（原作者 luolin-ai）
- **Gitee 镜像**：`gitee.com/mirrors/wechatpadpro`
- **原理**：模拟 iPad 客户端登录微信，通过 Pad 协议实现消息收发
- **部署方式**：Docker 部署，依赖 MySQL + Redis
- **接口**：HTTP REST API（含 Swagger UI）+ WebSocket 消息推送
- **已有生态**：LangBot、AstrBot、autMan 等 AI 机器人框架已集成
- **API 文档**：`https://doc.apipost.net/docs/460ada21e884000?locale=zh-cn`

### 功能支持

| 功能 | 支持 |
|------|------|
| 文本消息收发 | ✅ |
| 图片消息收发 | ✅ |
| 语音消息 | ✅ |
| 文件传输 | ✅ |
| 名片发送 | ✅ |
| 朋友圈互动 | ✅ |
| 好友管理 | ✅ |
| 群聊管理 | ✅ |
| 红包/转账 | ✅ |

---

## 三、API 接口模型

### 3.1 认证体系

| 概念 | 说明 |
|------|------|
| **adminKey** | 管理员密钥，启动时在日志中生成，用于管理操作 |
| **token** | 授权码，通过 adminKey 调用接口生成，有效期可设 365 天 |

### 3.2 关键 API

| 功能 | 方式 | 接口 |
|------|------|------|
| 生成授权码 | POST | `/admin/GanAuthKey1` |
| 获取登录二维码 | POST | `/login/GetLoginQrCodeNew` |
| 唤醒登录（免扫码） | POST | `/login/WakeUpLogin` |
| 发送文本消息 | POST | `/msg/SendTextMsg` |
| 发送图片消息 | POST | `/msg/SendImageMsg` |
| 消息事件流 | WebSocket | `ws://host:port/ws` |

### 3.3 消息接收

- 通过 **WebSocket** 长连接接收实时消息推送
- 消息格式为自定义 JSON（非 OneBot 标准）
- 包含发送者 wxid、消息内容、消息类型、群 ID 等信息

### 3.4 消息发送

- 通过 **HTTP POST** 调用对应接口
- 需要携带 token 认证
- 支持文本、图片、文件、语音等多种格式

---

## 四、ZeroClaw 对接方案

### 4.1 与现有 NapCat channel 的相似度

| 对比项 | NapCat (QQ) | WeChatPadPro (微信) |
|--------|------------|---------------------|
| 消息接收 | WebSocket | WebSocket |
| 消息发送 | HTTP POST | HTTP POST |
| 消息格式 | OneBot/CQ码 | 自定义 JSON |
| 私聊/群聊 | ✅ | ✅ |
| 图片消息 | ✅ | ✅ |
| 语音消息 | ✅ | ✅ |
| 代码行数 | ~543 行 Rust | 预估 500-700 行 |

### 4.2 开发内容

1. **新增文件**：`src/channels/wechat_pad.rs`
   - WebSocket 连接管理（断线重连）
   - 消息接收 → 解析为 `ChannelMessage`
   - 回复发送 → HTTP API 调用
   - 图片/语音等富媒体处理

2. **配置结构体**（在 config 中新增）：
   ```toml
   [channels_config.wechat_pad]
   host = "127.0.0.1"
   port = 8849
   admin_key = "your_admin_key"
   token = "your_token"
   wxid = "wxid_xxx"           # 登录的微信号 wxid
   proxy = ""                  # 可选，Socks5 代理
   allowed_users = ["*"]
   ```

3. **注册到 channel 系统**：在 `mod.rs` 中注册新 channel

### 4.3 开发工作量

- **预估**：1-2 天
- **难度**：中等（参照 NapCat 实现，架构相似）
- **主要工作**：理解 WeChatPadPro 的 JSON 消息格式并适配

---

## 五、部署方案（115 服务器）

### 5.1 服务器环境检查

| 检查项 | 状态 | 说明 |
|--------|------|------|
| x86_64 架构 | ✅ | WeChatPadPro 不支持 ARM |
| Docker | ✅ | 已安装 28.2.2 |
| MySQL | ✅ | 需新建容器或复用现有 PostgreSQL（不兼容，需新建 MySQL） |
| Redis | ✅ | 已有 Docker Redis（端口 9396），可复用或新建 |
| 磁盘空间 | ⚠️ | 剩余 ~19GB，够用但偏紧 |
| 同城代理 | ❌ | 需额外配置（见风控章节） |

### 5.2 Docker 部署命令

```bash
# 克隆项目
cd /opt/huanxing
git clone https://github.com/WeChatPadPro/WeChatPadPro.git wechat-pad
cd wechat-pad/deploy

# 修改 .env 中的密码（重要！）
# ADMIN_KEY=自定义强密码
# REDIS_PASS=自定义密码
# MYSQL 密码等

# 启动
docker compose up -d

# 查看日志，获取 adminKey
docker logs wechatpadpro
```

### 5.3 登录流程

1. 访问 Swagger UI：`http://115.191.47.200:1239`
2. 填入 adminKey
3. 调用 `/admin/GanAuthKey1` 生成 token（设 365 天）
4. 调用 `/login/GetLoginQrCodeNew` 获取二维码（填入 Socks5 代理）
5. 手机微信扫码确认
6. 确认手机上显示 iPad 在线

---

## 六、⚠️ 风险评估

### 6.1 风险清单

| 风险 | 等级 | 说明 |
|------|------|------|
| **封号风险** | 🔴 高 | iPad 协议非官方，微信可能检测并封号 |
| **异地登录** | 🔴 高 | 服务器和手机不同城市会触发风控 |
| **新号风险** | 🔴 高 | 新注册微信号直接上机器人，极易被封 |
| **频率限制** | 🟡 中 | 消息过于频繁会触发风控 |
| **协议更新** | 🟡 中 | 微信更新可能导致协议失效 |
| **项目持续性** | 🟡 中 | 前身 Gewechat 已停维，当前项目活跃但无保证 |
| **法律风险** | 🟡 中 | 逆向协议可能涉及法律问题 |
| **不支持 ARM** | 🟢 低 | 115 服务器是 x86，无影响 |

### 6.2 风控缓解措施

1. **用小号测试**：绝对不要用主力微信号
2. **养号策略**：
   - 前 3 天仅保持在线，不发消息
   - 第 4-7 天逐步启用基础功能
   - 7 天后开放全功能
3. **同城代理**：必须配置与手机同城的 Socks5 代理
4. **频率控制**：限制消息发送频率（建议每分钟不超过 5 条）
5. **唤醒登录**：掉线后用"唤醒登录"而非重新扫码，避免"新设备"标记
6. **保持手机在线**：微信要求至少一台手机在线

---

## 七、与其他方案对比

| 方案 | 难度 | 稳定性 | 封号风险 | 功能 | 适用场景 |
|------|------|--------|----------|------|----------|
| **WeChatPadPro (iPad协议)** | 中 | 中 | 高 | 全功能 | 个人号自动化 |
| 企业微信 API | 低 | 高 | 无 | 受限 | 企业内部/客服 |
| 微信公众号 | 低 | 高 | 无 | 受限 | 公众号场景 |
| 微信小程序 | 中 | 高 | 无 | 受限 | 小程序内嵌 |
| WeChat Hook (PC) | 高 | 低 | 极高 | 全功能 | 不推荐 |

---

## 八、实施路线

### 阶段一：验证（1 天）
- [ ] 在 115 服务器部署 WeChatPadPro Docker
- [ ] 用测试微信号登录
- [ ] 手动测试消息收发
- [ ] 确认 WebSocket 消息格式

### 阶段二：开发（1-2 天）
- [ ] 开发 `wechat_pad.rs` channel
- [ ] 实现文本消息收发
- [ ] 实现图片消息
- [ ] 本地测试通过

### 阶段三：集成（0.5 天）
- [ ] 配置到唤星 config.toml
- [ ] 部署到 115 服务器
- [ ] 端到端测试
- [ ] 风控策略配置

### 阶段四：上线
- [ ] 小范围内测
- [ ] 监控封号风险
- [ ] 调优消息频率

---

## 九、结论

**技术上完全可行**，WeChatPadPro 的 HTTP + WebSocket 架构与现有 NapCat channel 高度相似，开发工作量约 1-2 天。

**核心风险是封号**——iPad 协议是非官方逆向，微信有权随时封禁。建议：
1. 用专门的小号
2. 配置同城代理
3. 做好被封的心理准备
4. 如果是商业用途，优先考虑企业微信 API
