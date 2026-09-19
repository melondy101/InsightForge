import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
  appVersion,
  callCommand,
  createFakeCommandClient,
  createHttpCommandClient,
  selectCommandClient,
  setCommandClient,
  tauriCommandClient,
  toCommandError,
} from './command-client';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: vi.fn(async () => '0.1.0'),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
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

  it('answers the version of whichever channel is installed', async () => {
    const restore = setCommandClient(createFakeCommandClient({}, '3.2.1'));
    await expect(appVersion()).resolves.toBe('3.2.1');

    restore();
    await expect(appVersion()).resolves.toBe('0.1.0');
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

describe('http command client', () => {
  const fetchMock = vi.fn();

  beforeEach(() => {
    fetchMock.mockReset();
  });

  function client(overrides: Partial<Parameters<typeof createHttpCommandClient>[0]> = {}) {
    return createHttpCommandClient({
      baseUrl: 'https://sync.example.test/',
      version: '9.9.9',
      fetchImpl: fetchMock as unknown as typeof fetch,
      ...overrides,
    });
  }

  it('posts the command name and its args to the rpc endpoint', async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ id: 'd1' }), { status: 200 }));

    await expect(client().call('drafts_open', { filename: 'a.md' })).resolves.toEqual({ id: 'd1' });

    const [url, init] = fetchMock.mock.calls[0];
    expect(url).toBe('https://sync.example.test/rpc/drafts_open');
    expect(init.method).toBe('POST');
    expect(init.body).toBe(JSON.stringify({ filename: 'a.md' }));
  });

  it('answers the app version from its own configuration, without a Tauri runtime', async () => {
    await expect(client().appVersion()).resolves.toBe('9.9.9');
    expect(fetchMock).not.toHaveBeenCalled();
  });

  it('carries the supplied credential on every request', async () => {
    fetchMock.mockResolvedValue(new Response('null', { status: 200 }));

    await client({ token: () => 'tok-1' }).call('drafts_list');

    const [, init] = fetchMock.mock.calls[0];
    expect(init.headers.authorization).toBe('Bearer tok-1');
  });

  it('surfaces a structured server error unchanged', async () => {
    fetchMock.mockResolvedValue(
      new Response(JSON.stringify({ code: 'conflict', message: 'rev diverged' }), { status: 409 }),
    );

    await expect(client().call('sync_push')).rejects.toEqual({
      code: 'conflict',
      message: 'rev diverged',
    });
  });

  it('names an unstructured failure after the status it came with', async () => {
    fetchMock.mockResolvedValue(new Response('gateway exploded', { status: 502 }));

    await expect(client().call('drafts_list')).rejects.toMatchObject({
      code: 'http_error',
      message: 'gateway exploded',
    });
  });

  it('reports an unreachable runtime instead of hanging the caller', async () => {
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));

    await expect(client().call('drafts_list')).rejects.toMatchObject({
      code: 'network_unreachable',
    });
  });

  it('aborts a request that outlives the timeout', async () => {
    fetchMock.mockImplementation(
      (_url: string, init: { signal: AbortSignal }) =>
        new Promise((_resolve, reject) => {
          init.signal.addEventListener('abort', () =>
            reject(Object.assign(new Error('aborted'), { name: 'AbortError' })),
          );
        }),
    );

    await expect(client({ timeoutMs: 5 }).call('drafts_list')).rejects.toMatchObject({
      code: 'timeout',
    });
  });
});

describe('runtime selection', () => {
  it('keeps the tauri channel when the desktop bridge is present', () => {
    const selected = selectCommandClient({
      env: { VITE_API_BASE_URL: 'https://sync.example.test' },
      hasTauriBridge: true,
    });

    expect(selected).toBe(tauriCommandClient);
  });

  it('chooses the http channel when an api base is configured', async () => {
    const selected = selectCommandClient({
      env: { VITE_API_BASE_URL: 'https://sync.example.test', VITE_APP_VERSION: '1.2.3' },
      hasTauriBridge: false,
    });

    await expect(selected.appVersion()).resolves.toBe('1.2.3');
  });

  it('fails loudly in a browser that was built without an api base', async () => {
    const selected = selectCommandClient({ env: {}, hasTauriBridge: false });

    await expect(selected.call('drafts_list')).rejects.toMatchObject({
      code: 'runtime_unconfigured',
    });
  });
});

describe('event channel behind the same seam', () => {
  beforeEach(() => {
    vi.mocked(listen).mockReset();
    setCommandClient(tauriCommandClient);
  });

  it('hands the desktop event payload to the subscriber and closes on unlisten', async () => {
    const unlisten = vi.fn();
    vi.mocked(listen).mockImplementation(async (_channel, handler) => {
      handler({ payload: { type: 'file_indexed', processed: 3 } } as never);
      return unlisten;
    });
    const seen: unknown[] = [];

    const close = tauriCommandClient.subscribe('sources://progress', (payload) =>
      seen.push(payload),
    );
    await Promise.resolve();

    expect(seen).toEqual([{ type: 'file_indexed', processed: 3 }]);
    close();
    expect(unlisten).toHaveBeenCalled();
  });

  it('still unsubscribes when the listener resolves after the component is gone', async () => {
    const unlisten = vi.fn();
    let resolveListen: (fn: () => void) => void = () => {};
    vi.mocked(listen).mockReturnValue(
      new Promise((resolve) => {
        resolveListen = resolve;
      }),
    );

    const close = tauriCommandClient.subscribe('sources://progress', () => {});
    close();
    resolveListen(unlisten);
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalled();
  });

  it('lets a fake runtime push events to subscribers', () => {
    const fake = createFakeCommandClient();
    const seen: unknown[] = [];
    const close = fake.subscribe('sources://progress', (payload) => seen.push(payload));

    fake.emit('sources://progress', { type: 'backfill_started' });
    close();
    fake.emit('sources://progress', { type: 'ignored_after_close' });

    expect(seen).toEqual([{ type: 'backfill_started' }]);
  });

  it('reads web runtime events off the server stream', () => {
    const stream: {
      onmessage: ((event: { data: string }) => void) | null;
      close: () => void;
    } = { onmessage: null, close: vi.fn() };
    const opened: string[] = [];
    const client = createHttpCommandClient({
      baseUrl: 'https://sync.example.test',
      version: '9.9.9',
      eventSource: (url) => {
        opened.push(url);
        return stream;
      },
    });
    const seen: unknown[] = [];

    const close = client.subscribe('sources://progress', (payload) => seen.push(payload));
    stream.onmessage?.({ data: JSON.stringify({ type: 'file_indexed' }) });
    stream.onmessage?.({ data: 'not json' });
    close();

    expect(opened).toEqual(['https://sync.example.test/events/sources%3A%2F%2Fprogress']);
    expect(seen).toEqual([{ type: 'file_indexed' }]);
    expect(stream.close).toHaveBeenCalled();
  });
});
