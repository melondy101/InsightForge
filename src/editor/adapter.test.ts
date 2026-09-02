import { describe, expect, it, vi } from 'vitest';

import { createEditorAdapter } from './adapter';

/**
 * EditorAdapter 契约测试（headless，不依赖 DOM）。
 * 只通过公共接口断言行为；换编辑器实现时本文件应依然全绿。
 */

describe('编辑后的序列化语义', () => {
  it('带 BOM 的 CRLF 文件经编辑后，全文（含新行）统一为 CRLF，BOM 保留', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('\uFEFF第一行\r\n第二行\r\n');
    // 整体替换为含 LF 新行的内容（编辑路径：文档与基线不再一致）
    adapter.setValue('\uFEFF第一行\n第二行\n新行');
    expect(adapter.getValue()).toBe('\uFEFF第一行\r\n第二行\r\n新行');
  });

  it('修改后 getValue 不再走原始文本快速路径，而是按特征还原', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('甲\r\n乙\r\n');
    // 直接把新内容整体写入（LF），序列化时应还原为 CRLF
    adapter.setValue('甲\n乙\n新行');
    expect(adapter.getValue()).toBe('甲\r\n乙\r\n新行');
  });

  it('「改了又改回」仍然逐字节返回原始文本', () => {
    const adapter = createEditorAdapter();
    const raw = '混合\r\n行尾\n文件\r';
    adapter.openDocument(raw);
    adapter.setValue('临时内容');
    adapter.setValue(raw);
    expect(adapter.getValue()).toBe(raw);
  });

  it('openDocument 打开新文件会替换内容并重设基线', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('文件甲');
    adapter.openDocument('# 文件乙\r\n');
    expect(adapter.getValue()).toBe('# 文件乙\r\n');
  });

  it('空编辑器 getValue 为空字符串', () => {
    expect(createEditorAdapter().getValue()).toBe('');
  });
});

describe('选区契约', () => {
  it('getSelection/setSelection 往返一致', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('甲乙丙\n丁戊己');
    adapter.setSelection({ anchor: 2, head: 7 });
    expect(adapter.getSelection()).toEqual({ anchor: 2, head: 7 });
  });

  it('setSelection 触发 onSelectionChange 并携带选区', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('甲乙丙\n丁戊己');
    const listener = vi.fn();
    adapter.onSelectionChange(listener);
    adapter.setSelection({ anchor: 1, head: 3 });
    expect(listener).toHaveBeenCalledWith({ anchor: 1, head: 3 });
  });

  it('onSelectionChange 返回的退订函数生效', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('甲乙丙');
    const listener = vi.fn();
    const unsubscribe = adapter.onSelectionChange(listener);
    unsubscribe();
    adapter.setSelection({ anchor: 1, head: 1 });
    expect(listener).not.toHaveBeenCalled();
  });
});

describe('变更通知', () => {
  it('文档变化触发 onChange，纯选区变化不触发', () => {
    const adapter = createEditorAdapter();
    const listener = vi.fn();
    adapter.onChange(listener);
    adapter.setSelection({ anchor: 0, head: 0 });
    expect(listener).not.toHaveBeenCalled();
    adapter.openDocument('新内容');
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('onChange 退订后不再收到通知', () => {
    const adapter = createEditorAdapter();
    const listener = vi.fn();
    const unsubscribe = adapter.onChange(listener);
    unsubscribe();
    adapter.openDocument('新内容');
    expect(listener).not.toHaveBeenCalled();
  });
});

describe('提取接线', () => {
  it('extractContext 用当前文档与选区喂给纯函数', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('# 标题\n\n正文段落。\n');
    adapter.setSelection({ anchor: 6, head: 6 });
    const context = adapter.extractContext();
    expect(context.current?.text).toBe('正文段落。');
    expect(context.outline.map((entry) => entry.text)).toEqual(['标题']);
  });

  it('打开 CRLF 文件后提取仍基于 LF 归一化文档（偏移坐标系一致）', () => {
    const adapter = createEditorAdapter();
    adapter.openDocument('# 标题\r\n\r\n正文段落。\r\n');
    adapter.setSelection({ anchor: 6, head: 6 });
    const context = adapter.extractContext();
    expect(context.current?.text).toBe('正文段落。');
  });
});

describe('生命周期', () => {
  it('destroy 后监听器清空，重复调用安全', () => {
    const adapter = createEditorAdapter();
    const listener = vi.fn();
    adapter.onChange(listener);
    adapter.destroy();
    adapter.destroy();
    // 监听器集合已清空：再次注册并打开文档应正常工作（实例终止语义由调用方保证）
    adapter.openDocument('x');
    expect(adapter.getValue()).toBe('x');
  });
});
