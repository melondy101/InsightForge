import { useTranslation } from 'react-i18next';
import { useReferences } from './useReferences';

export function ReferencesView({ draftId }: { draftId: string }) {
  const { t } = useTranslation();
  const { references, error, actions } = useReferences(draftId);

  return (
    <section className="references-view" data-el="references-view">
      <h3>{t('references.title')}</h3>
      {error && (
        <div className="references-view__error" data-el="references-error">
          <span>{error.message}</span>
          <button type="button" onClick={actions.dismissError}>
            {t('references.dismiss')}
          </button>
        </div>
      )}
      <ul className="references-view__list" data-el="references-list">
        {references.map((ref) => (
          <li key={ref.id} className="evi" data-el="reference-item">
            <div className="evi__head">
              <span className="evi__src">{ref.sourceName}</span>
              <span className="evi__path">{ref.relPath}:{ref.startLine}-{ref.endLine}</span>
            </div>
            <p className="evi__snip">{ref.snippet}</p>
            <div className="evi__foot">
              {!ref.adopted && (
                <button type="button" onClick={() => actions.adopt(ref.id)}>
                  {t('references.adopt')}
                </button>
              )}
              {ref.sourceDeleted && (
                <span className="references-view__badge" data-el="reference-source-deleted">
                  {t('references.sourceDeleted')}
                </span>
              )}
              {ref.adopted && (
                <span className="references-view__badge" data-el="reference-adopted">
                  {t('references.adopted')}
                </span>
              )}
            </div>
          </li>
        ))}
        {references.length === 0 && (
          <li className="references-view__empty" data-el="references-empty">
            {t('references.empty')}
          </li>
        )}
      </ul>
    </section>
  );
}
