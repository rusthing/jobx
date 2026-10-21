import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { fileURLToPath, URL } from "node:url";

/**
 * 开发期把 `/api` 反代到 JobX 服务端（默认 127.0.0.1:9001，见 jobx-svr.toml 的 web.port），
 * 并去掉 `/api` 前缀 —— 后端真实路径是 `/jobx/job/list` 这类，不带 `/api`。
 *
 * 换后端地址：VITE_PROXY_TARGET=http://10.0.0.7:9001 pnpm dev
 */
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  server: {
    port: 5180,
    proxy: {
      "/api": {
        target: process.env.VITE_PROXY_TARGET ?? "http://127.0.0.1:9001",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
  },
  preview: {
    port: 4180,
  },
  build: {
    target: "es2022",
    sourcemap: false,
  },
});
