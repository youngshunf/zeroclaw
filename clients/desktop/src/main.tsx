/**
 * 唤星桌面端入口文件
 *
 * 复用 web/ 的 App 组件，在 Tauri 环境中运行。
 * 通过 vite.config.ts 的 alias 将 @ 指向 web/src/。
 */

// ⚡ 必须在 import App 之前设置标志位，否则模块加载时读不到
declare global {
  interface Window {
    __HUANXING_DESKTOP__?: boolean;
  }
}
window.__HUANXING_DESKTOP__ = true;

import React from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import App from "@/App";
// 桌面端专用 CSS 入口（含 @source 指向 web/src，保证 Tailwind 扫描到所有 class）
import "./app.css";
// web 原版自定义样式（含 .app-shell / .electric-button 等自定义 class）
import "@/index.css";
// 唤星主题覆盖
import "@/huanxing/styles/huanxing-theme.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <BrowserRouter basename="/">
      <App />
    </BrowserRouter>
  </React.StrictMode>
);
