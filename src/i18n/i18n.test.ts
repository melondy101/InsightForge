import { expect, it } from 'vitest';
import { i18n } from './index';
import enUS from './locales/en-US.json';
import zhCN from './locales/zh-CN.json';

// 展开嵌套资源对象为「a.b.c」键路径列表，用于断言两语言键一致。
function flattenKeys(node: unknown, prefix = ''): string[] {
  if (typeof node !== 'object' || node === null) {
    return [prefix];
  }
  return Object.entries(node).flatMap(([key, value]) =>
    flattenKeys(value, prefix ? `${prefix}.${key}` : key),
  );
}

it('默认以 zh-CN 呈现三栏工作区占位文案', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('shell.sidebar')).toBe('侧栏');
  expect(i18n.t('shell.editor')).toBe('编辑器');
  expect(i18n.t('shell.evidenceShelf')).toBe('证据架');
});

it('切换到 en-US 时呈现英文占位文案', () => {
  i18n.changeLanguage('en-US');
  expect(i18n.t('shell.sidebar')).toBe('Sidebar');
  expect(i18n.t('shell.editor')).toBe('Editor');
  expect(i18n.t('shell.evidenceShelf')).toBe('Evidence shelf');
});

it('zh-CN 与 en-US 资源树保持键一致', () => {
  expect(flattenKeys(enUS)).toEqual(flattenKeys(zhCN));
});

it('来源管理文案在两种语言下可用', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('sources.nav.sources')).toBe('来源管理');
  expect(i18n.t('sources.status.indexing')).toBe('索引中');
  expect(i18n.t('sources.error.source_paused')).toBe('来源已暂停，恢复后才能同步');
  i18n.changeLanguage('en-US');
  expect(i18n.t('sources.nav.sources')).toBe('Sources');
  expect(i18n.t('sources.status.indexing')).toBe('Indexing');
  expect(i18n.t('sources.error.source_paused')).toBe('Source is paused; resume it before syncing');
});
