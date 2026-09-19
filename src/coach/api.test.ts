import { beforeEach, describe, expect, it } from 'vitest';

import { createFakeCommandClient, setCommandClient } from '../native/command-client';
import { coachApi, type CoachSession, type CoachTurn } from './api';

const SESSION: CoachSession = {
  id: 's-1',
  draftId: 'draft-1',
  state: 'active',
  currentRound: 1,
  maxRounds: 3,
  createdAt: '2026-09-19T00:00:00.000000Z',
  updatedAt: '2026-09-19T00:00:00.000000Z',
};

const TURN: CoachTurn = {
  id: 't-1',
  sessionId: 's-1',
  draftId: 'draft-1',
  role: 'coach',
  content: 'Question: 这一段你想让读者带走什么？',
  action: 'question',
  roundNumber: 1,
  createdAt: '2026-09-19T00:00:00.000000Z',
  helpful: false,
};

describe('coach facade', () => {
  let client: ReturnType<typeof createFakeCommandClient>;

  beforeEach(() => {
    client = createFakeCommandClient({
      coach_start: () => SESSION,
      coach_respond: () => TURN,
      coach_get_session: () => SESSION,
      coach_latest_session: () => null,
      coach_turns: () => [TURN],
      coach_mark_helpful: () => null,
      coach_archive: () => null,
    });
    setCommandClient(client);
  });

  it('每条方法发的命令名与参数键与 Rust 侧一致', async () => {
    await coachApi.start('draft-1');
    await coachApi.respond({ sessionId: 's-1', draftId: 'draft-1', userMessage: '我不知道' });
    await coachApi.getSession('s-1');
    await coachApi.latestSession('draft-1');
    await coachApi.turns('s-1');
    await coachApi.markHelpful('t-1', true);
    await coachApi.archive('s-1');

    expect(client.calls).toEqual([
      { command: 'coach_start', args: { draftId: 'draft-1' } },
      {
        command: 'coach_respond',
        args: { input: { sessionId: 's-1', draftId: 'draft-1', userMessage: '我不知道' } },
      },
      { command: 'coach_get_session', args: { sessionId: 's-1' } },
      { command: 'coach_latest_session', args: { draftId: 'draft-1' } },
      { command: 'coach_turns', args: { sessionId: 's-1' } },
      { command: 'coach_mark_helpful', args: { turnId: 't-1', helpful: true } },
      { command: 'coach_archive', args: { sessionId: 's-1' } },
    ]);
  });

  it('草稿从未陪练过时 latestSession 回 null，而不是抛错', async () => {
    await expect(coachApi.latestSession('draft-1')).resolves.toBeNull();
  });

  it('Rust 侧的守门错误码原样上抛，交给视图映射文案', async () => {
    setCommandClient(
      createFakeCommandClient({
        coach_start: () => {
          throw { code: 'ai_disabled', message: 'AI is switched off' };
        },
      }),
    );

    await expect(coachApi.start('draft-1')).rejects.toEqual({
      code: 'ai_disabled',
      message: 'AI is switched off',
    });
  });
});
