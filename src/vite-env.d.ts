/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** 网页 runtime 的同步服务根地址；缺省表示「只在桌面里跑」。 */
  readonly VITE_API_BASE_URL?: string;
  /** 构建期注入的应用版本，与 workspace Cargo.toml 同源。 */
  readonly VITE_APP_VERSION?: string;
}
