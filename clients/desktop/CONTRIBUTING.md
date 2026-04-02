# 唤星桌面端前端开发规范

> 本文档为唤星 (HuanXing) 桌面端前端开发的权威规范。所有新功能、新页面、新组件的开发必须遵循本文档约定，确保 UI 一致性、代码复用率和可维护性。

---

## 1. 技术栈概览

| 层级 | 技术 | 说明 |
|------|------|------|
| 框架 | React 19 + TypeScript | SPA 架构 |
| 桌面壳 | Tauri 2.x (Rust) | 原生窗口管理 |
| 路由 | react-router-dom 7 | 页面导航 |
| 样式 | Tailwind CSS v4 + 语义化 @theme | 原子类优先，彻底去除内联样式，天然双主题兼容 |
| 图标 | lucide-react | 统一图标库 |
| 基础 UI | Radix UI | 无障碍原语组件 |
| 命令面板 | cmdk | 命令面板交互 |
| Markdown | react-markdown + shiki | 内容渲染 |
| 图片浏览 | react-photo-view | 图片查看器 |
| 代码编辑 | @uiw/react-codemirror | 代码编辑面板 |

---

## 2. 目录结构约定

> [!NOTE]
> 项目已完成目录结构统一化迁移，原 `src/huanxing/` 子目录已合并至 `src/` 根级。所有代码按功能模块分组，不再区分"上游"与"唤星"物理隔离层。

```
clients/desktop/src/
├── App.tsx               # 应用入口、路由、认证守卫、主题管理
├── main.tsx              # React 挂载点
├── config.ts             # 唤星会话配置管理
├── onboard.ts            # 首次使用引导流程
│
├── assets/               # 静态资源（Logo、图标 SVG/PNG）
│
├── components/           # 公共组件（跨页面复用）
│   ├── ui/               # 基础 UI 原语（Input, Select, Dialog, AlertDialog, Textarea, FolderTreeSelect）
│   ├── billing/          # 订阅/充值组件（CheckoutModal, CreditsTab, SubscriptionTab, UsageStats）
│   ├── chat/             # 聊天组件（SessionList, StreamingBubble, ProgressPanel, HxImage*）
│   │   └── input/        # 聊天输入组件（HxChatInput, HxMentionMenu, HxSlashMenu, HxVoiceButton）
│   ├── config/           # 配置表单组件（ConfigFormEditor, ConfigRawEditor, ConfigSection）
│   │   └── fields/       # 表单字段组件（NumberField, SelectField, TagListField, TextField, ToggleField）
│   ├── effects/          # 视觉特效组件（StarfieldCanvas, SupernovaCanvas, GlowingStar）
│   ├── integrations/     # 集成引导组件（ChatChannelsGuide）
│   ├── layout/           # 布局组件（HuanxingLayout, NavRail, Header, Sidebar, SettingsPanel）
│   ├── markdown/         # Markdown 渲染（Markdown, CodeBlock, CollapsibleSection）
│   ├── mermaid/          # Mermaid 图表渲染（MermaidViewer）
│   ├── onboard/          # 引导进度组件（OnboardProgress）
│   ├── profile/          # 用户资料组件（AvatarCropDialog）
│   └── sop/              # SOP 组件（SopRunPanel, SopHistoryList）
│
├── hooks/                # 公共 Hooks（跨模块复用）
│
├── lib/                  # 公共工具库（API 客户端、WebSocket、i18n、工具函数）
│
├── pages/                # 页面级组件（按功能模块分子目录）
│   ├── agent/            # AI 对话主界面（ChatLayout, AgentChat）
│   ├── agents/           # Agent 管理（AgentManager）
│   ├── auth/             # 登录认证（Login）
│   ├── channels/         # 渠道管理（ChannelsLayout, WeixinAuthModal）
│   ├── contacts/         # 通讯录（Contacts）
│   ├── docs/             # 文档管理（Documents）
│   ├── engine/           # 引擎管理（Engine）
│   ├── hasn/             # HASN 社交（HasnChat）
│   ├── market/           # 应用市场（Marketplace）
│   ├── profile/          # 个人资料（ProfilePage）
│   ├── settings/         # 设置子页（Dashboard, Config, Cost, Cron, Tools, Memory, Logs, Doctor, Devices, Integrations）
│   └── sop/              # SOP 工作台（SopWorkbench, SopEditor）
│
├── stores/               # Zustand 状态管理（useSubscriptionStore）
├── styles/               # 样式文件（huanxing.css, huanxing-theme.css, chatscope-*.css）
│   └── modules/          # 模块化样式（layout.css, chat.css, contacts.css, components.css）
├── test/                 # 测试辅助
└── types/                # TypeScript 类型定义
```

