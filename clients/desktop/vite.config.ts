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
      // 唤星后端 API → 云端服务器
      // 包含: /api/v1/hasn/app/*（云端 HASN 认证）和所有其他 /api/v1/* 业务接口
      // 注：Sidecar HASN API 已从前端直连 localhost:42620，不再经过此代理
      "/api/v1": {
        target: "http://127.0.0.1:8020",
        changeOrigin: true,
        secure: true,
      },
      // ZeroClaw sidecar（通用 /api 路由）
      "/pair": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/health": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/ws": {
        target: "http://localhost:42620",
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
