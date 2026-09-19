import { readdirSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

import { splitSnippet } from './snippet';

// FTS5 的 snippet() 把用户原文原样带回来，只在命中词两侧套 <mark></mark>。
// 所以尖括号有两种来路：我们加的标记，和用户文件里本来就有的标签。
describe('splitSnippet', () => {
  it('命中词标出来，其余部分保持原文', () => {
    expect(splitSnippet('前置 …<mark>知识</mark> 后续')).toEqual([
      { text: '前置 …', marked: false },
      { text: '知识', marked: true },
      { text: ' 后续', marked: false },
    ]);
  });

  it('来源文件里的 HTML 只是文字，不是标记', () => {
    expect(splitSnippet('<img src=x onerror=alert(1)>正文')).toEqual([
      { text: '<img src=x onerror=alert(1)>正文', marked: false },
    ]);
  });

  it('没有标记时整段是一个普通片段', () => {
    // 两字符查询走线性回退，那条路径不产生标记。
    expect(splitSnippet('…一小段普通正文…')).toEqual([{ text: '…一小段普通正文…', marked: false }]);
  });

  it('标记不配对也不会吞掉后半段', () => {
    expect(splitSnippet('a<mark>b')).toEqual([
      { text: 'a', marked: false },
      { text: 'b', marked: true },
    ]);
  });
});

// 这条是全仓不变量，不只针对检索视图：用户内容一律按文本渲染。
it('src 下不出现 dangerouslySetInnerHTML', () => {
  const offenders = sourceFiles().filter((file) =>
    readFileSync(file, 'utf8').includes('dangerouslySetInnerHTML'),
  );

  expect(offenders).toEqual([]);
});

function sourceFiles(): string[] {
  const root = fileURLToPath(new URL('../', import.meta.url)).replace(/[\\/]$/, '').replace(/\\/g, '/');
  const files = walk(root);
  // 守门本身不许自我豁免：扫不到源码文件就是这条测试失效了。
  expect(files.length).toBeGreaterThan(30);
  return files;
}

function walk(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const full = `${dir}/${entry.name}`;
    if (entry.isDirectory()) return walk(full);
    if (!/\.(ts|tsx)$/.test(entry.name)) return [];
    return /\.test\.(ts|tsx)$/.test(entry.name) ? [] : [full];
  });
}
