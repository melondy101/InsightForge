import { describe, expect, it } from 'vitest';

import { i18n } from '../i18n';
import zhCN from '../i18n/locales/zh-CN.json';
import enUS from '../i18n/locales/en-US.json';
import { ERROR_CODES, errorText, type Translate } from './error-text';

const t: Translate = (key, options) => String(i18n.t(key, options));

describe('错误码 → 文案', () => {
  it('每个 Rust 错误码在两种语言下都有一句话文案', () => {
    const zh = zhCN.errors as Record<string, string>;
    const en = enUS.errors as Record<string, string>;

    expect(ERROR_CODES.filter((code) => !zh[code])).toEqual([]);
    expect(ERROR_CODES.filter((code) => !en[code])).toEqual([]);
  });

  it('没登记过的码不会把英文码丢给用户，而是公共兜底带原文', () => {
    i18n.changeLanguage('zh-CN');

    expect(errorText(t, { code: 'made_up_code', message: 'boom' })).toBe('操作没有完成：boom');
  });

  it('视图里的专属文案优先于公共表', () => {
    i18n.changeLanguage('zh-CN');

    const shown = errorText(t, { code: 'ai_disabled', message: 'off' }, 'coach');

    expect(shown).toBe('AI 总开关是关着的。到设置里打开并填好服务商、模型与 Key，再来叫陪练。');
    expect(shown).not.toBe(zhCN.errors.ai_disabled);
  });

  it('视图没有专属文案时落到公共表', () => {
    i18n.changeLanguage('zh-CN');

    expect(errorText(t, { code: 'io_error', message: 'EACCES' }, 'drafts')).toBe(
      '读写本机文件失败，请检查磁盘与权限。',
    );
  });
});
