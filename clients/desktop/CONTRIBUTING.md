# 唤星桌面端前端开发规范

> 本文档为唤星 (HuanXing) 桌面端前端开发的权威规范。所有新功能、新页面、新组件的开发必须遵循本文档约定，确保 UI 一致性、代码复用率和可维护性。

---

## 1. 技术栈概览

| 层级 | 技术 | 说明 |
|------|------|------|
| 框架 | React 19 + TypeScript | SPA 架构 |
| 桌面壳 | Tauri 2.x (Rust) | 原生窗口管理 |
| 路由 | react-router-dom 7 | 页面导航 |
| 样式 | CSS Variables + Tailwind 4 | 主题驱动，变量优先 |
| 图标 | lucide-react | 统一图标库 |
| 基础 UI | Radix UI | 无障碍原语组件 |
| 命令面板 | cmdk | 命令面板交互 |
| Markdown | react-markdown + shiki | 内容渲染 |
| 图片浏览 | react-photo-view | 图片查看器 |
| 代码编辑 | @uiw/react-codemirror | 代码编辑面板 |

---

## 2. 目录结构约定

```
clients/desktop/src/
├── components/           # 上游 ZeroClaw 公共组件（不要修改）
│   ├── ui/               # 基础 UI 原语 (Select, ...)
│   ├── chat/             # 上游聊天组件
│   ├── config/           # 配置表单组件
│   └── layout/           # 上游布局组件
├── hooks/                # 公共 hooks（跨模块复用）
├── lib/                  # 公共工具库（ws, api, i18n, session...）
├── huanxing/             # ⭐ 唤星自有代码（零入侵隔离层）
│   ├── components/       # 唤星专属组件
│   │   ├── layout/       # NavRail, App Shell
│   │   ├── chat/         # 聊天输入、消息气泡、图片消息
│   │   ├── sop/          # SOP 运行面板、历史列表
│   │   ├── markdown/     # Markdown 渲染增强
│   │   └── profile/      # 用户资料组件
│   ├── pages/            # 页面级组件
│   ├── lib/              # 唤星专属 API 客户端
│   └── styles/           # huanxing.css 主题系统
```

### 规则

> [!IMPORTANT]
> - `src/components/` 下的文件属于上游 ZeroClaw，**禁止直接修改**，需通过 `huanxing/` 层包装扩展
> - 所有唤星新增代码必须放在 `src/huanxing/` 目录内

---

## 3. 主题系统（最重要）

### 3.1 CSS 变量体系

项目采用 **CSS Variables + `[data-theme]` 属性** 实现双主题。所有颜色必须通过变量引用，**禁止硬编码颜色值**。

```css
/* Light（默认） — 定义在 :root */
--hx-bg-main:      #FFFFFF         /* 主背景 */
--hx-bg-panel:     #FAFAFA         /* 面板/卡片背景 */
--hx-bg-input:     #F9FAFB         /* 输入框/次级容器背景 */
--hx-bg-rail:      #F5F3FF         /* 导航栏背景 */
--hx-bg-hover:     #F3F0FF         /* hover 态 */

--hx-text-primary:   #111827       /* 主文字 */
--hx-text-secondary: #6B7280       /* 辅助文字 */
--hx-text-tertiary:  #9CA3AF       /* 提示文字 */

--hx-border:       #E5E7EB         /* 边框 */
--hx-border-light: #F3F4F6         /* 轻边框 */

--hx-purple:       #7C3AED         /* 品牌主色 */
--hx-purple-hover: #6D28D9         /* 按钮 hover */
--hx-purple-bg:    rgba(124,58,237,0.08) /* 品牌背景 */
--hx-blue:         #6366F1         /* 辅助蓝 */
--hx-green:        #10B981         /* 成功/在线 */
--hx-red:          #EF4444         /* 错误/危险 */
--hx-amber:        #F59E0B         /* 警告 */

--hx-radius-sm:    8px
--hx-radius-md:    12px
--hx-radius-lg:    16px
--hx-radius-full:  9999px

--hx-shadow-sm:    0 1px 2px rgba(0,0,0,0.04)
--hx-shadow-md:    0 4px 12px rgba(0,0,0,0.06)
```

暗色模式通过 `[data-theme="dark"]` 自动覆盖所有变量值。

### 3.2 使用方式

**✅ 正确（使用 CSS 变量）：**

