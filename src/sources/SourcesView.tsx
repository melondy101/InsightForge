import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import type { SourceSummary } from './api';
import { SYNC_INTERVAL_CHOICES } from './api';
import { progressPercent } from './progress';
import { useSources } from './useSources';
import { type CommandErrorShape } from '../native/command-client';

/** 错误 code → i18n key；未知 code 回退到 generic（带 Rust message 供诊断）。 */
function errorText(t: (key: string, options?: Record<string, unknown>) => string, error: CommandErrorShape): string {
  const known = [
    'invalid_name',
    'duplicate_root',
    'source_not_found',
    'source_paused',
    'already_indexing',
    'invalid_rel_path',
    'invalid_root',
    'root_not_a_directory',
    'invalid_ignore_glob',
    'invalid_sync_interval',
    'repository_error',
    'io_error',
    'cancelled',
  ];
  if (known.includes(error.code)) {
    return t(`sources.error.${error.code}`);
  }
  return t('sources.error.generic', { message: error.message });
}

interface AddFormProps {
  onAdd: (root: string, name: string) => void;
  onPick: () => Promise<string | null>;
  defaultIgnoreGlobs: readonly string[];
  busy: boolean;
}

function AddForm({ onAdd, onPick, defaultIgnoreGlobs, busy }: AddFormProps) {
  const { t } = useTranslation();
  const [name, setName] = useState('');
  const [rootPath, setRootPath] = useState('');
  const [picking, setPicking] = useState(false);

  const canSubmit = !busy && name.trim().length > 0 && rootPath.trim().length > 0;

  return (
    <section className="sources__add" data-el="sources-add">
      <h3>{t('sources.addTitle')}</h3>
      <label className="sources__field">
        <span>{t('sources.nameLabel')}</span>
        <input
          data-el="sources-add-name"
          value={name}
          placeholder={t('sources.namePlaceholder')}
          onChange={(event) => setName(event.target.value)}
        />
      </label>
      <label className="sources__field">
        <span>{t('sources.pathLabel')}</span>
        <div className="sources__field-row">
          <input
            data-el="sources-add-path"
            value={rootPath}
            placeholder={t('sources.pathPlaceholder')}
            onChange={(event) => setRootPath(event.target.value)}
          />
          <button
            type="button"
            data-el="sources-add-pick"
            disabled={picking || busy}
            onClick={async () => {
              setPicking(true);
              const picked = await onPick();
              if (picked) {
                setRootPath(picked);
              }
              setPicking(false);
            }}
          >
            {t('sources.pickDirectory')}
          </button>
        </div>
      </label>
      <p className="sources__globs" data-el="sources-default-globs">
        {t('sources.defaultIgnoreTitle')}：{defaultIgnoreGlobs.join('、')}
      </p>
      <button
        type="button"
        data-el="sources-add-submit"
        disabled={!canSubmit}
        onClick={() => {
          onAdd(rootPath.trim(), name.trim());
          setName('');
          setRootPath('');
        }}
      >
        {t('sources.addButton')}
      </button>
    </section>
  );
}

interface SourceRowProps {
  source: SourceSummary;
  progress: { processed: number; totalFiles: number } | undefined;
  t: (key: string, options?: Record<string, unknown>) => string;
  onPause: (id: string, paused: boolean) => void;
  onSync: (id: string) => void;
  onReconcile: (id: string) => void;
  onRebuildIndex: (id: string) => void;
  onRemove: (id: string) => void;
  onSetSyncInterval: (id: string, intervalMinutes: number) => void;
}

