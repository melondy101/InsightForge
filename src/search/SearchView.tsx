import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { searchApi, type EvidenceCard, type FragmentContext } from './api';
import { referencesApi } from '../references/api';
import { sourcesApi, type IndexStatus } from '../sources/api';
import { useAppState } from '../AppStateContext';
import { toCommandError, type CommandErrorShape } from '../native/command-client';
import { errorText } from '../ui/error-text';

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
  const [error, setError] = useState<CommandErrorShape | null>(null);
  const [context, setContext] = useState<FragmentContext | null>(null);
  const [pinnedFragmentId, setPinnedFragmentId] = useState<number | null>(null);
  const [pinError, setPinError] = useState<string | null>(null);

  /** 索引是否已完成且 FTS 可搜索（时序契约）。 */
  const [indexStatus, setIndexStatus] = useState<IndexStatus | null>(null);
  const [indexLoading, setIndexLoading] = useState(false);
  const [indexError, setIndexError] = useState<string | null>(null);

  /** sourceId 切换时重新 await；不依赖 BackfillCompleted 事件（Tauri dev 模式下事件可能丢失）。 */
  useEffect(() => {
    let cancelled = false;
    setIndexStatus(null);
    setIndexLoading(true);
    setIndexError(null);
    setResults([]);
    setQuery('');
    setContext(null);

    sourcesApi
      .awaitIndexCommitted(sourceId)
      .then((status) => {
        if (cancelled) return;
        setIndexStatus(status);
        setIndexLoading(false);
      })
      .catch((err) => {
        if (cancelled) return;
        setIndexError(err instanceof Error ? err.message : String(err));
        setIndexLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [sourceId]);

  const searchReady = indexStatus?.searchable ?? false;
  const canSearch = !loading && !indexLoading && query.trim().length > 0 && searchReady;

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
      setError(toCommandError(err));
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
          {errorText(t, error, 'search')}
        </p>
      ) : null}

      {indexError ? (
        <p className="search__error" data-el="search-index-error" role="alert">
          {indexError}
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
          placeholder={indexLoading ? t('search.indexingPlaceholder') : t('search.placeholder')}
          disabled={indexLoading || !searchReady}
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
          {indexLoading
            ? t('search.indexing')
            : searchReady
              ? t('search.submit')
              : t('search.notSearchable')}
        </button>
      </div>

      {indexLoading ? (
        <p data-el="search-index-loading" className="search__status">
          {t('search.indexLoading')}
        </p>
      ) : indexStatus && !searchReady ? (
        <p data-el="search-index-not-searchable" className="search__status">
          {t('search.notSearchable')}
        </p>
      ) : null}

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
              <li className="evi" key={card.fragmentId} data-el="search-card">
                <div className="evi__head">
                  <strong data-el="search-card-source" className="evi__src">{card.sourceName}</strong>
                  <span data-el="search-card-path" className="evi__path">{card.relPath}</span>
                </div>
                <p
                  className="evi__snip"
                  data-el="search-card-snippet"
                  dangerouslySetInnerHTML={{ __html: card.snippet }}
                />
                <div className="evi__foot">
                  <span data-el="search-card-rank" className="evi__rank">
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
