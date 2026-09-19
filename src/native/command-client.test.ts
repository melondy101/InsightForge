import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  callCommand,
  createFakeCommandClient,
  setCommandClient,
  tauriCommandClient,
  toCommandError,
} from './command-client';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('command client seam', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    setCommandClient(tauriCommandClient);
  });

  it('resolves the command result through the tauri transport', async () => {
    vi.mocked(invoke).mockResolvedValue({ id: 'd1' });

    await expect(callCommand('drafts_open', { filename: 'a.md' })).resolves.toEqual({ id: 'd1' });
    expect(invoke).toHaveBeenCalledWith('drafts_open', { filename: 'a.md' });
  });

  it('turns a transport rejection into a structured command error', async () => {
    vi.mocked(invoke).mockRejectedValue({ code: 'draft_not_found', message: 'no such draft' });

    await expect(callCommand('drafts_open')).rejects.toEqual({
      code: 'draft_not_found',
      message: 'no such draft',
    });
  });

  it('keeps a foreign rejection observable instead of swallowing it', async () => {
    vi.mocked(invoke).mockRejectedValue('boom');

    await expect(callCommand('drafts_list')).rejects.toMatchObject({
      code: 'unknown',
      message: 'boom',
    });
  });

  it('routes every caller through the client installed for this process', async () => {
    const fake = createFakeCommandClient({ drafts_list: () => [{ id: 'd1' }] });
    const restore = setCommandClient(fake);

    await expect(callCommand('drafts_list')).resolves.toEqual([{ id: 'd1' }]);
    expect(fake.calls).toEqual([{ command: 'drafts_list', args: {} }]);

    restore();
    vi.mocked(invoke).mockResolvedValue([]);
    await callCommand('drafts_list');
    expect(invoke).toHaveBeenCalled();
  });

  it('fails loudly when a fake has no handler for a command', async () => {
    setCommandClient(createFakeCommandClient());

    await expect(callCommand('thoughts_list')).rejects.toMatchObject({
      code: 'unknown_command',
    });
  });
});

describe('toCommandError', () => {
  it('passes an already structured error through unchanged', () => {
    const error = { code: 'ai_disabled', message: 'AI is off' };

    expect(toCommandError(error)).toBe(error);
  });

  it('labels anything else as unknown', () => {
    expect(toCommandError(new Error('boom'))).toEqual({ code: 'unknown', message: 'boom' });
  });
});
