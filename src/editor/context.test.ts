import { describe, expect, it } from 'vitest';

import { extractEditorContext } from './context';

/**
 * 提取纯函数的契约（供 09 号票出站包消费）：
 * 输入是 LF 行尾的文档字符串 + 光标/选区偏移量，输出「所在段落、前后相邻段落、标题大纲」。
 * 段落 = 连续非空行块；空行 = 空行或全空白行；标题行自成一块但仍在段落结构中。
 */
describe('光标在单个段落内', () => {
  const doc = '第一段开头，第一段中间。\n\n第二段整体。';

  it('光标在段中提取所在段落，前段与后段各一块', () => {
    // 光标落在第一段中间（第 6 个码元之后）
    const context = extractEditorContext(doc, { anchor: 6, head: 6 });
    expect(context.current?.text).toBe('第一段开头，第一段中间。');
    expect(context.before).toBeNull();
    expect(context.after?.text).toBe('第二段整体。');
  });

  it('光标在段首与段尾仍在同一段落', () => {
    const atStart = extractEditorContext(doc, { anchor: 0, head: 0 });
    expect(atStart.current?.text).toBe('第一段开头，第一段中间。');
    const atEnd = extractEditorContext(doc, { anchor: doc.indexOf('\n'), head: doc.indexOf('\n') });
    expect(atEnd.current?.text).toBe('第一段开头，第一段中间。');
    expect(atEnd.after?.text).toBe('第二段整体。');
  });

  it('连续非空行（软换行）构成同一段落', () => {
    const wrapped = '第一行段内，\n第二行段内。\n\n下一段。';
    const context = extractEditorContext(wrapped, { anchor: 3, head: 3 });
    expect(context.current?.text).toBe('第一行段内，\n第二行段内。');
    expect(context.before).toBeNull();
    expect(context.after?.text).toBe('下一段。');
  });
});

describe('跨段选区', () => {
  const doc = '甲段。\n\n乙段一。\n乙段二。\n\n丙段。';

  it('选区覆盖多段时扩展为触碰块的整段范围，前后各取相邻块', () => {
    // 选区从「乙段一」中间跨到「丙段」开头：触碰乙块与丙块 → current = 乙块起点..丙块终点
    const start = doc.indexOf('乙段一') + 1;
    const end = doc.indexOf('丙段') + 1;
    const context = extractEditorContext(doc, { anchor: start, head: end });
    expect(context.current?.text).toBe('乙段一。\n乙段二。\n\n丙段。');
    expect(context.current?.start).toBe(doc.indexOf('乙段'));
    expect(context.current?.end).toBe(doc.length);
    expect(context.before?.text).toBe('甲段。');
    expect(context.after).toBeNull();
  });

  it('反向选区（anchor 在 head 之后）与正向等价', () => {
    const start = doc.indexOf('乙段一') + 1;
    const end = doc.indexOf('丙段') + 1;
    const context = extractEditorContext(doc, { anchor: end, head: start });
    expect(context.current?.text).toBe('乙段一。\n乙段二。\n\n丙段。');
    expect(context.before?.text).toBe('甲段。');
  });
});

describe('光标在空行与文档边界', () => {
  const doc = '上一段。\n\n\n下一段。';

  it('光标在空行：current 为 null，前后取两侧最近段落', () => {
    // 两个空行中的第二个
    const cursor = doc.indexOf('\n', doc.indexOf('\n') + 1) + 1;
    const context = extractEditorContext(doc, { anchor: cursor, head: cursor });
    expect(context.current).toBeNull();
    expect(context.before?.text).toBe('上一段。');
    expect(context.after?.text).toBe('下一段。');
  });

  it('光标在文档开头边界：before 为 null', () => {
    const context = extractEditorContext('首段。\n\n尾段。', { anchor: 0, head: 0 });
    expect(context.current?.text).toBe('首段。');
    expect(context.before).toBeNull();
    expect(context.after?.text).toBe('尾段。');
  });

  it('光标在文档结尾边界：after 为 null', () => {
    const context = extractEditorContext('首段。\n\n尾段。', { anchor: 7, head: 7 });
    expect(context.current?.text).toBe('尾段。');
    expect(context.after).toBeNull();
  });

  it('空文档：全部为 null，大纲为空', () => {
    const context = extractEditorContext('', { anchor: 0, head: 0 });
    expect(context.current).toBeNull();
    expect(context.before).toBeNull();
    expect(context.after).toBeNull();
    expect(context.outline).toEqual([]);
  });

  it('全空白字符的行视为空行', () => {
    const doc2 = '上一段。\n \t \n下一段。';
    const cursor = doc2.indexOf(' \t ');
    const context = extractEditorContext(doc2, { anchor: cursor, head: cursor });
    expect(context.current).toBeNull();
    expect(context.before?.text).toBe('上一段。');
    expect(context.after?.text).toBe('下一段。');
  });
});

describe('标题大纲', () => {
  it('多级标题带祖先路径，同级重新入栈', () => {
    const doc = '# 第一章\n\n正文。\n\n## 第一节\n\n### 小点\n\n## 第二节\n\n# 第二章\n\n## 附节\n';
    const outline = extractEditorContext(doc, { anchor: 0, head: 0 }).outline;
    expect(outline.map((entry) => [entry.level, entry.text, entry.path])).toEqual([
      [1, '第一章', []],
      [2, '第一节', ['第一章']],
      [3, '小点', ['第一章', '第一节']],
      [2, '第二节', ['第一章']],
      [1, '第二章', []],
      [2, '附节', ['第二章']],
    ]);
  });

  it('代码栅栏内的 # 行不算标题', () => {
    const doc = '# 真标题\n\n```bash\n## 不是标题\n```\n\n正文。\n';
    const outline = extractEditorContext(doc, { anchor: 0, head: 0 }).outline;
    expect(outline.map((entry) => entry.text)).toEqual(['真标题']);
  });

  it('波浪线栅栏同样生效，且不匹配少于 3 个标记的行', () => {
    const doc = '## 标题\n\n~~~\n# 栅栏内\n~~~\n\n## 另一节\n';
    const outline = extractEditorContext(doc, { anchor: 0, head: 0 }).outline;
    expect(outline.map((entry) => entry.text)).toEqual(['标题', '另一节']);
  });

  it('无空格的 # 行与四个以上 # 不算标题', () => {
    const doc = '#非标题\n####也是\n### 真标题\n####### 七个不算\n';
    const outline = extractEditorContext(doc, { anchor: 0, head: 0 }).outline;
    expect(outline.map((entry) => [entry.level, entry.text])).toEqual([[3, '真标题']]);
  });
});
