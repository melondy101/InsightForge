import { useTranslation } from 'react-i18next';
import { useEgress } from './useEgress';

export function EgressView() {
  const { t } = useTranslation();
  const { bundle, records, error, actions } = useEgress();

  return (
    <section className="egress-view" data-el="egress-view">
      <h3>{t('egress.title')}</h3>

      {error && (
        <div className="egress-view__error" data-el="egress-error">
          <span>{error.message}</span>
          <button type="button" onClick={actions.dismissError}>
            {t('egress.dismiss')}
          </button>
        </div>
      )}

      <div className="egress-view__bundle" data-el="egress-bundle">
        <h4>{t('egress.title')}</h4>
        {bundle ? (
          <div className="egress-view__bundle-content">
            <p>
              {t('egress.fragmentCount')}: {bundle.fragmentCount}
            </p>
            <ul className="egress-view__fragments" data-el="egress-fragments">
              {bundle.fragments.map((fragment, index) => (
                <li key={index} className="egress-view__fragment" data-el="egress-fragment">
                  <span className="egress-view__fragment-kind">{fragment.kind}</span>
                  <span className="egress-view__fragment-label">{fragment.label}</span>
                  <span className="egress-view__fragment-text">{fragment.text}</span>
                </li>
              ))}
            </ul>
          </div>
        ) : (
          <p className="egress-view__empty" data-el="egress-empty">
            {t('egress.empty')}
          </p>
        )}
      </div>

      <div className="egress-view__audit" data-el="egress-audit">
        <h4>Audit Log</h4>
        <ul className="egress-view__audit-list" data-el="egress-audit-list">
          {records.map((record) => (
            <li key={record.id} className="egress-view__audit-item" data-el="egress-audit-item">
              <span>{record.model}</span>
              <span>{record.fragmentCount} fragments</span>
              <span>{record.createdAt}</span>
              <span>{record.status}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  );
}
