import { describe, expect, it } from 'vitest';
import { act, renderHook, waitFor } from '@testing-library/react';

import { createFakeCommandClient, setCommandClient } from '../native/command-client';
import type { CoachSession, CoachTurn } from './api';
import { useCoach } from './useCoach';

const STAMP = '2026-09-19T00:00:00.000000Z';

const SESSION: CoachSession = {
  id: 's-1',
  draftId: 'draft-1',
  state: 'active',
  currentRound: 1,
  maxRounds: 3,
  createdAt: STAMP,
  updatedAt: STAMP,
};

function turn(role: 'user' | 'coach', content: string): CoachTurn {
  return {
    id: `t-${role}-${content.length}`,
    sessionId: 's-1',
    draftId: 'draft-1',
    role,
    content,
    action: role === 'coach' ? 'question' : null,
    roundNumber: 1,
    createdAt: STAMP,
    helpful: false,
  };
}

describe('useCoach', () => {
  it('挂载时不自己开会话，只查一次该草稿的最近会话', async () => {
    const client = createFakeCommandClient({ coach_latest_session: () => null });
    setCommandClient(client);

    const { result } = renderHook(() => useCoach('draft-1'));
    await waitFor(() => expect(client.calls).toHaveLength(1));

    expect(result.current.session).toBeNull();
    expect(client.calls[0].command).toBe('coach_latest_session');
  });

  it('草稿上次陪练过，挂载就把会话与轮次读回来', async () => {
    setCommandClient(
      createFakeCommandClient({
        coach_latest_session: () => SESSION,
        coach_get_session: () => SESSION,
        coach_turns: () => [turn('user', '我卡住了'), turn('coach', 'Question: 结论是什么？')],
      }),
    );

    const { result } = renderHook(() => useCoach('draft-1'));
    await waitFor(() => expect(result.current.turns).toHaveLength(2));

    expect(result.current.session?.id).toBe('s-1');
  });

  it('作者那一轮也回到记录里：coach_respond 只回陪练轮，轮次一律从库重读', async () => {
    let turns = [turn('user', '我卡住了'), turn('coach', 'Question: 结论是什么？')];
    setCommandClient(
      createFakeCommandClient({
        coach_latest_session: () => SESSION,
        coach_get_session: () => SESSION,
        coach_turns: () => turns,
        coach_respond: () => {
          const coach = turn('coach', 'Question: 读者为什么要读下去？');
          turns = [...turns, turn('user', '因为这里有个反直觉的事实'), coach];
          return coach;
        },
      }),
    );

    const { result } = renderHook(() => useCoach('draft-1'));
    await waitFor(() => expect(result.current.turns).toHaveLength(2));

    await act(async () => {
      await result.current.actions.send('因为这里有个反直觉的事实');
    });

    expect(result.current.turns.map((row) => row.role)).toEqual([
      'user',
      'coach',
      'user',
      'coach',
    ]);
  });

  it('AI 总开关关掉时给出错误码，由视图映射成文案', async () => {
    const client = createFakeCommandClient({
      coach_latest_session: () => null,
      coach_start: () => {
        throw { code: 'ai_disabled', message: 'AI is switched off' };
      },
    });
    setCommandClient(client);

    const { result } = renderHook(() => useCoach('draft-1'));
    await waitFor(() => expect(client.calls).toHaveLength(1));

    await act(async () => {
      await result.current.actions.start();
    });

    expect(result.current.error?.code).toBe('ai_disabled');
    expect(result.current.session).toBeNull();
  });

  it('标「有帮助」再读回，那一轮就是已标记；再点一次就取消', async () => {
    let helpful = false;
    setCommandClient(
      createFakeCommandClient({
        coach_latest_session: () => SESSION,
        coach_get_session: () => SESSION,
        coach_turns: () => [{ ...turn('coach', 'Question: 结论是什么？'), helpful }],
        coach_mark_helpful: (args) => {
          helpful = args.helpful === true;
        },
      }),
    );

    const { result } = renderHook(() => useCoach('draft-1'));
    await waitFor(() => expect(result.current.turns).toHaveLength(1));
    const coachTurnId = result.current.turns[0].id;

    await act(async () => {
      await result.current.actions.markHelpful(coachTurnId, true);
    });
    expect(result.current.turns[0].helpful).toBe(true);

    await act(async () => {
      await result.current.actions.markHelpful(coachTurnId, false);
    });
    expect(result.current.turns[0].helpful).toBe(false);
  });
});
