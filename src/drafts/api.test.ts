import { beforeEach, describe, expect, it } from 'vitest';

import { createFakeCommandClient, setCommandClient } from '../native/command-client';
import { draftsApi } from './api';

describe('drafts facade', () => {
  let fake: ReturnType<typeof createFakeCommandClient>;

  beforeEach(() => {
    fake = createFakeCommandClient({
      drafts_save: ({ filename, content }) => ({ id: 'd1', filename, content: String(content) }),
    });
    setCommandClient(fake);
  });

  it('sends the typed command and arguments the Rust layer expects', async () => {
    await expect(draftsApi.save('a.md', '# 标题')).resolves.toMatchObject({ filename: 'a.md' });

    expect(fake.calls).toEqual([
      { command: 'drafts_save', args: { filename: 'a.md', content: '# 标题' } },
    ]);
  });

  it('surfaces a rejected command to callers as a structured error', async () => {
    setCommandClient(
      createFakeCommandClient({
        drafts_save: () => {
          throw { code: 'conflict', message: 'external edit detected' };
        },
      }),
    );

    await expect(draftsApi.save('a.md', 'x')).rejects.toEqual({
      code: 'conflict',
      message: 'external edit detected',
    });
  });
});
