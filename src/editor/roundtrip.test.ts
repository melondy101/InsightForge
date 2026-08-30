/// <reference types="node" />
import { describe, expect, it } from 'vitest';

import { createEditorAdapter } from './adapter';

/**
 * Markdown round-trip 语料（本票核心资产，spec Testing Decisions 指定的第一组 TS 测试）。
 *
 * 契约：打开 → 不做任何修改 → 序列化，与原始文件逐字节一致（Buffer 比较）。
 * 已知陷阱：CodeMirror 文档内部统一 LF——adapter 必须记录原始 EOL 风格与 BOM
 * 并在序列化时还原（未修改时直接返回原始文本，见 file-text.ts）。
 * 这些测试只依赖 EditorAdapter 公共接口，换编辑器实现时应依然全绿。
 */

/** 语料自检：三种行尾风格都必须在语料中，防止后人删样例造成覆盖静默缩水。 */
function assertCorpusCoversAllLineEndings(corpus: { raw: string }[]): void {
  const styles = new Set(corpus.map((entry) => (entry.raw.includes('\r\n') ? 'crlf' : entry.raw.includes('\n') ? 'lf' : entry.raw.includes('\r') ? 'cr' : 'none')));
  for (const style of ['crlf', 'lf', 'cr'] as const) {
    if (!styles.has(style)) {
      throw new Error(`corpus is missing ${style} samples`);
    }
  }
}

export const roundTripCorpus: { name: string; raw: string }[] = [
  {
    name: '纯 ASCII',
    raw: '# Heading\n\nSome plain ASCII text with 123 numbers.\n',
  },
  {
    name: '中文长文',
    raw:
      '# 工作台手记\n\n' +
      '中文连续输入的长段落：在真正的写作场景里，创作者会一口气敲下几百个字，' +
      '中间夹着标点（逗号、句号、顿号、引号「」）、数字（2026 年）、英文名词（CodeMirror）与全角符号（——）。' +
      '编辑器必须一个字不丢、一个字不错位。\n\n' +
      '第二个段落继续压测：知识库只读，草稿即成文，文件是唯一真相；这三句话重复三遍也必须毫发无损。' +
      '知识库只读，草稿即成文，文件是唯一真相。知识库只读，草稿即成文，文件是唯一真相。\n',
  },
  { name: 'CRLF 行尾', raw: '# Windows\r\n\r\n每一行都以 CRLF 结尾。\r\n第二行。\r\n' },
  { name: 'LF 行尾', raw: '# Unix\n\n每一行都以 LF 结尾。\n第二行。\n' },
  { name: 'CR 行尾（经典 Mac）', raw: '# Classic\r每一行都以 CR 结尾。\r第二行。\r' },
  { name: 'BOM 开头', raw: '\uFEFF# 带 BOM 的文件\r\n\r\n有些 Windows 工具会写入 BOM。\r\n' },
  { name: '无尾换行', raw: '# 无尾换行\n\n最后一行没有换行符' },
  { name: '尾随空格（硬换行）', raw: '第一行以两个空格结尾  \n第二行正常。\n' },
  { name: 'Tab 缩进', raw: '- 顶层\n\t- 制表符缩进的子项\n\t\t- 更深层\n' },
  {
    name: '嵌套列表',
    raw: '- 一层\n  - 二层\n    - 三层\n1. 有序\n   1. 有序嵌套\n- [ ] 任务\n- [x] 已完成\n',
  },
  {
    name: '代码栅栏',
    raw: '```ts\nconst x: string = "中文与 \'quotes\'";\n```\n\n~~~\nfenced tilde\n~~~\n',
  },
  {
    name: '表格',
    raw: '| 列一 | 列二 |\n| --- | --- |\n| 甲 | 乙 |\n| 长 内 容 | `code` |\n',
  },
  { name: '引用块', raw: '> 一层引用\n> > 嵌套引用\n> - 引用里的列表\n' },
  { name: 'emoji 与代理对', raw: '# 🚀 标题\n\n中文 🎉 混排 👨‍👩‍👧‍👦（ZWJ 序列）与 ☝🏻（肤色调制）。\n' },
  { name: '行内代码与强调', raw: '行内 `code`、**粗体**、*斜体*、~~删除线~~、[链接](https://example.com)。\n' },
  { name: '混合行尾', raw: '第一行 CRLF\r\n第二行 LF\n第三行 CR\r' },
  { name: '空文件', raw: '' },
  { name: '只有换行的文件', raw: '\n\n\n' },
];

describe('round-trip 语料：打开 → 不修改 → 序列化逐字节一致', () => {
  it('语料覆盖三种行尾风格', () => {
    assertCorpusCoversAllLineEndings(roundTripCorpus);
  });

  it.each(roundTripCorpus)('$name', ({ raw }) => {
    const adapter = createEditorAdapter();
    adapter.openDocument(raw);
    const value = adapter.getValue();
    expect(Buffer.from(value, 'utf8').equals(Buffer.from(raw, 'utf8'))).toBe(true);
  });
});

describe('round-trip 语料：真实临时文件逐字节一致（Buffer 级）', () => {
  it('写入磁盘 → 打开 → 不修改 → 写回，两个文件字节完全相同', async () => {
    const { mkdtemp, writeFile, readFile, rm } = await import('node:fs/promises');
    const { tmpdir } = await import('node:os');
    const { join } = await import('node:path');
    const dir = await mkdtemp(join(tmpdir(), 'if05-roundtrip-'));
    try {
      const original = join(dir, 'original.md');
      const saved = join(dir, 'saved.md');
      const bytes = Buffer.from(
        '\uFEFF# 中文草稿\r\n\r\n带 BOM 的 CRLF 文件。\r\n\r\n```code\r\n原样\r\n```\r\n',
        'utf8',
      );
      await writeFile(original, bytes);
      const adapter = createEditorAdapter();
      adapter.openDocument((await readFile(original, 'utf8')).valueOf());
      await writeFile(saved, Buffer.from(adapter.getValue(), 'utf8'));
      expect(Buffer.compare(await readFile(original), await readFile(saved))).toBe(0);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
