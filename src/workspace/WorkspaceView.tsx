import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown, markdownKeymap } from '@codemirror/lang-markdown';
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language';
import { EditorView, keymap, placeholder } from '@codemirror/view';

import { createEditorAdapter, type EditorAdapter } from '../editor/adapter';
import { headingPathAt, paragraphIndexAt } from '../editor/context';
import { applyDividerDrag, clampPaneWidths, type DividerSide, type PaneWidths } from './layout';
import { foundryTheme } from '../editor/theme';

declare global {
  interface Window {
    /**
     * 仅开发构建暴露的编辑器调试钩子（Golden Path 驱动用）：
     * 生产构建不设置，运行时行为与生产一致。
     */
    __ifEditor?: EditorAdapter;
  }
}

const INITIAL_LEFT_RATIO = 0.2;
const INITIAL_RIGHT_RATIO = 0.2;

interface EditorStatus {
  empty: boolean;
  paragraphIndex: number | null;
  headingPath: string[];
}

/**
 * 工作台：可调三栏（左：证据架/检索占位；中：编辑器；右：预留）。
 * 分隔条拖拽经纯函数 applyDividerDrag 约束，中栏宽度无论怎么拖都不低于总宽 35%；
 * 编辑器实现藏在 EditorAdapter 之后（CONTEXT.md §七），本组件不接触 CodeMirror 类型。
 */
export function WorkspaceView() {
  const { t } = useTranslation();
  const panesRef = useRef<HTMLDivElement>(null);
  const [widths, setWidths] = useState<PaneWidths | null>(null);

  // 初次测量与窗口尺寸变化时重新约束（保持中栏 ≥ 35%）。
  useEffect(() => {
    const element = panesRef.current;
    if (!element) {
      return;
    }
    const remeasure = () => {
      const total = element.clientWidth;
      setWidths(
        (previous) =>
          previous
            ? clampPaneWidths(previous, total)
            : {
                left: total * INITIAL_LEFT_RATIO,
                center: total * (1 - INITIAL_LEFT_RATIO - INITIAL_RIGHT_RATIO),
                right: total * INITIAL_RIGHT_RATIO,
              },
      );
    };
    remeasure();
    const observer = new ResizeObserver(remeasure);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const handleDrag = (side: DividerSide) => (deltaPx: number) => {
    const element = panesRef.current;
    if (!element) {
      return;
    }
    setWidths(
      (previous) => previous && applyDividerDrag(previous, element.clientWidth, side, deltaPx),
    );
  };

  return (
    <div className="workspace-view" data-el="workspace-view">
      <div className="workspace-view__panes" ref={panesRef} data-el="workspace-panes">
        <section
          className="ws-pane ws-pane--left"
          data-el="workspace-pane-left"
          style={{ width: widths ? widths.left + 'px' : undefined }}
        >
          <h3 className="ws-pane__title">{t('workspace.paneLeft')}</h3>
        </section>
        <div
          className="ws-divider"
          data-el="divider-left"
          role="separator"
          aria-orientation="vertical"
          onPointerDown={startDrag(handleDrag('left'))}
        />
        <section
          className="ws-pane ws-pane--center"
          data-el="workspace-pane-center"
          style={{ width: widths ? widths.center + 'px' : undefined }}
        >
          <EditorPane />
        </section>
        <div
          className="ws-divider"
          data-el="divider-right"
          role="separator"
          aria-orientation="vertical"
          onPointerDown={startDrag(handleDrag('right'))}
        />
        <section
          className="ws-pane ws-pane--right"
          data-el="workspace-pane-right"
          style={{ width: widths ? widths.right + 'px' : undefined }}
        >
          <h3 className="ws-pane__title">{t('workspace.paneRight')}</h3>
        </section>
      </div>
    </div>
  );
}

function startDrag(onMove: (deltaPx: number) => void) {
  return (event: React.PointerEvent<HTMLDivElement>) => {
    event.preventDefault();
    const target = event.currentTarget;
    target.setPointerCapture(event.pointerId);
    let lastX = event.clientX;
    const move = (moveEvent: PointerEvent) => {
      const delta = moveEvent.clientX - lastX;
      lastX = moveEvent.clientX;
      onMove(delta);
    };
    const stop = () => {
      target.removeEventListener('pointermove', move);
      target.removeEventListener('pointerup', stop);
      target.removeEventListener('pointercancel', stop);
    };
    target.addEventListener('pointermove', move);
    target.addEventListener('pointerup', stop);
    target.addEventListener('pointercancel', stop);
  };
}

/** 中栏编辑器：EditorAdapter 的生命周期与状态条（提取 API 的可见冒烟）。 */
function EditorPane() {
  const { t } = useTranslation();
  const hostRef = useRef<HTMLDivElement>(null);
  const adapterRef = useRef<EditorAdapter | null>(null);
  const [status, setStatus] = useState<EditorStatus>({ empty: true, paragraphIndex: null, headingPath: [] });

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const adapter = createEditorAdapter({
      extensions: [
        foundryTheme,
        history(),
        keymap.of([...markdownKeymap, ...defaultKeymap, ...historyKeymap]),
        markdown(),
        syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
        EditorView.lineWrapping,
        placeholder(t('workspace.editorPlaceholder')),
      ],
    });
    adapter.mount(host);
    adapterRef.current = adapter;
    if (import.meta.env.DEV) {
      window.__ifEditor = adapter;
    }
    return () => {
      adapter.destroy();
      adapterRef.current = null;
      if (import.meta.env.DEV) {
        delete window.__ifEditor;
      }
    };
    // 仅挂载时创建一次；placeholder 随首次语言固定（语言切换后重启应用生效）。
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const adapter = adapterRef.current;
    if (!adapter) {
      return;
    }
    const refresh = () => {
      const doc = adapter.getValue();
      const { anchor } = adapter.getSelection();
      setStatus({
        empty: doc === '',
        paragraphIndex: paragraphIndexAt(doc, anchor),
        headingPath: headingPathAt(doc, anchor),
      });
    };
    refresh();
    const offChange = adapter.onChange(refresh);
    const offSelection = adapter.onSelectionChange(refresh);
    return () => {
      offChange();
      offSelection();
    };
  }, []);

  return (
    <div className="editor-pane" data-el="editor-pane">
      <div className="editor-pane__host" ref={hostRef} data-el="editor-host" />
      <div className="editor-pane__status" data-el="editor-status">
        <span data-el="editor-status-paragraph">
          {status.empty
            ? t('workspace.status.emptyDoc')
            : status.paragraphIndex === null
              ? t('workspace.status.blankLine')
              : t('workspace.status.paragraph', { index: status.paragraphIndex })}
        </span>
        <span data-el="editor-status-heading">
          {status.headingPath.length > 0
            ? status.headingPath.join(' › ')
            : t('workspace.status.noHeading')}
        </span>
      </div>
    </div>
  );
}
