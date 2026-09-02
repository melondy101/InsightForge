import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';

export default tseslint.config(
  {
    // .scratch 是票据工作/证据目录（handoff 驱动脚本等），非产品代码；tsc 的 include 同样不含它。
    ignores: ['dist/**', 'target/**', 'src-tauri/**', 'node_modules/**', '.scratch/**'],
  },
  ...tseslint.configs.recommended,
  {
    files: ['src/**/*.{ts,tsx}'],
    plugins: {
      'react-hooks': reactHooks,
    },
    rules: {
      'react-hooks/rules-of-hooks': 'error',
      'react-hooks/exhaustive-deps': 'warn',
    },
  },
);
