import { describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';

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
  toCommandError: (err: unknown) => ({ code: 'unknown', message: String(err) }),
}));

function emit(payload: Record<string, unknown>) {
  for (const cb of capturedCbs) {
    cb({ payload });
  }
}

describe('useSources', () => {
  it('backfill_started 将进度初始化为 0/N', async () => {
    capturedCbs.length = 0;

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
});
