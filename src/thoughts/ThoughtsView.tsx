import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useThoughts } from './useThoughts';

export function ThoughtsView() {
  const { t } = useTranslation();
  const { cards, error, actions } = useThoughts();

  const [idea, setIdea] = useState('');
  const [sourceId, setSourceId] = useState('');
  const [sourceName, setSourceName] = useState('');
  const [relPath, setRelPath] = useState('');
  const [headingPath, setHeadingPath] = useState('');
  const [draftId, setDraftId] = useState('');
  const [draftFilename, setDraftFilename] = useState('');

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!idea.trim() || !draftFilename.trim()) {
      return;
    }
    await actions.upsert({
      idea: idea.trim(),
      sourceId: sourceId.trim() || '__unknown__',
      sourceName: sourceName.trim() || 'Unknown source',
      relPath: relPath.trim() || '__unknown__',
      headingPath: headingPath.trim() || '__unknown__',
      draftId: draftId.trim() || '__unknown__',
      draftFilename: draftFilename.trim(),
    });
    setIdea('');
    setSourceId('');
    setSourceName('');
    setRelPath('');
    setHeadingPath('');
    setDraftId('');
    setDraftFilename('');
  };

  return (
    <section className="thoughts-view" data-el="thoughts-view">
      <h3>{t('thoughts.title')}</h3>

      {error && (
        <div className="thoughts-view__error" data-el="thoughts-error">
          <span>{error.message}</span>
          <button type="button" onClick={actions.dismissError}>
            {t('thoughts.dismiss')}
          </button>
        </div>
      )}

      <form className="thoughts-view__form" data-el="thoughts-form" onSubmit={handleSubmit}>
        <input
          className="thoughts-view__input"
          value={idea}
          onChange={(e) => setIdea(e.target.value)}
          placeholder={t('thoughts.ideaPlaceholder')}
          data-el="thoughts-idea"
        />
        <input
          className="thoughts-view__input"
          value={sourceId}
          onChange={(e) => setSourceId(e.target.value)}
          placeholder={t('thoughts.sourceIdPlaceholder')}
          data-el="thoughts-source-id"
        />
        <input
          className="thoughts-view__input"
          value={sourceName}
          onChange={(e) => setSourceName(e.target.value)}
          placeholder={t('thoughts.sourceNamePlaceholder')}
          data-el="thoughts-source-name"
        />
        <input
          className="thoughts-view__input"
          value={relPath}
          onChange={(e) => setRelPath(e.target.value)}
          placeholder={t('thoughts.relPathPlaceholder')}
          data-el="thoughts-rel-path"
        />
        <input
          className="thoughts-view__input"
          value={headingPath}
          onChange={(e) => setHeadingPath(e.target.value)}
          placeholder={t('thoughts.headingPathPlaceholder')}
          data-el="thoughts-heading-path"
        />
        <input
          className="thoughts-view__input"
          value={draftId}
          onChange={(e) => setDraftId(e.target.value)}
          placeholder={t('thoughts.draftIdPlaceholder')}
          data-el="thoughts-draft-id"
        />
        <input
          className="thoughts-view__input"
          value={draftFilename}
          onChange={(e) => setDraftFilename(e.target.value)}
          placeholder={t('thoughts.draftFilenamePlaceholder')}
          data-el="thoughts-draft-filename"
        />
        <button type="submit" data-el="thoughts-submit">
          {t('thoughts.submit')}
        </button>
      </form>

      <ul className="thoughts-view__list" data-el="thoughts-list">
        {cards.map((card) => (
          <li key={card.id} className="thoughts-view__item" data-el="thoughts-item">
            <div className="thoughts-view__meta">
              <span>{card.idea}</span>
              <span>
                {card.sourceName} / {card.relPath}
              </span>
              <span>
                {t('thoughts.draft')}: {card.draftFilename}
              </span>
            </div>
            <div className="thoughts-view__actions">
              <button type="button" onClick={() => actions.delete(card.id)}>
                {t('thoughts.delete')}
              </button>
            </div>
          </li>
        ))}
        {cards.length === 0 && (
          <li className="thoughts-view__empty" data-el="thoughts-empty">
            {t('thoughts.empty')}
          </li>
        )}
      </ul>
    </section>
  );
}
