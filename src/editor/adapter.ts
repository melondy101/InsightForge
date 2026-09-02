import { EditorSelection, EditorState, type Extension, type TransactionSpec } from '@codemirror/state';
import { EditorView, type ViewUpdate } from '@codemirror/view';

import type { EditorSelection as ContextSelection, ExtractedContext } from './context';
import { extractEditorContext } from './context';
import { analyzeFileText, restoreFileText, type FileTextTraits } from './file-text';

/**
 * EditorAdapter —— 编辑器实现适配层（CONTEXT.md §七）。
 *
 * 设计理由：
 * - 接口只暴露「文档与选区」的语义操作（打开/读取/替换/选区/上下文提取），不暴露任何
 *   CodeMirror 类型；未来更换编辑器实现（或升级大版本）只动本文件。
 * - openDocument/getValue 以「文件文本」（含 BOM 与原始行尾）为边界：编辑器内部的
 *   LF 归一化与序列化还原都藏在实现里，调用方（06 号票的草稿保存）拿到的永远是文件字节。
 * - round-trip 语料测试与本接口的其余契约测试全部 headless 运行（只依赖
 *   @codemirror/state 的文档模型），不要求 DOM，也不耦合 EditorView。
 * - extractContext 喂数据给纯函数模块 context.ts，保证提取逻辑可独立测试、供 09 出站包复用。
 */

/** 选区（文档偏移量，UTF-16 码元）。anchor 与 head 相等即纯光标。 */
export interface EditorSelectionRange {
  anchor: number;
  head: number;
}

export interface EditorAdapter {
  /** 打开文件文本（含 BOM 与原始行尾），替换当前内容并重设 round-trip 基线与文件特征。 */
  openDocument(rawText: string): void;
  /** 当前文件文本。未修改时与打开时逐字节一致。 */
  getValue(): string;
  /**
   * 编辑语义：用文件文本整体替换当前内容，但**保留**打开文件的特征（BOM/行尾）与基线——
   * round-trip 的对象始终是「打开的那个文件」（如冲突解决「采用外部」后保存，仍是原文件字节形态）。
   */
  setValue(rawText: string): void;
  getSelection(): EditorSelectionRange;
  setSelection(selection: EditorSelectionRange): void;
  /** 光标/选区前后的段落与标题大纲（喂给纯函数 extractEditorContext）。 */
  extractContext(): ExtractedContext;
  /** 文档内容变化通知；返回退订函数。 */
  onChange(listener: () => void): () => void;
  /** 选区变化通知；返回退订函数。 */
  onSelectionChange(listener: (selection: EditorSelectionRange) => void): () => void;
  /** 挂载到宿主元素（真实 DOM 编辑视图）；headless 使用时可完全不调用。 */
  mount(host: HTMLElement): void;
  /** 卸载编辑视图，保留文档状态。 */
  unmount(): void;
  focus(): void;
  /** 卸载视图并释放监听器；此后实例不再可用。 */
  destroy(): void;
}

export interface EditorAdapterOptions {
  /**
   * 编辑器实现扩展（语法高亮、按键映射、换行策略等）。
   * 适配层自身的行为（round-trip、通知、选区）不依赖任何扩展。
   */
  extensions?: Extension[];
}

export function createEditorAdapter(options: EditorAdapterOptions = {}): EditorAdapter {
  return new CodeMirrorAdapter(options.extensions ?? []);
}

class CodeMirrorAdapter implements EditorAdapter {
  private state: EditorState;
  private view: EditorView | null = null;
  /** 打开/setValue 时的原始文件文本（含 BOM 与原始行尾）。 */
  private originalRaw = '';
  private traits: FileTextTraits = { bom: false, eol: null };
  /** 打开/setValue 时的 LF 归一化文本；当前文档与之逐字符一致即可走原始文本快速路径。 */
  private baseline = '';
  private readonly changeListeners = new Set<() => void>();
  private readonly selectionListeners = new Set<(selection: EditorSelectionRange) => void>();

  constructor(extensions: Extension[]) {
    this.state = EditorState.create({
      doc: '',
      extensions: [
        EditorView.updateListener.of((update) => this.onViewUpdate(update)),
        ...extensions,
      ],
    });
  }

  openDocument(rawText: string): void {
    const { text, traits } = analyzeFileText(rawText);
    this.originalRaw = rawText;
    this.traits = traits;
    this.baseline = text;
    this.replaceWholeDocument(text);
  }

  setValue(rawText: string): void {
    // 编辑语义：只替换文档内容；特征（BOM/行尾）与基线仍属于打开的文件。
    const { text } = analyzeFileText(rawText);
    this.replaceWholeDocument(text);
  }

  getValue(): string {
    const doc = this.current().doc.toString();
    // 未修改（包括「改了又改回」）时直接返回原始文本，对混合行尾等一切形态逐字节无损。
    if (doc === this.baseline) {
      return this.originalRaw;
    }
    return restoreFileText(doc, this.traits);
  }

  getSelection(): EditorSelectionRange {
    const main = this.current().selection.main;
    return { anchor: main.anchor, head: main.head };
  }

  setSelection(selection: EditorSelectionRange): void {
    this.apply({
      selection: EditorSelection.range(selection.anchor, selection.head),
      scrollIntoView: true,
    });
  }

  extractContext(): ExtractedContext {
    const selection: ContextSelection = this.getSelection();
    return extractEditorContext(this.current().doc.toString(), selection);
  }

  onChange(listener: () => void): () => void {
    this.changeListeners.add(listener);
    return () => this.changeListeners.delete(listener);
  }

  onSelectionChange(listener: (selection: EditorSelectionRange) => void): () => void {
    this.selectionListeners.add(listener);
    return () => this.selectionListeners.delete(listener);
  }

  mount(host: HTMLElement): void {
    if (this.view) {
      return;
    }
    this.view = new EditorView({ state: this.state, parent: host });
  }

  unmount(): void {
    if (!this.view) {
      return;
    }
    this.state = this.view.state;
    this.view.destroy();
    this.view = null;
  }

  focus(): void {
    this.view?.focus();
  }

  destroy(): void {
    this.unmount();
    this.changeListeners.clear();
    this.selectionListeners.clear();
  }

  private current(): EditorState {
    return this.view ? this.view.state : this.state;
  }

  /** 统一的事务入口：有视图走视图（更新监听器负责通知），headless 手动通知。 */
  private apply(spec: TransactionSpec): void {
    if (this.view) {
      this.view.dispatch(spec);
      return;
    }
    const transaction = this.state.update(spec);
    const { docChanged, selection } = transaction;
    this.state = transaction.state;
    if (docChanged) {
      this.notifyChange();
    }
    // 与视图语义对齐：显式设置选区，或文档变化导致选区被重映射，都通知。
    if (selection !== undefined || docChanged) {
      this.notifySelection();
    }
  }

  private onViewUpdate(update: ViewUpdate): void {
    if (update.docChanged) {
      this.notifyChange();
    }
    if (update.selectionSet) {
      this.notifySelection();
    }
  }

  private notifyChange(): void {
    for (const listener of this.changeListeners) {
      listener();
    }
  }

  private notifySelection(): void {
    const selection = this.getSelection();
    for (const listener of this.selectionListeners) {
      listener(selection);
    }
  }

  private replaceWholeDocument(text: string): void {
    this.apply({ changes: { from: 0, to: this.current().doc.length, insert: text } });
  }
}
