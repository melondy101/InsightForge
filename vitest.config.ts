import { defineConfig } from 'vitest/config';

// Vitest：Node 环境即可覆盖 i18n 资源与纯逻辑；UI 层暂不写测试（见 spec）。
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.{ts,tsx}'],
  },
});
