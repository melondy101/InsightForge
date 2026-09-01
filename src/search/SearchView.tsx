import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { searchApi, type EvidenceCard, type FragmentContext, type SearchErrorShape, toSearchError } from './api';
import { referencesApi } from '../references/api';
import { useAppState } from '../AppStateContext';

function errorText(
  t: (key: string, options?: Record<string, unknown>) => string,
  error: SearchErrorShape,
): string {
  const known = [
    'repository_error',
    'source_not_found',
    'invalid_query',
  ];
  if (known.includes(error.code)) {
    return t(`search.error.${error.code}`);
  }
  return t('search.error.generic', { message: error.message });
}

interface SearchViewProps {
  sourceId: string;
  sourceName: string;
}

export function SearchView({ sourceId, sourceName }: SearchViewProps) {
  const { t } = useTranslation();
  const { activeDraft } = useAppState();
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<EvidenceCard[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<SearchErrorShape | null>(null);
  const [context, setContext] = useState<FragmentContext | null>(null);
  const [pinnedFragmentId, setPinnedFragmentId] = useState<number | null>(null);
  const [pinError, setPinError] = useState<string | null>(null);

  const canSearch = !loading && query.trim().length > 0;

  async function handleSearch() {
    if (!canSearch) {
      return;
    }
    setLoading(true);
    setError(null);
    setContext(null);
    try {
      const cards = await searchApi.query(sourceId, query.trim(), 20);
      setResults(cards);
    } catch (err) {
      setError(toSearchError(err));
      setResults([]);
    } finally {
      setLoading(false);
    }
  }

  async function handleOpenContext(card: EvidenceCard) {
    setLoading(true);
    setError(null);
    try {
      const ctx = await searchApi.openContext(card.sourceId, card.fragmentId);
      setContext(ctx);
    } catch (err) {
      setError({ code: 'unknown', message: String(err) });
      setContext(null);
    } finally {
      setLoading(false);
    }
  }

  async function handlePinToDraft(card: EvidenceCard) {
    if (!activeDraft) {
      setPinError(t('search.pinNoDraft'));
      return;
    }
    setPinError(null);
    try {
      await referencesApi.create(activeDraft.id, card.fragmentId);
      setPinnedFragmentId(card.fragmentId);
    } catch (err) {
      setPinError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <div className="search" data-el="search-view">
      <h2 data-el="search-title">{t('search.title')}</h2>
      <p data-el="search-source">{t('search.sourceLabel', { name: sourceName })}</p>

      {error ? (
        <p className="search__error" data-el="search-error" role="alert">
          {errorText(t, error)}
        </p>
      ) : null}

      {pinError ? (
        <p className="search__error" data-el="search-pin-error" role="alert">
          {pinError}
        </p>
      ) : null}

      <div className="search__form" data-el="search-form">
        <input
          data-el="search-input"
          value={query}
          placeholder={t('search.placeholder')}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') {
              handleSearch();
            }
          }}
        />
        <button
          type="button"
          data-el="search-submit"
          disabled={!canSearch}
          onClick={handleSearch}
        >
          {t('search.submit')}
        </button>
      </div>

      {context ? (
        <section className="search__context" data-el="search-context">
          <h3 data-el="search-context-title">{t('search.contextTitle')}</h3>
          <p data-el="search-context-path">
            {context.sourceName} / {context.relPath}
            {context.headingPath ? ` / ${context.headingPath}` : ''}
          </p>
          <pre data-el="search-context-text">{context.text}</pre>
          <button type="button" data-el="search-context-close" onClick={() => setContext(null)}>
            {t('search.contextClose')}
          </button>
        </section>
      ) : null}

      <section className="search__results" data-el="search-results">
        {results.length === 0 ? (
          <p data-el="search-empty">{loading ? t('search.loading') : t('search.empty')}</p>
        ) : (
          <ul>
            {results.map((card) => (
              <li className="search__card" key={card.fragmentId} data-el="search-card">
                <div className="search__card-header">
                  <strong data-el="search-card-source">{card.sourceName}</strong>
                  <span data-el="search-card-path">{card.relPath}</span>
                </div>
                <p
                  className="search__card-snippet"
                  data-el="search-card-snippet"
                  dangerouslySetInnerHTML={{ __html: card.snippet }}
                />
                <div className="search__card-footer">
                  <span data-el="search-card-rank">
                    {t('search.rank', { bm25: card.bm25.toFixed(2) })}
                  </span>
                  <button
                    type="button"
                    data-el="search-card-context"
                    onClick={() => handleOpenContext(card)}
                  >
                    {t('search.openContext')}
                  </button>
                  <button
                    type="button"
                    data-el="search-card-pin"
                    disabled={!activeDraft}
                    onClick={() => handlePinToDraft(card)}
                  >
                    {pinnedFragmentId === card.fragmentId ? t('search.pinSuccess') : t('search.pinToDraft')}
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}