```tsx
// 方式一：inline style（推荐，因为变量值是动态的）
<div style={{ background: 'var(--hx-bg-panel)', color: 'var(--hx-text-primary)', border: '1px solid var(--hx-border)' }}>

// 方式二：在 huanxing.css 中定义 class，用变量
.hx-my-card {
  background: var(--hx-bg-panel);
  border: 1px solid var(--hx-border);
  border-radius: var(--hx-radius-md);
  box-shadow: var(--hx-shadow-sm);
}
```

**❌ 错误（硬编码颜色值，导致主题不跟随）：**

```tsx
// 禁止！只能在一个主题下正常显示
<div className="bg-white text-gray-900 border-gray-200">
<div className="bg-[#050b1a] text-[#a7c4f3]">
<div style={{ background: '#1a1f2e', color: '#f9fafb' }}>
```

### 3.3 主题切换机制

主题状态由 `NavRail.tsx` 管理，存储在 `localStorage('huanxing_theme')`，通过设置 `.hx-app` 元素的 `data-theme` 属性触发。

```tsx
// 如果需要在组件内读取当前主题：
const isDark = document.querySelector('.hx-app')?.getAttribute('data-theme') === 'dark';
```

---

## 4. 组件复用规范

### 4.1 优先级决策树

开发新功能时，按以下顺序选择方案：

```
1. 项目内已有组件？ → 直接用
      ↓ 没有
2. 已安装的依赖库能做？ → 用库组件
      ↓ 不行
3. Radix UI 有对应原语？ → 基于 Radix 封装
      ↓ 没有
4. 社区成熟库？ → 评估后引入
      ↓ 都没有
5. 自行实现（最后手段）
```

### 4.2 项目内已有可复用资源

#### UI 基础组件 (`src/components/ui/`)

| 组件 | 路径 | 说明 |
|------|------|------|
| `Select` | `components/ui/Select.tsx` | 基于 Radix 的下拉选择器，已适配主题 |

#### 已安装的 UI 原语库

| 库 | 用途 | 使用场景 |
|----|------|----------|
| `@radix-ui/react-select` | 下拉选择 | 所有 select / dropdown |
| `@radix-ui/react-popover` | 弹出层 | tooltip, popover, 菜单 |
| `cmdk` | 命令面板 | 斜杠菜单、全局搜索 |
| `lucide-react` | 图标 | **所有图标统一用这个** |
| `react-photo-view` | 图片查看 | 图片灯箱浏览器 |
| `react-easy-crop` | 图片裁剪 | 头像裁剪上传 |
| `@uiw/react-codemirror` | 代码编辑器 | 代码块、配置编辑 |
| `react-markdown` + `remark-gfm` + `rehype-raw` | Markdown 渲染 | 聊天消息、文档展示 |
| `shiki` | 代码高亮 | Markdown 中的代码块 |

#### 公共 Hooks (`src/hooks/`)

| Hook | 用途 |
|------|------|
| `useActiveAgent` | 获取/切换当前活跃 Agent |
| `useApi` | API 请求封装 (loading, error, data) |
| `useAuth` | 认证状态管理 |
| `useWebSocket` | WebSocket 连接管理 |
| `useSSE` | Server-Sent Events 封装 |
| `useDraft` | 草稿状态管理 |

#### 公共工具 (`src/lib/`)

| 模块 | 用途 |
|------|------|
| `ws.ts` | WsMultiplexer 全局 WebSocket 多路复用 |
| `api.ts` | API 基础请求封装 |
| `i18n.ts` | 国际化翻译 |
| `session-manager.ts` | 会话管理 |
| `auth.ts` | 认证工具 |

#### 唤星专属 API 客户端 (`src/huanxing/lib/`)

| 模块 | 用途 |
|------|------|
| `agent-api.ts` | Agent CRUD |
| `sop-api.ts` | SOP 列表、执行、历史 |
| `hasn-api.ts` | HASN 社交网络 API |
| `marketplace-api.ts` | 应用市场 API |
| `huanxing-api.ts` | 唤星平台通用 API |
| `file-upload.ts` | 文件上传 |
| `token-refresh.ts` | Token 刷新 |

### 4.3 新增依赖的审批标准

引入新的 npm 依赖必须满足：

1. **必要性**：已有依赖和自有代码确实无法满足
2. **体积**：tree-shakeable，gzip 后 < 50KB（优先选择小库）
3. **维护度**：近 6 个月有更新，周下载量 > 5K
4. **兼容性**：支持 React 19 + ESM
5. **无样式入侵**：不带全局 CSS Reset 或强制样式

---

## 5. 页面开发模板