### 规则

> [!IMPORTANT]
> - 所有导入路径必须使用 `@/` 别名（如 `import { Input } from '@/components/ui/Input'`），**禁止使用** `../../` 深层相对路径
> - 新增页面必须放在 `pages/{功能模块}/` 子目录下，不允许直接在 `pages/` 根级创建文件（`ImageViewer.tsx` 为历史遗留例外）
> - 新增公共组件必须放在 `components/{功能分类}/` 下，确保可被多页面复用
> - `lib/` 下的模块按职责单一化原则拆分，每个文件只暴露一个功能域的 API

---

## 3. 主题系统（最重要）

### 3.1 CSS 变量体系与 Tailwind v4 集成

项目采用 **CSS Variables + `[data-theme]` 属性** 实现双主题。所有的语义化颜色必须通过 Tailwind 原子类引用，**绝对禁止硬编码颜色值或使用内联样式（inline style）设置颜色**。

核心颜色体系已经在 `index.css` 中完整映射为 Tailwind CSS v4 兼容变量集合（例如 `--hx-bg-panel` 可以通过 `bg-hx-bg-panel` 调用）：

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

暗色模式通过在根节点打入 `[data-theme="dark"]` 属性，自动覆盖上述所有变量值，让全局组件瞬时热切为夜间模式。

### 3.2 使用方式

**✅ 正确（使用 Tailwind 语义化原子类）：**

```tsx
// 方式一：Tailwind class（极其推荐，已深度集成暗色模式解析）
<div className="bg-hx-bg-panel text-hx-text-primary border border-hx-border rounded-hx-radius-md shadow-hx-shadow-sm">

// 方式二：特定的状态控制（需要时配合 dark: 变体使用，如特定组件在黑夜模式的强化显示）
<div className="text-gray-500 dark:text-gray-400 bg-hx-bg-main hover:bg-hx-bg-hover">
```

**❌ 错误（硬编码颜色值或使用内联样式）：**

```tsx
// 禁止 1：硬编码色值导致无法跟随主题
<div className="bg-white text-gray-900 border-gray-200">
<div className="bg-[#050b1a] text-[#a7c4f3]">

// 禁止 2：滥用内联样式（React Style 计算缓慢且无法响应现代伪类、媒体查询及变体）
<div style={{ background: 'var(--hx-bg-panel)', color: 'var(--hx-text-primary)' }}>
```

### 3.3 主题切换机制

主题状态由 `NavRail.tsx` 管理，存储在 `localStorage('huanxing_theme')`。
我们直接在**DOM 最高层**触发根节点 `<html>` 的状态赋值（为彻底规避 Tailwind v4 `:root` CSS Variable Cascade 下潜继承 Bug）：

```tsx
// 内部机制：直接强加于 root，打通 Tailwind v4 全局组件级联
const root = document.documentElement;  
root.setAttribute('data-theme', isDark ? 'dark' : 'light');
if (isDark) root.classList.add('dark');
else root.classList.remove('dark');
```

此外，在 `index.css` 已经针对旧版原子类配置了定制的 Dark 模式钩子，因此原有的 `dark:text-white`、`dark:bg-slate-800` 等系列样式依旧可以同源流转运作：
```css
@custom-variant dark (&:where(.dark, .dark *, [data-theme="dark"], [data-theme="dark"] *));
```

---

## 4. 组件复用规范（禁止重复造轮子）

> [!CAUTION]
> **不要自己定义 UI 组件！** 这是本项目最重要的工程纪律之一。每一个自定义组件都意味着额外的维护成本、不一致的交互行为和潜在的无障碍缺陷。请严格遵循以下决策树。

### 4.1 优先级决策树

开发新功能时，**必须**按以下顺序逐级排查，只有当上一级确实无法满足需求时才可进入下一级：

