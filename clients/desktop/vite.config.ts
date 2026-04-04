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
      // Sidecar HASN 端点（必须在 /api/v1 之前，否则会被后端代理捕获）
      "/api/v1/hasn/connect": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api/v1/hasn/disconnect": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api/v1/hasn/status": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api/v1/hasn/send": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api/v1/hasn/report": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      "/api/v1/agent/hasn-invoke": {
        target: "http://localhost:42620",
        changeOrigin: true,
      },
      // 唤星后端 API → 云端服务器
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
