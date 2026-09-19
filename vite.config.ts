import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri 前端约定：固定 1420 端口（tauri.conf.json 的 devUrl 指向这里），
// 构建产物 dist/ 由 Tauri 以自定义协议打包。
export default defineConfig({
  // 没有这个插件时 dev 下没有 Fast Refresh，改一行组件就整页重载。
  plugins: [react()],
  // 部署到项目 Pages 子路径（/Repo/）时要带上，否则资源 404；桌面与根路径部署用 '/'。
  base: process.env.VITE_BASE_PATH ?? '/',
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
