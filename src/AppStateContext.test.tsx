import { describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { Component, type ReactNode } from 'react';

import {
  DraftsModelContext,
  useAppState,
  useAppActions,
  useDraftsModel,
} from './AppStateContext';
import { useDrafts } from './drafts/useDrafts';
import type { DraftFile } from './drafts/api';

// ---- mocks ----

const mockInvoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

function makeDraftFile(overrides: Partial<DraftFile> = {}): DraftFile {
  return {
    id: 'draft-1',
    path: '/drafts/draft-1.md',
    filename: 'draft-1.md',
    baselineHash: 'hash',
    baselineMtimeMs: 0,
    baselineSize: 0,
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...overrides,
  };
}

/** 捕获子树渲染期错误的 ErrorBoundary，用于验证 hook 在 provider 外抛错。 */
class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  render() {
    if (this.state.error) {
      return <div data-testid="error-boundary">{this.state.error.message}</div>;
    }
    return this.props.children;
  }
}

// ---- DraftsModelContext ----

describe('DraftsModelContext', () => {
  it('useDraftsModel 返回 provider 注入的同一份 model', () => {
    const fakeModel = {
      drafts: [],
      activeDraft: null,
      content: '',
      status: 'idle' as const,
      conflict: null,
      error: null,
      actions: {
        create: vi.fn(),
        open: vi.fn(),
        save: vi.fn(),
        delete: vi.fn(),
        refresh: vi.fn(),
        resolveConflict: vi.fn(),
        dismissError: vi.fn(),
      },
    };

    const wrapper = ({ children }: { children: ReactNode }) => (
      <DraftsModelContext.Provider value={fakeModel}>{children}</DraftsModelContext.Provider>
    );

    const { result } = renderHook(() => useDraftsModel(), { wrapper });
    expect(result.current).toBe(fakeModel);
    expect(result.current.activeDraft).toBeNull();
  });

  it('useDraftsModel 在 provider 外部抛出明确错误', () => {
    const spy = vi.spyOn(console, 'error').mockImplementation(() => {});

    let caughtMessage = '';
    const Probe = () => {
      try {
        useDraftsModel();
      } catch (err) {
        caughtMessage = err instanceof Error ? err.message : String(err);
        throw err;
      }
      return null;
    };

    // 用 ErrorBoundary 包裹，防止 React 把错误传播为测试失败
    renderHook(() => null, {
      wrapper: ({ children }) => (
        <ErrorBoundary>
          <Probe />
          {children}
        </ErrorBoundary>
      ),
    });

    expect(caughtMessage).toContain('useDraftsModel');
    expect(caughtMessage).toContain('DraftsModelContext');

    spy.mockRestore();
  });
});

// ---- AppStateContext / AppActionsContext defaults ----

describe('AppStateContext 默认值', () => {
  it('useAppState 返回空 sources 和 null activeDraft', () => {
    const { result } = renderHook(() => useAppState());
    expect(result.current.sources).toEqual([]);
    expect(result.current.activeDraft).toBeNull();
  });

  it('useAppActions 返回可调用的 no-op actions', async () => {
    const { result } = renderHook(() => useAppActions());
    await expect(result.current.refreshSources()).resolves.toBeUndefined();
    await expect(result.current.refreshDrafts()).resolves.toBeUndefined();
    await expect(result.current.openDraft('x.md')).resolves.toBeUndefined();
  });
});

// ---- useDrafts hook：共享状态的真实来源 ----

describe('useDrafts（共享状态源）', () => {
  it('初始状态：drafts 为空、activeDraft 为 null、status idle', () => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue([]);

    const { result } = renderHook(() => useDrafts());

    expect(result.current.drafts).toEqual([]);
    expect(result.current.activeDraft).toBeNull();
    expect(result.current.status).toBe('idle');
    expect(result.current.content).toBe('');
  });

  it('actions.refresh 调用 drafts_list 并更新 drafts 列表', async () => {
    mockInvoke.mockReset();
    const draftA = makeDraftFile({ id: 'a', filename: 'a.md' });
    const draftB = makeDraftFile({ id: 'b', filename: 'b.md' });
    mockInvoke.mockResolvedValue([draftA, draftB]);

    const { result } = renderHook(() => useDrafts());

    await act(async () => {
      await result.current.actions.refresh();
    });

    expect(result.current.drafts).toHaveLength(2);
    expect(result.current.drafts[0].filename).toBe('a.md');
    expect(result.current.drafts[1].filename).toBe('b.md');
    const listCalls = mockInvoke.mock.calls.filter((c) => c[0] === 'drafts_list');
    expect(listCalls.length).toBeGreaterThanOrEqual(1);
  });

  it('actions.open 设置 activeDraft 并刷新列表', async () => {
    mockInvoke.mockReset();
    const opened = makeDraftFile({ id: 'opened-1', filename: 'opened.md' });
    mockInvoke
      .mockResolvedValueOnce(undefined)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce(opened)
      .mockResolvedValueOnce([opened]);

    const { result } = renderHook(() => useDrafts());

    await act(async () => {
      await result.current.actions.open('opened.md');
    });

    expect(result.current.activeDraft).not.toBeNull();
    expect(result.current.activeDraft?.id).toBe('opened-1');
    expect(result.current.activeDraft?.filename).toBe('opened.md');
  });

  it('actions.delete 在 activeDraft 为 null 时是 no-op 且不抛错', async () => {
    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue([]);

    const { result } = renderHook(() => useDrafts());

    await act(async () => {
      await result.current.actions.delete();
    });

    expect(result.current.activeDraft).toBeNull();
    expect(result.current.status).toBe('idle');
  });
});
