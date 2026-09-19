import { expect, it } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
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

it('默认以 zh-CN 呈现工作台文案', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('shell.sidebar')).toBe('侧栏');
  expect(i18n.t('workspace.paneLeft')).toBe('证据架 / 检索');
  expect(i18n.t('workspace.status.paragraph', { index: 2 })).toBe('第 2 段');
  expect(i18n.t('workspace.status.noHeading')).toBe('无标题');
});

it('切换到 en-US 时呈现英文文案', () => {
  i18n.changeLanguage('en-US');
  expect(i18n.t('shell.sidebar')).toBe('Sidebar');
  expect(i18n.t('workspace.paneLeft')).toBe('Evidence / Search');
  expect(i18n.t('workspace.status.paragraph', { index: 2 })).toBe('Paragraph 2');
  expect(i18n.t('workspace.status.noHeading')).toBe('No heading');
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

it('设置文案（08 号票 settings.* 命名空间）在两种语言下可用', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('settings.nav.settings')).toBe('设置');
  expect(i18n.t('settings.enabledLabel')).toBe('AI 总开关');
  expect(i18n.t('settings.apiKeyConfigured')).toBe('已配置（存于系统凭据库）');
  expect(i18n.t('settings.error.ai_disabled')).toBe('AI 总开关已关闭');
  expect(i18n.t('settings.testResult.failStatus', { status: 401 })).toBe('连接失败（HTTP 401）');
  i18n.changeLanguage('en-US');
  expect(i18n.t('settings.nav.settings')).toBe('Settings');
  expect(i18n.t('settings.enabledLabel')).toBe('AI master switch');
  expect(i18n.t('settings.apiKeyConfigured')).toBe('Configured (stored in OS credential store)');
  expect(i18n.t('settings.error.ai_disabled')).toBe('The AI master switch is off');
  expect(i18n.t('settings.testResult.failStatus', { status: 401 })).toBe(
    'Connection failed (HTTP 401)',
  );
});

it('陪练文案在两种语言下可用（coach.* 命名空间）', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('coach.nav.coach')).toBe('陪练');
  expect(i18n.t('coach.start')).toBe('我卡住了');
  expect(i18n.t('coach.rounds', { round: 1, max: 3, left: 2 })).toBe('1/3 轮 · 还剩 2 轮');
  expect(i18n.t('coach.action.micro_task')).toBe('微任务');
  expect(i18n.t('coach.error.ai_disabled')).toContain('AI 总开关');
  i18n.changeLanguage('en-US');
  expect(i18n.t('coach.nav.coach')).toBe('Coach');
  expect(i18n.t('coach.start')).toBe("I'm stuck");
  expect(i18n.t('coach.rounds', { round: 1, max: 3, left: 2 })).toBe('1/3 rounds · 2 left');
  expect(i18n.t('coach.action.micro_task')).toBe('Micro-task');
  expect(i18n.t('coach.error.ai_disabled')).toContain('AI master switch');
});

it('检索时序契约文案在两种语言下可用（C14）', () => {
  i18n.changeLanguage('zh-CN');
  expect(i18n.t('search.indexing')).toBe('索引中…');
  expect(i18n.t('search.indexingPlaceholder')).toBe('等待索引完成…');
  expect(i18n.t('search.notSearchable')).toBe('索引尚未完成，无法检索');
  expect(i18n.t('search.indexLoading')).toBe('正在等待索引提交完成…');
  i18n.changeLanguage('en-US');
  expect(i18n.t('search.indexing')).toBe('Indexing…');
  expect(i18n.t('search.indexingPlaceholder')).toBe('Waiting for indexing…');
  expect(i18n.t('search.notSearchable')).toBe('Index not yet complete; search is unavailable');
  expect(i18n.t('search.indexLoading')).toBe('Waiting for index commit…');
});

/** 递归收集 src 下的 .ts/.tsx（跳过测试与语言包本身）。 */
function sourcesUnder(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = `${dir}/${name}`;
    if (statSync(full).isDirectory()) return sourcesUnder(full);
    if (!/\.(ts|tsx)$/.test(name) || /\.test\./.test(name) || name === 'i18n.test.ts') return [];
    return [full];
  });
}

it('源码里每个静态 t() 键都在两种语言里有文案（缺键即红）', () => {
  const srcRoot = fileURLToPath(new URL('../', import.meta.url)).replace(/[\\/]$/, '');
  const zhKeys = new Set(flattenKeys(zhCN));
  const enKeys = new Set(flattenKeys(enUS));
  // i18next 复数键写成 x_one / x_other，所以基名也算命中。
  const hits = (keys: Set<string>, key: string) =>
    keys.has(key) || keys.has(`${key}_one`) || keys.has(`${key}_other`);
  const missing: { file: string; key: string }[] = [];
  let used = 0;

  for (const file of sourcesUnder(srcRoot)) {
    const source = readFileSync(file, 'utf8');
    for (const match of source.matchAll(/\bt\(\s*'([a-zA-Z0-9_.]+)'/g)) {
      used += 1;
      const key = match[1];
      if (!hits(zhKeys, key) || !hits(enKeys, key)) missing.push({ file, key });
    }
  }

  // 路径写错时这个断言会红，别让它扫了个空还「通过」。
  expect(used).toBeGreaterThan(100);
  expect(missing).toEqual([]);
});