```tsx
// src/huanxing/pages/MyNewPage.tsx

import React, { useState, useEffect } from 'react';
import { SomeIcon } from 'lucide-react';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';

export default function MyNewPage() {
  const [activeAgent] = useActiveAgent();

  return (
    <div style={{
      display: 'flex', flexDirection: 'column',
      height: '100%', width: '100%',
      background: 'var(--hx-bg-main)',
      color: 'var(--hx-text-primary)',
    }}>
      {/* 顶栏 */}
      <div style={{
        flexShrink: 0,
        borderBottom: '1px solid var(--hx-border)',
        background: 'var(--hx-bg-panel)',
        padding: '16px 24px',
      }}>
        <h1 style={{ fontSize: 18, fontWeight: 700, margin: 0 }}>页面标题</h1>
      </div>

      {/* 内容区 */}
      <div style={{ flex: 1, overflowY: 'auto', padding: 24 }}>
        {/* 卡片示例 */}
        <div style={{
          background: 'var(--hx-bg-panel)',
          border: '1px solid var(--hx-border)',
          borderRadius: 'var(--hx-radius-md)',
          padding: 16,
          boxShadow: 'var(--hx-shadow-sm)',
        }}>
          <h2 style={{ color: 'var(--hx-text-primary)', marginBottom: 8 }}>卡片标题</h2>
          <p style={{ color: 'var(--hx-text-secondary)', fontSize: 13 }}>描述内容</p>
        </div>
      </div>
    </div>
  );
}
```

---

## 6. 样式编写规范

### 6.1 推荐方式

| 优先级 | 方式 | 适用场景 |
|--------|------|----------|
| ⭐⭐⭐ | `style={{ ... var(--hx-*) }}` | 页面级布局、主题色引用 |
| ⭐⭐ | `.hx-xxx` class in `huanxing.css` | 高频复用样式、动画 |
| ⭐ | Tailwind utilities | 布局辅助（`flex`, `gap-4`, `grid`） |

### 6.2 规则

1. **颜色/背景/边框/阴影** → 必须用 `var(--hx-*)` 变量
2. **布局** (`flex`, `grid`, `gap`, `padding`) → Tailwind class 或 inline style 均可
3. **交互动画** → 在 `huanxing.css` 中定义 `.hx-xxx` class
4. **禁止使用** Tailwind 颜色类（`bg-white`, `text-gray-900`, `border-gray-200`）
5. **圆角** → 用 `var(--hx-radius-sm/md/lg/full)`

### 6.3 CSS class 命名规范

```
.hx-{组件名}                 → .hx-nav-rail
.hx-{组件名}-{子元素}         → .hx-nav-item
.hx-{组件名}-{状态}           → .hx-nav-item.active
```

---

## 7. 图标使用规范

统一使用 `lucide-react`，**禁止**引入其他图标库。

```tsx
import { Play, CheckCircle, AlertTriangle } from 'lucide-react';

// 尺寸统一方式
<Play style={{ width: 16, height: 16, color: 'var(--hx-purple)' }} />

// 或 className（注意不要用颜色 class）
<Play className="w-4 h-4" style={{ color: 'var(--hx-green)' }} />
```

常用图标速查：

| 场景 | 图标 |
|------|------|
| 播放/启动 | `Play` |
| 成功 | `CheckCircle` / `CheckCircle2` |
| 警告 | `AlertTriangle` |
| 错误 | `XCircle` / `AlertCircle` |
| 加载中 | `Loader2` (配合 `animation: hx-spin`) |
| 刷新 | `RefreshCw` |
| 发送 | `Send` |
| 设置 | `Settings` |
| 搜索 | `Search` |
| 历史 | `History` |
| 工作流 | `Workflow` |
| Agent | `Bot` |
| 工具 | `Wrench` |
| 下载 | `Download` |

---

## 8. API 客户端规范

### 8.1 新增 API 模块

所有唤星专属 API 放在 `src/huanxing/lib/` 下：

```ts
// src/huanxing/lib/my-api.ts
import { apiFetch } from './huanxing-api';   // 唤星统一 fetch wrapper

export interface MyResponse { ... }

export async function getMyData(agentName: string): Promise<MyResponse> {
  return apiFetch<MyResponse>(`/api/my-endpoint?agent=${encodeURIComponent(agentName)}`);
}
```

### 8.2 WebSocket 消息

统一通过 `wsMultiplexer` 订阅，**不要**为每个功能创建独立连接。

```ts
import { wsMultiplexer } from '@/lib/ws';

const unsubscribe = wsMultiplexer.subscribe(sessionId, (msg) => { ... });
// 清理
return () => unsubscribe();
```

