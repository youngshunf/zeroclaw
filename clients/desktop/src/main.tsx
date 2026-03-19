/**
 * 唤星桌面端入口文件
 *
 * @ alias 指向 clients/desktop/src/，不再依赖 web/ 目录。
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
// 桌面端专用 CSS 入口（Tailwind + @source 扫描桌面端自身）
import "./app.css";
// 自定义样式（.app-shell / .electric-button 等）
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
