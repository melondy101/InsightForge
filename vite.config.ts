import { defineConfig } from 'vite';

// Tauri 前端约定：固定 1420 端口（tauri.conf.json 的 devUrl 指向这里），
// 构建产物 dist/ 由 Tauri 以自定义协议打包。
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
  },
});
