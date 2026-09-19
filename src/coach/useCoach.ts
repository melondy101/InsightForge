import { useCallback, useEffect, useState } from 'react';

import { coachApi, type CoachSession, type CoachTurn } from './api';
import { toCommandError, type CommandErrorShape } from '../native/command-client';

export interface CoachModel {
  session: CoachSession | null;
  turns: CoachTurn[];
  busy: boolean;
  error: CommandErrorShape | null;
  actions: {
    /** 作者显式说「我卡住了」才开会话；不会自己冒出来。 */
    start: () => Promise<void>;
    send: (userMessage: string) => Promise<void>;
    markHelpful: (turnId: string, helpful: boolean) => Promise<void>;
    archive: () => Promise<void>;
    dismissError: () => void;
  };
}

/**
 * 陪练会话视图模型。
 *
 * 轮次一律从库里重读而不是本地拼：`coach_respond` 只回陪练那一轮，
 * 作者的话也进了库——本地拼会让对话少一半。
 */
export function useCoach(draftId: string): CoachModel {
  const [session, setSession] = useState<CoachSession | null>(null);
  const [turns, setTurns] = useState<CoachTurn[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<CommandErrorShape | null>(null);

  const load = useCallback(
    async (target: CoachSession) => {
      const [fresh, rows] = await Promise.all([
        coachApi.getSession(target.id),
        coachApi.turns(target.id),
      ]);
      setSession(fresh);
      setTurns(rows);
    },
    [],
  );

  useEffect(() => {
    let cancelled = false;
    setSession(null);
    setTurns([]);
    if (!draftId) return;
    void (async () => {
      try {
        const latest = await coachApi.latestSession(draftId);
        if (latest && !cancelled) await load(latest);
      } catch (err) {
        if (!cancelled) setError(toCommandError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [draftId, load]);

  const run = useCallback(
    async (action: () => Promise<void>) => {
      setBusy(true);
      setError(null);
      try {
        await action();
      } catch (err) {
        setError(toCommandError(err));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const actions = {
    start: () =>
      run(async () => {
        const created = await coachApi.start(draftId);
        setSession(created);
        setTurns(await coachApi.turns(created.id));
      }),
    send: (userMessage: string) =>
      run(async () => {
        if (!session) return;
        await coachApi.respond({ sessionId: session.id, draftId, userMessage });
        await load(session);
      }),
    markHelpful: (turnId: string, helpful: boolean) =>
      run(async () => {
        if (!session) return;
        await coachApi.markHelpful(turnId, helpful);
        setTurns(await coachApi.turns(session.id));
      }),
    archive: () =>
      run(async () => {
        if (!session) return;
        await coachApi.archive(session.id);
        await load(session);
      }),
    dismissError: () => setError(null),
  };

  return { session, turns, busy, error, actions };
}
