import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [vue()],

  resolve: {
    alias: {
      // 绕过 monaco-editor 的 exports 字段限制，使子路径导入（含 worker）可被 Vite/Rollup 解析
      "monaco-editor": new URL("./node_modules/monaco-editor/", import.meta.url).pathname,
    },
  },

  // monaco-editor 体量大且含 worker 子路径，跳过依赖预构建以避免 optimizer 报错
  optimizeDeps: {
    exclude: ["monaco-editor"],
  },

  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          // 将 monaco-editor 拆为独立 chunk，主包变小、缓存友好
          if (id.includes("node_modules/monaco-editor/")) return "monaco";
        },
      },
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
