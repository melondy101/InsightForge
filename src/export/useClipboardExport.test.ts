import { beforeEach, describe, expect, it, vi } from 'vitest';
import { renderHook, act } from '@testing-library/react';

import { useClipboardExport } from './useClipboardExport';

const writeClipboard = vi.fn();

vi.mock('./api', () => ({
  exportApi: {
    writeClipboard: (...args: [{ html: string; text: string }]) => writeClipboard(...args),
  },
}));

describe('useClipboardExport', () => {
  beforeEach(() => {
    writeClipboard.mockReset();
  });

  it('一次写入富文本与纯文本两种剪贴板格式', async () => {
    writeClipboard.mockResolvedValue(undefined);
    const { result } = renderHook(() => useClipboardExport());

    await act(async () => {
      await result.current.actions.exportToClipboard('# 标题\n\n正文', '标题', '#c86428');
    });

    expect(writeClipboard).toHaveBeenCalledTimes(1);
    const [payload] = writeClipboard.mock.calls[0];
    expect(payload.html).toContain('<h1');
    expect(payload.text).toBe('标题\n\n正文');
    expect(result.current.error).toBeNull();
    expect(result.current.exporting).toBe(false);
  });

  it('剪贴板写入失败时把错误暴露给调用方', async () => {
    writeClipboard.mockRejectedValue({ code: 'clipboard_unavailable', message: '剪贴板被占用' });
    const { result } = renderHook(() => useClipboardExport());

    await act(async () => {
      await result.current.actions.exportToClipboard('正文', '标题', '#c86428');
    });

    expect(result.current.error).toContain('剪贴板被占用');
    expect(result.current.exporting).toBe(false);
  });
});