```
1. 项目内已有组件？ → 直接使用，禁止二次封装同功能组件
      ↓ 确认没有（在 components/ 全目录搜索过）
2. 已安装的依赖库能做？ → 用库组件 + 唤星主题样式适配
      ↓ 确认不行
3. Radix UI 有对应原语？ → npm install @radix-ui/react-xxx
   → 封装为 components/ui/Xxx.tsx，适配 hx-* 主题
   → 作为公共组件供全项目复用
      ↓ Radix 没有
4. 社区成熟库？ → 评估后引入（须满足 §4.3 审批标准）
      ↓ 都没有
5. 自行实现（最后手段，须在 PR 中说明为何前 4 级均不可行）
```

> [!IMPORTANT]
> **业务组件同理**：在 `components/` 中新增业务组件前，先确认是否已有功能相近的组件可以扩展或复合使用。拒绝为单一页面创建仅用一次的"公共组件"——那应该是页面内的局部组件。

### 4.1.1 Radix UI 封装标准流程

当需要新的基础 UI 原语（如 Tooltip、Switch、Tabs 等）时：

1. **安装**：`npm install @radix-ui/react-xxx`
2. **封装**：在 `src/components/ui/Xxx.tsx` 创建封装组件
3. **样式适配**：使用 `hx-*` 语义化 Tailwind 类适配项目主题，确保 Light/Dark 双模式兼容
4. **导出**：组件必须是通用的，不绑定任何业务逻辑
5. **文档**：在本文件 §4.2 的 UI 基础组件表中登记

```tsx
// 示例：src/components/ui/Tooltip.tsx
import * as TooltipPrimitive from '@radix-ui/react-tooltip';

export function Tooltip({ children, content }: { children: React.ReactNode; content: string }) {
  return (
    <TooltipPrimitive.Provider>
      <TooltipPrimitive.Root>
        <TooltipPrimitive.Trigger asChild>{children}</TooltipPrimitive.Trigger>
        <TooltipPrimitive.Content
          className="bg-hx-bg-panel text-hx-text-primary text-xs px-3 py-1.5 rounded-hx-radius-sm
                     border border-hx-border shadow-hx-shadow-md z-50"
          sideOffset={5}
        >
          {content}
          <TooltipPrimitive.Arrow className="fill-hx-bg-panel" />
        </TooltipPrimitive.Content>
      </TooltipPrimitive.Root>
    </TooltipPrimitive.Provider>
  );
}
```

### 4.2 项目内已有可复用资源

> [!WARNING]
> 在新建任何组件之前，请先通读本节清单。如果你需要的功能已经存在，**直接引用**。

#### UI 基础组件 (`src/components/ui/`)

| 组件 | 路径 | 说明 |
|------|------|------|
| `Input` | `components/ui/Input.tsx` | 文本输入框，已适配 `hx-input` 主题 |
| `Textarea` | `components/ui/Textarea.tsx` | 多行文本域，已适配主题 |
| `Select` | `components/ui/Select.tsx` | 基于 Radix 的下拉选择器，已适配主题 |
| `Dialog` | `components/ui/Dialog.tsx` | 基于 Radix 的模态对话框，已适配主题 |
| `AlertDialog` | `components/ui/AlertDialog.tsx` | 基于 Radix 的确认弹窗（删除/危险操作），已适配主题 |
| `FolderTreeSelect` | `components/ui/FolderTreeSelect.tsx` | 树形文件夹选择器 |

#### 已安装的 UI 原语库

| 库 | 用途 | 使用场景 |
|----|------|----------|
| `@radix-ui/react-select` | 下拉选择 | 所有 select / dropdown |
| `@radix-ui/react-dialog` | 模态对话框 | 表单弹窗、详情面板 |
| `@radix-ui/react-alert-dialog` | 确认弹窗 | 删除确认、危险操作二次确认 |
| `@radix-ui/react-popover` | 弹出层 | tooltip, popover, 菜单 |
| `cmdk` | 命令面板 | 斜杠菜单、全局搜索 |
| `lucide-react` | 图标 | **所有图标统一用这个** |
| `react-photo-view` | 图片查看 | 图片灯箱浏览器 |
| `react-easy-crop` | 图片裁剪 | 头像裁剪上传 |
| `@uiw/react-codemirror` | 代码编辑器 | 代码块、配置编辑 |
| `react-markdown` + `remark-gfm` + `rehype-raw` | Markdown 渲染 | 聊天消息、文档展示 |
| `shiki` | 代码高亮 | Markdown 中的代码块 |
| `@tiptap/react` + `@tiptap/starter-kit` | 富文本编辑 | 文档编辑器 |
| `qrcode.react` | 二维码生成 | 支付/分享二维码 |
| `zustand` | 状态管理 | 全局状态存储 |

