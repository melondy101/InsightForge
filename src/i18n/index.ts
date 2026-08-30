import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';

import enUS from './locales/en-US.json';
import zhCN from './locales/zh-CN.json';

// 用户可见文案一律走 i18n（CONTEXT.md §七）：zh-CN 默认，en-US 备选。
export const localeResources = {
  'zh-CN': { translation: zhCN },
  'en-US': { translation: enUS },
} as const;

export const DEFAULT_LOCALE = 'zh-CN';

void i18next.use(initReactI18next).init({
  resources: localeResources,
  lng: DEFAULT_LOCALE,
  fallbackLng: DEFAULT_LOCALE,
  interpolation: {
    // React 已对插值结果做转义，i18next 无需重复处理。
    escapeValue: false,
  },
});

export { i18next as i18n };
