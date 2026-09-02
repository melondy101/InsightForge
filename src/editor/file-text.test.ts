import { describe, expect, it } from 'vitest';

import { analyzeFileText, restoreFileText } from './file-text';

/**
 * 文件文本归一化（round-trip 的地基）：
 * CodeMirror 文档内部只用 LF，文件层的 BOM 与 CRLF/CR/LF 差异由本模块记录并在序列化时还原。
 */
describe('analyzeFileText（文件 → 编辑器文本 + 特征）', () => {
  it('CRLF 文件：文本归一为 LF，特征记录 crlf', () => {
    const { text, traits } = analyzeFileText('甲\r\n乙\r\n');
    expect(text).toBe('甲\n乙\n');
    expect(traits).toEqual({ bom: false, eol: 'crlf' });
  });

  it('LF 文件原样，特征记录 lf', () => {
    const { text, traits } = analyzeFileText('甲\n乙\n');
    expect(text).toBe('甲\n乙\n');
    expect(traits).toEqual({ bom: false, eol: 'lf' });
  });

  it('经典 Mac CR 文件：文本归一为 LF，特征记录 cr', () => {
    const { text, traits } = analyzeFileText('甲\r乙\r');
    expect(text).toBe('甲\n乙\n');
    expect(traits).toEqual({ bom: false, eol: 'cr' });
  });

  it('BOM（U+FEFF）被剥掉并记录在特征中', () => {
    const { text, traits } = analyzeFileText('\uFEFF# 标题\r\n\r\n正文\r\n');
    expect(text).toBe('# 标题\n\n正文\n');
    expect(traits).toEqual({ bom: true, eol: 'crlf' });
  });

  it('无尾换行的文件保持无尾换行', () => {
    const { text, traits } = analyzeFileText('甲\r\n乙');
    expect(text).toBe('甲\n乙');
    expect(traits).toEqual({ bom: false, eol: 'crlf' });
  });

  it('混合行尾取第一个行分隔符的风格', () => {
    const { text, traits } = analyzeFileText('甲\r\n乙\n丙\r丁');
    expect(text).toBe('甲\n乙\n丙\n丁');
    expect(traits).toEqual({ bom: false, eol: 'crlf' });
  });

  it('单行无换行的文件 eol 为 null', () => {
    const { text, traits } = analyzeFileText('只有一行');
    expect(text).toBe('只有一行');
    expect(traits).toEqual({ bom: false, eol: null });
  });

  it('空文件', () => {
    const { text, traits } = analyzeFileText('');
    expect(text).toBe('');
    expect(traits).toEqual({ bom: false, eol: null });
  });

  it('只有 BOM 的文件', () => {
    const { text, traits } = analyzeFileText('\uFEFF');
    expect(text).toBe('');
    expect(traits).toEqual({ bom: true, eol: null });
  });
});

describe('restoreFileText（编辑器文本 + 特征 → 文件）', () => {
  it('按特征还原 CRLF 与 LF', () => {
    expect(restoreFileText('甲\n乙\n', { bom: false, eol: 'crlf' })).toBe('甲\r\n乙\r\n');
    expect(restoreFileText('甲\n乙\n', { bom: false, eol: 'lf' })).toBe('甲\n乙\n');
  });

  it('按特征还原 CR', () => {
    expect(restoreFileText('甲\n乙\n', { bom: false, eol: 'cr' })).toBe('甲\r乙\r');
  });

  it('BOM 还原到文件开头', () => {
    expect(restoreFileText('# 标题\n', { bom: true, eol: 'crlf' })).toBe('\uFEFF# 标题\r\n');
  });

  it('eol 为 null（原文件无换行）时新增换行用 LF', () => {
    expect(restoreFileText('一行\n两行', { bom: false, eol: null })).toBe('一行\n两行');
  });

  it('与 analyzeFileText 组合是恒等变换（行尾风格单一的文件）', () => {
    const samples = ['甲\r\n乙\r\n', '\uFEFFx\ny', 'a\rb\rc', '无换行', '', '\uFEFF'];
    for (const raw of samples) {
      const { text, traits } = analyzeFileText(raw);
      expect(restoreFileText(text, traits)).toBe(raw);
    }
  });

  it('混合行尾的文件一旦有编辑，全文统一为检测到的风格（与常见编辑器一致）', () => {
    // 未修改时逐字节往返由 adapter 的原始文本路径保证（见 adapter 测试）；
    // restore 只服务于「有编辑后」的序列化。
    const { text, traits } = analyzeFileText('混\r\n合\n行\r');
    expect(restoreFileText(text, traits)).toBe('混\r\n合\r\n行\r\n');
  });
});
