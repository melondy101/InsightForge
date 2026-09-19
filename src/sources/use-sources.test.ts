import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';

import { sourcesApi } from './api';
import { useSources } from './useSources';

const capturedCbs: Array<(ev: { payload: Record<string, unknown> }) => void> = [];

const mockListen = vi.fn((_channel: string, cb: (ev: { payload: Record<string, unknown> }) => void) => {
  capturedCbs.push(cb);
  return Promise.resolve(() => {});
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => mockListen(...args as [string, (ev: { payload: Record<string, unknown> }) => void]),
}));

vi.mock('./api', () => ({
  sourcesApi: {
    add: vi.fn(),
    list: vi.fn().mockResolvedValue([]),
    pause: vi.fn(),
    remove: vi.fn(),
    sync: vi.fn(),
  },
}));

function emit(payload: Record<string, unknown>) {
  for (const cb of capturedCbs) {
    cb({ payload });
  }
}

describe('useSources', () => {
  beforeEach(() => {
    // 进度由列表授权，所以每个用例都要显式给出列表状态，别把上一个用例的桩带进来。
    vi.mocked(sourcesApi.list).mockReset();
    vi.mocked(sourcesApi.list).mockResolvedValue([]);
  });

  it('backfill_started 将进度初始化为 0/N', async () => {
    capturedCbs.length = 0;
    vi.mocked(sourcesApi.list).mockResolvedValue([
      source('src-1', 'indexing', { processed: 0, totalFiles: 5 }),
    ]);

    const { result } = renderHook(() => useSources());

    await act(async () => {
      emit({
        type: 'backfill_started',
        sourceId: 'src-1',
        totalFiles: 5,
      });
      await Promise.resolve();
    });

    expect(result.current.progress['src-1']).toEqual({ processed: 0, totalFiles: 5 });
  });

  it('file_indexed 事件序列使 processed 单调递增到 N', async () => {
    capturedCbs.length = 0;
    vi.mocked(sourcesApi.list).mockResolvedValue([
      source('src-1', 'indexing', { processed: 0, totalFiles: 3 }),
    ]);

    const { result } = renderHook(() => useSources());

    await act(async () => {
      emit({
        type: 'backfill_started',
        sourceId: 'src-1',
        totalFiles: 3,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'a.md',
        processed: 1,
        totalFiles: 3,
        fragmentCount: 2,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'b.md',
        processed: 2,
        totalFiles: 3,
        fragmentCount: 1,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'c.md',
        processed: 3,
        totalFiles: 3,
        fragmentCount: 3,
      });
      await Promise.resolve();
    });

    expect(result.current.progress['src-1']).toEqual({ processed: 3, totalFiles: 3 });
  });

  it('backfill_completed 后进度条目被清除', async () => {
    capturedCbs.length = 0;

    const { result } = renderHook(() => useSources());

    await act(async () => {
      emit({
        type: 'backfill_started',
        sourceId: 'src-1',
        totalFiles: 2,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'a.md',
        processed: 1,
        totalFiles: 2,
        fragmentCount: 2,
      });
      emit({
        type: 'backfill_completed',
        sourceId: 'src-1',
        indexedFiles: 2,
        totalFiles: 2,
      });
      await Promise.resolve();
    });

    expect(result.current.progress['src-1']).toBeUndefined();
  });

  it('无变化同步的 file_indexed 事件携带 fragment_count=0', async () => {
    capturedCbs.length = 0;

    const { result } = renderHook(() => useSources());

    await act(async () => {
      emit({
        type: 'backfill_started',
        sourceId: 'src-1',
        totalFiles: 2,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'a.md',
        processed: 1,
        totalFiles: 2,
        fragmentCount: 0,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-1',
        relPath: 'b.md',
        processed: 2,
        totalFiles: 2,
        fragmentCount: 0,
      });
      emit({
        type: 'backfill_completed',
        sourceId: 'src-1',
        indexedFiles: 2,
        totalFiles: 2,
      });
      await Promise.resolve();
    });

    expect(result.current.progress['src-1']).toBeUndefined();
  });

  it('处理多个来源的并发进度', async () => {
    capturedCbs.length = 0;
    const { result } = renderHook(() => useSources());

    await act(async () => {
      emit({
        type: 'backfill_started',
        sourceId: 'src-a',
        totalFiles: 2,
      });
      emit({
        type: 'backfill_started',
        sourceId: 'src-b',
        totalFiles: 3,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-a',
        relPath: 'a1.md',
        processed: 1,
        totalFiles: 2,
        fragmentCount: 1,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-b',
        relPath: 'b1.md',
        processed: 1,
        totalFiles: 3,
        fragmentCount: 2,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-a',
        relPath: 'a2.md',
        processed: 2,
        totalFiles: 2,
        fragmentCount: 1,
      });
      emit({
        type: 'backfill_completed',
        sourceId: 'src-a',
        indexedFiles: 2,
        totalFiles: 2,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-b',
        relPath: 'b2.md',
        processed: 2,
        totalFiles: 3,
        fragmentCount: 0,
      });
      emit({
        type: 'file_indexed',
        sourceId: 'src-b',
        relPath: 'b3.md',
        processed: 3,
        totalFiles: 3,
        fragmentCount: 0,
      });
      emit({
        type: 'backfill_completed',
        sourceId: 'src-b',
        indexedFiles: 3,
        totalFiles: 3,
      });
      await Promise.resolve();
    });

    expect(result.current.progress['src-a']).toBeUndefined();
    expect(result.current.progress['src-b']).toBeUndefined();
  });

  it('列表带着在跑的作业时，一条事件都没收到也能显示进度', async () => {
    capturedCbs.length = 0;
    vi.mocked(sourcesApi.list).mockResolvedValueOnce([
      source('src-1', 'indexing', { processed: 1, totalFiles: 2 }),
    ]);

    const { result } = renderHook(() => useSources());
    await act(async () => {
      await Promise.resolve();
    });

    expect(result.current.progress['src-1']).toEqual({ processed: 1, totalFiles: 2 });
  });

  it('列表说该来源已空闲时进度必须抹掉，哪怕结束事件从未到达', async () => {
    capturedCbs.length = 0;
    vi.mocked(sourcesApi.list).mockResolvedValue([
      source('src-1', 'indexing', { processed: 1, totalFiles: 2 }),
    ]);

    const { result } = renderHook(() => useSources());
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.progress['src-1']).toBeDefined();

    // 补扫悄悄结束，而 backfill_completed 在 dev 模式下丢了。
    vi.mocked(sourcesApi.list).mockResolvedValueOnce([source('src-1', 'idle')]);
    await act(async () => {
      await result.current.actions.refresh();
    });

    expect(result.current.progress['src-1']).toBeUndefined();
  });
});

function source(
  id: string,
  status: 'idle' | 'indexing',
  progress?: { processed: number; totalFiles: number },
) {
  return {
    id,
    name: '笔记',
    rootPath: 'C:/notes',
    paused: false,
    status,
    fileCount: 2,
    ignoreGlobs: [],
    lastSyncedAt: null,
    progress: progress ? { ...progress, phase: 'indexing', fragmentCount: 0 } : null,
  } as unknown as Awaited<ReturnType<typeof sourcesApi.list>>[number];
}