#### 公共 Hooks (`src/hooks/`)

| Hook | 用途 |
|------|------|
| `useActiveAgent` | 获取/切换当前活跃 Agent |
| `useAgentSkills` | Agent 技能列表管理 |
| `useApi` | API 请求封装 (loading, error, data) |
| `useAuth` | 认证状态管理 |
| `useWebSocket` | WebSocket 连接管理 |
| `useSSE` | Server-Sent Events 封装 |
| `useDraft` | 草稿状态管理 |
| `useHasn` | HASN 协议交互 |
| `useHasnContacts` | HASN 通讯录管理 |
| `useSidecar` | ZeroClaw Sidecar 进程管理 |
| `useVoiceRecorder` | 语音录制 |

#### 公共工具库 (`src/lib/`)

| 模块 | 用途 |
|------|------|
| `api.ts` | API 基础请求封装（含 Token 注入） |
| `ws.ts` | WsMultiplexer 全局 WebSocket 多路复用 |
| `i18n.ts` | 国际化翻译 |
| `session-manager.ts` / `session-api.ts` | 会话管理 |
| `auth.ts` | 认证工具 |
| `agent-api.ts` | Agent CRUD |
| `sop-api.ts` | SOP 列表、执行、历史 |
| `hasn-api.ts` / `hasn-ws.ts` | HASN 社交网络 API 与 WebSocket |
| `marketplace-api.ts` | 应用市场 API |
| `huanxing-api.ts` | 唤星平台通用 API（登录、验证码等） |
| `subscription-api.ts` | 订阅与计费 API |
| `document-api.ts` | 文档管理 API |
| `file-upload.ts` | 文件上传 |
| `token-refresh.ts` | Token 自动刷新 |
| `audio.ts` | 音频播放工具 |
| `cropImage.ts` | 图片裁剪工具函数 |
| `connection.ts` | 连接状态管理 |
| `utils.ts` | 通用工具函数 |

### 4.3 新增依赖的审批标准

引入新的 npm 依赖必须满足：

1. **必要性**：已有依赖和自有代码确实无法满足
2. **体积**：tree-shakeable，gzip 后 < 50KB（优先选择小库）
3. **维护度**：近 6 个月有更新，周下载量 > 5K
4. **兼容性**：支持 React 19 + ESM
5. **无样式入侵**：不带全局 CSS Reset 或强制样式

### 4.4 Tauri 环境下的路径解析规范

在 Tauri 桌面端运行环境下（`tauri://localhost` 或特定的 tauri custom protocol），前端如果直接渲染后端返回的相对路径图片（如 `<img src="/api/agents/icon.png" />`），WebView 会将其解析为 `tauri://localhost/api/agents/icon.png` 导致资源 404 无法加载。

**✅ 正确（必须使用 `resolveApiUrl` 转换）：**

```tsx
import { resolveApiUrl } from '@/config';

// 组件渲染必须经过绝对路径转换
<img 
  src={resolveApiUrl(agent.icon_url)} 
  alt={agent.name} 
/>
```

**❌ 错误（直接使用相对路径）：**

```tsx
// 仅在浏览器 localhost 代理有效，Tauri 桌面端应用中会产生 404 错误
<img src={agent.icon_url} />
```

---

## 5. 页面开发模板

