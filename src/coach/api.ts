import { callCommand } from '../native/command-client';

/**
 * coach 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/coach.rs 的 serde 输出一一对应。
 */

export type CoachSessionState = 'active' | 'waiting_for_coach' | 'completed' | 'archived';

/** 五类动作白名单（铁律 2：只提示、不改写）。 */
export type CoachAction = 'question' | 'evidence' | 'tension' | 'directions' | 'micro_task';

export interface CoachSession {
  id: string;
  draftId: string;
  state: CoachSessionState;
  currentRound: number;
  maxRounds: number;
  createdAt: string;
  updatedAt: string;
}

export interface CoachTurn {
  id: string;
  sessionId: string;
  draftId: string;
  role: 'user' | 'coach';
  content: string;
  action: CoachAction | null;
  roundNumber: number;
  createdAt: string;
  helpful: boolean;
}

export interface CoachTurnInput {
  sessionId: string;
  draftId: string;
  userMessage: string;
}

export const coachApi = {
  start: (draftId: string) => callCommand<CoachSession>('coach_start', { draftId }),
  respond: (input: CoachTurnInput) => callCommand<CoachTurn>('coach_respond', { input }),
  getSession: (sessionId: string) => callCommand<CoachSession>('coach_get_session', { sessionId }),
  /** 该草稿最近一次会话；从未陪练过则为 null。 */
  latestSession: (draftId: string) =>
    callCommand<CoachSession | null>('coach_latest_session', { draftId }),
  turns: (sessionId: string) => callCommand<CoachTurn[]>('coach_turns', { sessionId }),
  markHelpful: (turnId: string, helpful: boolean) =>
    callCommand<void>('coach_mark_helpful', { turnId, helpful }),
  archive: (sessionId: string) => callCommand<void>('coach_archive', { sessionId }),
};