function SourceRow({
  source,
  progress,
  t,
  onPause,
  onSync,
  onReconcile,
  onRebuildIndex,
  onRemove,
  onSetSyncInterval,
}: SourceRowProps) {
  const status = source.paused
    ? t('sources.status.paused')
    : source.status === 'indexing'
      ? t('sources.status.indexing')
      : t('sources.status.idle');
  const current = progress ?? { processed: 0, totalFiles: 0 };
  const percent = progressPercent(current.processed, current.totalFiles);

  return (
    <li className="sources__row" data-el="sources-row">
      <span data-el="sources-row-name">{source.name}</span>
      <span className="sources__row-path" data-el="sources-row-path">
        {source.rootPath}
      </span>
      <span data-el="sources-row-status">{status}</span>
      <span data-el="sources-row-filecount">{source.fileCount}</span>
      <span className="sources__row-progress" data-el="sources-row-progress">
        {progress ? (
          <>
            <progress data-el="sources-progress-bar" value={percent} max={100} />
            <span data-el="sources-progress-label">
              {t('sources.progress', {
                processed: current.processed,
                total: current.totalFiles,
              })}
            </span>
          </>
        ) : null}
      </span>
      <span className="sources__row-actions" data-el="sources-row-actions">
        <label className="sources__interval" data-el="sources-row-interval">
          <span>{t('sources.syncInterval')}</span>
          <select
            data-el="sources-interval-select"
            aria-label={t('sources.syncInterval')}
            value={source.syncIntervalMinutes}
            onChange={(event) => onSetSyncInterval(source.id, Number(event.target.value))}
          >
            {SYNC_INTERVAL_CHOICES.map((minutes) => (
              <option key={minutes} value={minutes}>
                {t('sources.syncIntervalMinutes', { minutes })}
              </option>
            ))}
          </select>
        </label>
        <button type="button" data-el="sources-sync-btn" onClick={() => onSync(source.id)}>
          {t('sources.sync')}
        </button>
        <button type="button" data-el="sources-reconcile-btn" onClick={() => onReconcile(source.id)}>
          {t('sources.reconcile')}
        </button>
        <button type="button" data-el="sources-rebuild-btn" onClick={() => onRebuildIndex(source.id)}>
          {t('sources.rebuildIndex')}
        </button>
        {source.paused ? (
          <button type="button" data-el="sources-resume-btn" onClick={() => onPause(source.id, false)}>
            {t('sources.resume')}
          </button>
        ) : (
          <button type="button" data-el="sources-pause-btn" onClick={() => onPause(source.id, true)}>
            {t('sources.pause')}
          </button>
        )}
        <button
          type="button"
          data-el="sources-remove-btn"
          onClick={() => {
            if (window.confirm(t('sources.removeConfirm'))) {
              onRemove(source.id);
            }
          }}
        >
          {t('sources.remove')}
        </button>
      </span>
    </li>
  );
}

/** 来源管理视图：目录授权、忽略规则展示、来源列表与事件驱动的索引进度。 */
export function SourcesView() {
  const { t } = useTranslation();
  const { sources, progress, defaultIgnoreGlobs, error, actions } = useSources();

  return (
    <div className="sources" data-el="sources-view">
      <h2 data-el="sources-title">{t('sources.title')}</h2>
      {error ? (
        <p className="sources__error" data-el="sources-error" role="alert">
          {errorText(t, error)}
          <button type="button" data-el="sources-error-dismiss" onClick={actions.dismissError}>
            {t('sources.dismissError')}
          </button>
        </p>
      ) : null}

      <AddForm
        onAdd={(root, name) => void actions.addSource(root, name)}
        onPick={actions.pickDirectory}
        defaultIgnoreGlobs={defaultIgnoreGlobs}
        busy={false}
      />

      <section className="sources__list" data-el="sources-list">
        <h3>{t('sources.listTitle')}</h3>
        {sources.length === 0 ? (
          <p data-el="sources-empty">{t('sources.empty')}</p>
        ) : (
          <>
            <div className="sources__columns" data-el="sources-columns">
              <span>{t('sources.columnName')}</span>
              <span>{t('sources.columnPath')}</span>
              <span>{t('sources.columnStatus')}</span>
              <span>{t('sources.columnFiles')}</span>
              <span>{t('sources.columnProgress')}</span>
              <span>{t('sources.columnActions')}</span>
            </div>
            <ul>
              {sources.map((source) => (
                <SourceRow
                  key={source.id}
                  source={source}
                  progress={progress[source.id]}
                  t={t}
                  onPause={(id, paused) => void actions.pause(id, paused)}
                  onSync={(id) => void actions.sync(id)}
                  onReconcile={(id) => void actions.reconcile(id)}
                  onRebuildIndex={(id) => void actions.rebuildIndex(id)}
                  onRemove={(id) => void actions.remove(id)}
                  onSetSyncInterval={(id, minutes) => void actions.setSyncInterval(id, minutes)}
                />
              ))}
            </ul>
          </>
        )}
      </section>
    </div>
  );
}