```tsx
// src/pages/{module}/MyNewPage.tsx
// 例如：src/pages/settings/Billing.tsx

import React, { useState, useEffect } from 'react';
import { SomeIcon } from 'lucide-react';
import { useActiveAgent } from '@/hooks/useActiveAgent';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/Dialog';

export default function MyNewPage() {
  const [activeAgent] = useActiveAgent();

  return (
    <div className="flex flex-col h-full w-full bg-hx-bg-main text-hx-text-primary">
      {/* 顶栏 */}
      <div className="shrink-0 border-b border-hx-border bg-hx-bg-panel px-6 py-4">
        <h1 className="text-lg font-bold m-0">页面标题</h1>
      </div>

      {/* 内容区 */}
      <div className="flex-1 overflow-y-auto p-6">
        {/* 卡片示例 */}
        <div className="bg-hx-bg-panel border border-hx-border rounded-hx-radius-md p-4 shadow-hx-shadow-sm">
          <h2 className="text-hx-text-primary mb-2">卡片标题</h2>
          <p className="text-hx-text-secondary text-[13px]">描述内容</p>
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
| ⭐⭐⭐ | Tailwind 语义化原子类 (`bg-hx-*`, `text-hx-*`) | 所有界面的挂载、基础布局、排版、主题动态响应切换 |
| ⭐⭐ | `dark:` 组合类 | 需要特定在浅/暗模式指定极高反差与定制呈现的特例元素 |
| ⭐ | `.hx-xxx` class in `huanxing.css` | 超高频复用的复杂组合组件（例如聊天气泡）、高级动画及过渡效果 |
| ❌ | `style={{ ... }}` 行内样式 | **仅限于**注入动态 JS 计算属性（如 `width: ${progress}%`），**严禁用于色值、基础样式！** |

### 6.2 规则

1. **颜色/背景/边框/阴影** → 优先使用衍生自 `var(--hx-*)` 的专属 Tailwind class（如 `bg-hx-bg-panel`, `border-hx-border`）
2. **布局** (`flex`, `grid`, `gap`, `padding`) → 全面基于 Tailwind class 构建与微调
3. **交互动画** → 在 `huanxing.css` 中定义 `.hx-xxx` 专有 class 或借助 Tailwind `hover:` 一站解决
4. **禁止使用** 硬编码默认颜色类（如 `bg-white`, `text-gray-900`, `border-gray-200` 等），这些类在夜间模式时不具备自适应动态肤色切换能力
5. **圆角** → 使用系统的专属类包裹 `rounded-hx-radius-sm/md/lg/full`

### 6.3 CSS class 命名规范（仅限在 huanxing.css 手写扩充时参考）

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

// 正确使用方案：尾随统一的 Tailwind 工具类与文字色对齐缩放
<Play className="w-4 h-4 text-hx-purple" />
<CheckCircle className="w-5 h-5 text-hx-green" />
```

常用图标速查：

| 场景 | 图标 |
|------|------|
| 播放/启动 | `Play` |
| 成功 | `CheckCircle` / `CheckCircle2` |
| 警告 | `AlertTriangle` |
| 错误 | `XCircle` / `AlertCircle` |
| 加载中 | `Loader2` (配合 `className="animate-spin"`) |
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

所有 API 模块统一放在 `src/lib/` 下：