---

## 9. Checklist：新页面/功能上线前

- [ ] **主题兼容**：Light 和 Dark 模式下均正常显示
- [ ] **变量检查**：无硬编码的颜色值（搜索 `#xxx`, `rgb(`, `bg-white`, `text-gray-` 等排查）
- [ ] **组件复用**：使用了已有的 Select、图标库等，未重复造轮
- [ ] **API 规范**：新 API 调用放在 `huanxing/lib/` 下
- [ ] **零入侵**：未修改 `src/components/` 下的上游文件
- [ ] **TypeScript**：`npm run build` 零新增类型错误
- [ ] **响应式**：窗口缩小时内容不溢出

---

## 10. 反模式清单

| ❌ 不要这样做 | ✅ 应该这样做 |
|--------------|-------------|
| `className="bg-white border-gray-200"` | `style={{ background: 'var(--hx-bg-panel)', border: '1px solid var(--hx-border)' }}` |
| `className="bg-[#050b1a] text-[#a7c4f3]"` | `style={{ background: 'var(--hx-bg-main)', color: 'var(--hx-text-primary)' }}` |
| `import { FaXxx } from 'react-icons/fa'` | `import { Xxx } from 'lucide-react'` |
| 自己写 `<select>` 标签 | `import { Select } from '@/components/ui/Select'` |
| 每个功能新建 WebSocket | `wsMultiplexer.subscribe(sessionId, handler)` |
| 在 `src/components/` 改上游代码 | 在 `src/huanxing/components/` 新建包装 |
| `style={{ color: '#6B7280' }}` | `style={{ color: 'var(--hx-text-secondary)' }}` |
| `npm install some-large-ui-framework` | 先用 Radix 原语 + lucide 组合实现 |

---

## 附录 A：CSS 变量速查表

| 变量 | Light 值 | Dark 值 | 用途 |
|------|----------|---------|------|
| `--hx-bg-main` | `#FFFFFF` | `#1A1F2E` | 页面主背景 |
| `--hx-bg-panel` | `#FAFAFA` | `#111827` | 卡片/面板背景 |
| `--hx-bg-input` | `#F9FAFB` | `#1F2937` | 输入框/嵌套容器背景 |
| `--hx-bg-rail` | `#F5F3FF` | `#0B0F1A` | 导航栏背景 |
| `--hx-bg-hover` | `#F3F0FF` | `rgba(124,58,237,0.08)` | 悬停态背景 |
| `--hx-text-primary` | `#111827` | `#F9FAFB` | 主文字 |
| `--hx-text-secondary` | `#6B7280` | `#9CA3AF` | 辅助文字 |
| `--hx-text-tertiary` | `#9CA3AF` | `#6B7280` | 提示/禁用文字 |
| `--hx-border` | `#E5E7EB` | `#374151` | 边框 |
| `--hx-border-light` | `#F3F4F6` | `#1F2937` | 轻边框 |
| `--hx-purple` | `#7C3AED` | `#7C3AED` | 品牌色/CTA |
| `--hx-purple-bg` | `rgba(124,58,237,0.08)` | `rgba(124,58,237,0.12)` | 品牌淡底 |
| `--hx-shadow-sm` | `0 1px 2px rgba(0,0,0,0.04)` | `0 1px 3px rgba(0,0,0,0.3)` | 轻阴影 |
| `--hx-shadow-md` | `0 4px 12px rgba(0,0,0,0.06)` | `0 4px 12px rgba(0,0,0,0.4)` | 中阴影 |

## 附录 B：已安装依赖清单（禁止重复引入同类库）

| 领域 | 已有库 | 不要再装 |
|------|--------|----------|
| 图标 | `lucide-react` | react-icons, heroicons, fontawesome |
| 下拉选择 | `@radix-ui/react-select` | react-select, headlessui |
| 弹出层 | `@radix-ui/react-popover` | tippy.js, popper.js |
| CSS工具 | `tailwindcss` + `clsx` + `tailwind-merge` | styled-components, emotion |
| 图片查看 | `react-photo-view` | fslightbox, photoswipe |
| 图片裁剪 | `react-easy-crop` | react-avatar-editor, cropperjs |
| Markdown | `react-markdown` + `remark-gfm` | marked, markdown-it |
| 代码高亮 | `shiki` | highlight.js, prism.js |
| 代码编辑 | `@uiw/react-codemirror` | monaco-editor, ace-editor |
| 命令面板 | `cmdk` | 不需要额外库 |
