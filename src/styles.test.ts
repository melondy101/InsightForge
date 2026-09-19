import { readdirSync, readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { expect, it } from 'vitest';

/**
 * 样式契约（b20b03e 用截断 dump 恢复 styles.css 之后立下的门禁）。
 *
 * 两个方向都钉：
 * - 源码里用到的类必须在 styles.css 有定义——没定义的元素等于没样式，界面上看不出来，
 *   只有用户在真机上看到裸文本才会发现；
 * - styles.css 里定义的类必须在某处被引用——没人引用的规则会一直跟着文件长大，
 *   改主题时容易被当成「还有人在用」而留下不一致的补丁。
 */

const CLASS_TOKEN = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*(?:__[a-z0-9-]+)?(?:--[a-z0-9-]+)?$/;
const CLASS_NAME_ATTR = /className=(?:"([^"]*)"|'([^']*)'|\{`([^`]*)`\}|\{([^{}]*)\})/g;
const QUOTED = /['"`]([^'"`]*)['"`]/g;

function walk(dir: string, accept: (name: string) => boolean): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = `${dir}/${name}`;
    if (statSync(full).isDirectory()) return name === 'node_modules' ? [] : walk(full, accept);
    return accept(name) ? [full] : [];
  });
}

const srcRoot = fileURLToPath(new URL('.', import.meta.url)).replace(/[\\/]$/, '');
const repoRoot = fileURLToPath(new URL('../', import.meta.url)).replace(/[\\/]$/, '');

/** 从 className 取值里挑出真正的类名 token。 */
function tokensOf(value: string): string[] {
  return value
    .replace(/\$\{[^}]*\}/g, ' ')
    .split(/\s+/)
    .filter((tok) => CLASS_TOKEN.test(tok));
}

/** 视图里用到的类：只认静态字符串、模板字面量的字面段与三元里的字符串。 */
function classesUsedInView(): Set<string> {
  const used = new Set<string>();
  for (const file of walk(srcRoot, (name) => name.endsWith('.tsx'))) {
    const source = readFileSync(file, 'utf8');
    for (const match of source.matchAll(CLASS_NAME_ATTR)) {
      const [, quoted, single, template, expression] = match;
      if (quoted !== undefined || single !== undefined) {
        tokensOf(quoted ?? single).forEach((tok) => used.add(tok));
      } else if (template !== undefined) {
        tokensOf(template).forEach((tok) => used.add(tok));
      } else if (expression !== undefined && expression.includes('?')) {
        // 三元分支里的字符串就是最终类名；比较运算是条件，不是类名。
        const branches = expression.slice(expression.indexOf('?') + 1);
        for (const inner of branches.matchAll(QUOTED)) tokensOf(inner[1]).forEach((tok) => used.add(tok));
      }
    }
  }
  return used;
}

function classesDefinedInStylesheet(): Set<string> {
  const css = readFileSync(`${srcRoot}/styles.css`, 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
  const defined = new Set<string>();
  for (const match of css.matchAll(/\.(-?[_a-zA-Z][\w-]*)/g)) defined.add(match[1]);
  return defined;
}

/** 类是否被任何代码引用（含视图拼接的状态类与 e2e 选择器）。 */
function referencedSomewhere(tok: string): boolean {
  const files = [
    ...walk(srcRoot, (name) => /\.(ts|tsx)$/.test(name)),
    ...walk(`${repoRoot}/e2e`, (name) => /\.(ts|spec\.ts|ps1)$/.test(name)),
    `${repoRoot}/index.html`,
  ];
  const pattern = new RegExp(`(?:^|[^\\w-])${tok}(?:[^\\w-]|$)`);
  return files.some((file) => {
    try {
      return pattern.test(readFileSync(file, 'utf8'));
    } catch {
      return false;
    }
  });
}

it('源码里用到的每个类都在 styles.css 有定义', () => {
  const used = classesUsedInView();
  const defined = classesDefinedInStylesheet();

  expect([...used].filter((tok) => !defined.has(tok))).toEqual([]);
});

it('扫描真的落到了源码（路径写错时别「扫了个空还通过」）', () => {
  expect(classesUsedInView().size).toBeGreaterThan(80);
});

it('styles.css 里没有谁都不用的死规则', () => {
  const defined = [...classesDefinedInStylesheet()];

  expect(defined.filter((tok) => !referencedSomewhere(tok))).toEqual([]);
});