```ts
// src/lib/my-api.ts
import { apiFetch } from '@/lib/huanxing-api';   // 唤星统一 fetch wrapper

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

- [ ] **主题兼容**：Light 和 Dark 模式下均正常显示（特别检查标签、副标题、内嵌框）
- [ ] **变量检查**：无硬编码的颜色值和非法内联设定（搜索 `style={{ color: `、`bg-white` 排除污染）
- [ ] **组件复用**：已搜索 `components/ui/` 确认无现有可用组件，若新增了 UI 组件则已封装为公共组件并登记至本文档 §4.2
- [ ] **Radix 优先**：需要新基础 UI 时优先使用 Radix UI 原语封装，而非自造原生标签组件
- [ ] **API 规范**：新 API 调用统一挂靠 `src/lib/` 下封装好的 fetcher
- [ ] **导入路径**：所有 import 使用 `@/` 别名，无 `../../` 深层相对路径
- [ ] **TypeScript**：`npm run build` 确保核心静态解析未产生红线报警
- [ ] **自适应弹性（响应式）**：缩放视图边界窗口内容无硬溢出

---

## 10. 反模式清单

| ❌ 决不允许这样做 | ✅ 严丝合缝的安全做法 |
|--------------|-------------|
| `style={{ background: 'var(--hx-bg-panel)' }}` | `className="bg-hx-bg-panel"` |
| `className="bg-white border-gray-200 text-gray-900"` | `className="bg-hx-bg-panel border border-hx-border text-hx-text-primary"` |
| `className="bg-[#050b1a] text-[#a7c4f3]"` | `className="bg-hx-bg-main text-hx-text-primary"` |
| `style={{ color: '#6B7280' }}` | `className="text-hx-text-secondary"` |
| `import { FaXxx } from 'react-icons/fa'` | `import { Xxx } from 'lucide-react'` |
| 自己手撸 `<select>` 标签构建交互 | `import { Select } from '@/components/ui/Select'` |
| 自己手撸 `<dialog>` 或 `window.confirm()` | `import { Dialog } from '@/components/ui/Dialog'` 或 `AlertDialog` |
| 自己手撸 `<input>` 标签 | `import { Input } from '@/components/ui/Input'` |
| 为单个页面重复封装已有公共组件 | 直接引用 `@/components/` 下的已有组件 |
| 各自为政新建 WebSocket 连接通道 | `wsMultiplexer.subscribe(sessionId, handler)` |
| `npm install some-large-ui-framework` | 探索在已存 Radix 原语和原生 CSS 方案中闭环实现 |

---

## 附录 A：核心 CSS 原子类快查表（基于 Tailwind v4 @theme）

使用对应原子类会自动适配全局夜灯与白日双向解析（例如：通过 `bg-{变量}`、`text-{变量}` 自由组合）：

| Tailwind 魔术核心变量名 | Light 底盘值 | Dark 夜行值 | 对应经典用途 |
|------|----------|---------|------|
| `hx-bg-main` | `#FFFFFF` | `#1A1F2E` | 页面主背景最深处 |
| `hx-bg-panel` | `#FAFAFA` | `#111827` | 悬浮卡片/模态窗口/面板图层 |
| `hx-bg-input` | `#F9FAFB` | `#1F2937` | 搜索/输入框/下凹次级容器背景 |
| `hx-bg-rail` | `#F5F3FF` | `#0B0F1A` | 全局左侧超重磅导航栏基底 |
| `hx-bg-hover` | `#F3F0FF` | `rgba(124,58,...0.08)` | 面板列表交互高亮选定态 |
| `hx-text-primary` | `#111827` | `#F9FAFB` | 夺目的 H1~H4 标题及核心内容文字 |
| `hx-text-secondary` | `#6B7280` | `#9CA3AF` | 基础段落辅助文本，不宣兵夺主 |
| `hx-text-tertiary` | `#9CA3AF` | `#6B7280` | 版权时间戳/弱化标记/Placeholder |
| `hx-border` | `#E5E7EB` | `#374151` | 分隔核心图层与结构的基座边框线 |
| `hx-border-light` | `#F3F4F6` | `#1F2937` | 细软的局部界定线 |
| `hx-purple` | `#7C3AED` | `#7C3AED` | 大声告诉别人的 CTA 品牌基石色 |
| `hx-purple-bg` | `[alpha 0.08]` | `[alpha 0.12]` | 用户视线吸引专区图版垫底色 |

---

## 附录 B：已加盖审批封印的顶级依赖库阵列

| 挂载领域 | 认可体系库标准 | 坚决禁止偷渡进入的项目 |
|------|--------|----------|
| 一元化图标 | `lucide-react` | `react-icons`, `heroicons`, `fontawesome` 等 |
| 高级 Select 交互 | `@radix-ui/react-select` | 原生或 `headlessui` 及自造烂摊子 |
| 高阶悬浮层栈 | `@radix-ui/react-popover` | `tippy.js`, `popper.js` |
| CSS 原研工具 | `tailwindcss` + `clsx` + `tailwind-merge` | 沉重的 `styled-components` 等纯 CS-In-JS |
| 超大宽广暗房 | `react-photo-view` | `fslightbox`, `photoswipe` |
| 头像图片手术刀 | `react-easy-crop` | `react-avatar-editor`, `cropperjs` |
| Markdown 重塑者 | `react-markdown` + `remark-gfm` | `marked`, `markdown-it` |
| 代码夜灯渲染者 | `shiki` | `highlight.js`, `prism.js` |
| 专业代码录入 | `@uiw/react-codemirror` | `monaco-editor`, `ace-editor` 大量污染层 |
| 全局斜杠神界 | `cmdk` | 其他需要繁重预置面板指令插件 |
