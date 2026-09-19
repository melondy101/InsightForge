import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { CoachTurn } from './api';
import { useCoach } from './useCoach';
import { errorText, type Translate } from '../ui/error-text';

interface TurnRowProps {
  turn: CoachTurn;
  t: Translate;
  onMarkHelpful: (turnId: string, helpful: boolean) => void;
}

function TurnRow({ turn, t, onMarkHelpful }: TurnRowProps) {
  const isCoach = turn.role === 'coach';
  return (
    <li
      className={isCoach ? 'coach-turn coach-turn--coach' : 'coach-turn coach-turn--user'}
      data-el="coach-turn"
      data-role={turn.role}
    >
      <div className="coach-turn__meta" data-el="coach-turn-meta">
        <span data-el="coach-turn-round">{t('coach.round', { round: turn.roundNumber })}</span>
        {turn.action ? (
          <span className="coach-turn__action" data-el="coach-turn-action">
            {t(`coach.action.${turn.action}`)}
          </span>
        ) : null}
      </div>
      <p className="coach-turn__body" data-el="coach-turn-body">
        {turn.content}
      </p>
      {isCoach ? (
        <div className="coach-turn__foot">
          <button
            type="button"
            data-el="coach-helpful-btn"
            aria-pressed={turn.helpful}
            onClick={() => onMarkHelpful(turn.id, !turn.helpful)}
          >
            {turn.helpful ? t('coach.helpfulOn') : t('coach.helpful')}
          </button>
        </div>
      ) : null}
    </li>
  );
}

/**
 * 陪练面板：作者说「我卡住了」才开始，最多三轮后把笔还给作者。
 * 反代写：这里只显示提示、问题与微任务，不出现可插入正文的「补全」按钮。
 */
export function CoachPanel({ draftId }: { draftId: string }) {
  const { t } = useTranslation();
  const { session, turns, busy, error, actions } = useCoach(draftId);
  const [draft, setDraft] = useState('');

  const roundsLeft = session === null ? 0 : session.maxRounds - session.currentRound;
  const closed = session === null ? false : session.state === 'completed' || session.state === 'archived';

  async function submit() {
    const message = draft.trim();
    if (!message) return;
    setDraft('');
    await actions.send(message);
  }

  return (
    <section className="coach" data-el="coach-panel">
      <h2 data-el="coach-title">{t('coach.title')}</h2>
      <p className="coach__rule" data-el="coach-rule">
        {t('coach.rule')}
      </p>

      {error ? (
        <p className="coach__error" data-el="coach-error" role="alert">
          {errorText(t, error, 'coach')}
          <button type="button" data-el="coach-error-dismiss" onClick={actions.dismissError}>
            {t('coach.dismiss')}
          </button>
        </p>
      ) : null}

      {session === null ? (
        <div className="coach__start" data-el="coach-start-block">
          <p data-el="coach-start-hint">{t('coach.startHint')}</p>
          <button
            type="button"
            data-el="coach-start-btn"
            disabled={busy}
            onClick={() => void actions.start()}
          >
            {t('coach.start')}
          </button>
        </div>
      ) : (
        <>
          <div className="coach__head" data-el="coach-head">
            <span data-el="coach-rounds">
              {t('coach.rounds', {
                round: session.currentRound,
                max: session.maxRounds,
                left: roundsLeft,
              })}
            </span>
            <button
              type="button"
              data-el="coach-archive-btn"
              disabled={busy || session.state === 'archived'}
              onClick={() => void actions.archive()}
            >
              {t('coach.archive')}
            </button>
          </div>

          {turns.length > 0 ? (
            <ul className="coach__turns" data-el="coach-turns">
              {turns.map((turn) => (
                <TurnRow
                  key={turn.id}
                  turn={turn}
                  t={t}
                  onMarkHelpful={(turnId, helpful) => void actions.markHelpful(turnId, helpful)}
                />
              ))}
            </ul>
          ) : (
            <p data-el="coach-turns-empty">{t('coach.turnsEmpty')}</p>
          )}

          {closed ? (
            <div className="coach__closed" data-el="coach-closed">
              <p>{session.state === 'archived' ? t('coach.archived') : t('coach.roundsDone')}</p>
              <button
                type="button"
                data-el="coach-restart-btn"
                disabled={busy}
                onClick={() => void actions.start()}
              >
                {t('coach.again')}
              </button>
            </div>
          ) : (
            <div className="coach__compose" data-el="coach-compose">
              <textarea
                data-el="coach-input"
                value={draft}
                rows={3}
                placeholder={t('coach.placeholder')}
                onChange={(event) => setDraft(event.target.value)}
              />
              <div className="coach__compose-foot">
                <button
                  type="button"
                  data-el="coach-send-btn"
                  disabled={busy || draft.trim().length === 0}
                  onClick={() => void submit()}
                >
                  {t('coach.send')}
                </button>
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}
