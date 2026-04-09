import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// Tauri expects a fixed port during dev
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;
const TAURI_PLATFORM = process.env.TAURI_ENV_PLATFORM || '';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  define: {
    __TAURI_PLATFORM__: JSON.stringify(TAURI_PLATFORM),
  },
  resolve: {
    alias: {
      // 指向桌面端自身 src，不再依赖 web/src
      "@": path.resolve(__dirname, "./src"),
      // 显式指向桌面端 node_modules 的 index.css
      "tailwindcss": path.resolve(__dirname, "node_modules/tailwindcss/index.css"),
    },
  },

  // Vite options tailored for Tauri
  clearScreen: false,
  server: {
    host: TAURI_DEV_HOST || "localhost",
    port: 1420,
    strictPort: true,
    // Proxy API requests
    proxy: {
      // ⚠️ 注意：代理规则按声明顺序匹配，路径越具体越要放在越前面

      // 1. 云端 HASN App 认证 API（/api/v1/hasn/app/* → 云端 8020）
      //    包括：/auth/register, /auth/register-agent, /conversations, /contacts, /agents 等
      //    必须放在 /api/v1/hasn（sidecar）之前！
      "/api/v1/hasn/app": {
        target: "http://127.0.0.1:8020",
        changeOrigin: true,
        secure: false,
      },

      // 2. Sidecar HASN 节点 API（/api/v1/hasn/* → sidecar 42620）
      //    包括：/connect, /disconnect, /status, /send, /node/* 等本地 sidecar 端点
      //    必须放在 /api/v1（云端）之前！
      "/api/v1/hasn": {
        target: "http://127.0.0.1:42620",
        changeOrigin: true,
      },

      // 3. 其余云端后端 API（/api/v1/* → 云端 8020）
      "/api/v1": {
        target: "http://127.0.0.1:8020",
        changeOrigin: true,
        secure: false,
      },

      // 4. Sidecar 专属路由
      "/pair": {
        target: "http://127.0.0.1:42620",
        changeOrigin: true,
      },
      "/health": {
        target: "http://127.0.0.1:42620",
        changeOrigin: true,
      },

      // 5. Sidecar 通用 /api 路由（/api/agents, /api/config 等）
      "/api": {
        target: "http://127.0.0.1:42620",
        changeOrigin: true,
      },

      // 6. WebSocket → sidecar
      "/ws": {
        target: "http://127.0.0.1:42620",
        ws: true,
        changeOrigin: true,
      },
    },
  },
  build: {
    outDir: "dist",
    // Tauri uses Chromium on Windows and WebKit on macOS/Linux
    target: process.env.TAURI_ENV_PLATFORM === "windows" ? "chrome105" : "safari14",
    // Don't minify for debug builds
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    // Produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
