import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// Tauri expects a fixed port during dev
const TAURI_DEV_HOST = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      // Point @ to web/src so all existing imports work
      "@": path.resolve(__dirname, "../../web/src"),
      // web/src/index.css 中 @import "tailwindcss" 从 web/src 路径 resolve 不到
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
      // 唤星后端 API → localhost:8020
      "/api/v1": {
        target: "http://localhost:8020",
        changeOrigin: true,
      },
      // ZeroClaw sidecar
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
