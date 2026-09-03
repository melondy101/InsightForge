import { describe, expect, it, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';

import { useDrafts } from './useDrafts';
import type { DraftFile } from './api';

// vi.mock 工厂会被提升，无法引用顶层 const；用 vi.hoisted 定义 mock 并供工厂与断言共用。
const mocks = vi.hoisted(() => ({
  ensureDirectory: vi.fn().mockResolvedValue(undefined),
  list: vi.fn().mockResolvedValue([]),
  create: vi.fn(),
  open: vi.fn(),
  save: vi.fn(),
  delete: vi.fn(),
  resolveConflict: vi.fn(),
}));

vi.mock('./api', async () => {
  const actual = await vi.importActual<typeof import('./api')>('./api');
  return {
    ...actual,
    draftsApi: {
      ensureDirectory: mocks.ensureDirectory,
      list: mocks.list,
      create: mocks.create,
      open: mocks.open,
      save: mocks.save,
      delete: mocks.delete,
      resolveConflict: mocks.resolveConflict,
    },
  };
});

const sampleDraft: DraftFile = {
  id: 'd1',
  path: 'C:/x/note.md',
  filename: 'note.md',
  baselineHash: 'h',
  baselineMtimeMs: 1,
  baselineSize: 1,
  createdAt: '2026-01-01T00:00:00.000000Z',
  updatedAt: '2026-01-01T00:00:00.000000Z',
};

function conflictError(local: string, external: string) {
  // Rust 将 Conflict 序列化为 message 字段（camelCase JSON）
  const payload = JSON.stringify({
    message: 'file modified externally',
    baselineContent: 'base',
    localContent: local,
    externalContent: external,
  });
  return { code: 'conflict', message: payload };
}

describe('useDrafts conflict resolution', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (mocks.list).mockResolvedValue([sampleDraft]);
    (mocks.open).mockResolvedValue(sampleDraft);
    (mocks.resolveConflict).mockResolvedValue(sampleDraft);
  });

  it('捕获冲突时完整保留 externalContent（不静默丢弃）', async () => {
    (mocks.save).mockRejectedValueOnce(
      conflictError('MY-LOCAL', 'EXTERNAL-VERSION'),
    );

    const { result } = renderHook(() => useDrafts());
    await act(async () => {
      await result.current.actions.open('note.md');
    });
    act(() => {
      result.current.actions.setContent('MY-LOCAL');
    });
    await act(async () => {
      await result.current.actions.save();
    });

    expect(result.current.status).toBe('conflict');
    expect(result.current.conflict).not.toBeNull();
    expect(result.current.conflict?.external).toBe('EXTERNAL-VERSION');
    expect(result.current.conflict?.local).toBe('MY-LOCAL');
  });

  it('选择采用外部 → 调用 adopt_external 并清除冲突', async () => {
    (mocks.save).mockRejectedValueOnce(
      conflictError('MY-LOCAL', 'EXTERNAL-VERSION'),
    );

    const { result } = renderHook(() => useDrafts());
    await act(async () => {
      await result.current.actions.open('note.md');
    });
    await act(async () => {
      await result.current.actions.save();
    });

    await act(async () => {
      await result.current.actions.resolveConflict('external');
    });

    expect(mocks.resolveConflict).toHaveBeenCalledWith('note.md', 'adopt_external', 'MY-LOCAL');
    expect(result.current.conflict).toBeNull();
    expect(result.current.status).not.toBe('conflict');
  });

  it('选择保留本地 → 调用 keep_local；另存副本 → save_as_copy', async () => {
    (mocks.save).mockRejectedValueOnce(
      conflictError('MY-LOCAL', 'EXTERNAL-VERSION'),
    );

    const { result } = renderHook(() => useDrafts());
    await act(async () => {
      await result.current.actions.open('note.md');
    });
    await act(async () => {
      await result.current.actions.save();
    });

    await act(async () => {
      await result.current.actions.resolveConflict('local');
    });
    expect(mocks.resolveConflict).toHaveBeenLastCalledWith('note.md', 'keep_local', 'MY-LOCAL');

    (mocks.save).mockRejectedValueOnce(
      conflictError('MY-LOCAL-2', 'EXTERNAL-VERSION-2'),
    );
    await act(async () => {
      await result.current.actions.save();
    });
    await act(async () => {
      await result.current.actions.resolveConflict('saveAs');
    });
    expect(mocks.resolveConflict).toHaveBeenLastCalledWith('note.md', 'save_as_copy', 'MY-LOCAL-2');
  });
});
