import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { getVersion } from '@tauri-apps/api/app';

import {
  diagnosticsApi,
  formatDiagnosticBundle,
  formatEventLog,
  formatSnapshot,
  parseEventLog,
  type CommandErrorShape,
  type DiagnosticSnapshot,
  type LogEntry,
} from './api';

/**
 * 诊断面板：用户可一键复制或导出脱敏诊断快照（Phase C）。
 * 快照与事件日志均不含 API Key / 草稿正文 / 来源内容 / 用户文件路径，
 * 可放心粘贴给开发者或用户支持。
 */
export function DiagnosticsView() {
  const { t } = useTranslation();
  const [snapshot, setSnapshot] = useState<DiagnosticSnapshot | null>(null);
  const [events, setEvents] = useState<LogEntry[]>([]);
  const [error, setError] = useState<CommandErrorShape | null>(null);
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      const version = await getVersion();
      const [snap, logJson] = await Promise.all([
        diagnosticsApi.getSnapshot(version),
        diagnosticsApi.getEventLog(100),
      ]);
      setSnapshot(snap);
      setEvents(parseEventLog(logJson));
    } catch (err) {
      setError(err as CommandErrorShape);
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const bundle = snapshot ? formatDiagnosticBundle(snapshot, events) : '';

  const copy = useCallback(async () => {
    if (!bundle) return;
    try {
      await navigator.clipboard.writeText(bundle);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setError({ code: 'clipboard', message: 'copy failed' });
    }
  }, [bundle]);

  const exportFile = useCallback(() => {
    if (!bundle) return;
    const blob = new Blob([bundle], { type: 'text/plain;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const stamp = new Date().toISOString().replace(/[:.]/g, '-');
    const a = document.createElement('a');
    a.href = url;
    a.download = `insightforge-diagnostics-${stamp}.txt`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }, [bundle]);

  return (
    <div className="diagnostics" data-el="diagnostics-view">
      <h2 data-el="diagnostics-title">{t('diagnostics.title')}</h2>
      <p className="diagnostics__intro" data-el="diagnostics-intro">
        {t('diagnostics.intro')}
      </p>

      <div className="diagnostics__toolbar" data-el="diagnostics-toolbar">
        <button
          type="button"
          data-el="diagnostics-refresh"
          onClick={() => void load()}
          disabled={busy}
        >
          {t('diagnostics.refresh')}
        </button>
        <button
          type="button"
          data-el="diagnostics-copy"
          onClick={() => void copy()}
          disabled={!bundle}
        >
          {copied ? t('diagnostics.copied') : t('diagnostics.copy')}
        </button>
        <button
          type="button"
          data-el="diagnostics-export"
          onClick={exportFile}
          disabled={!bundle}
        >
          {t('diagnostics.export')}
        </button>
      </div>

      {error && (
        <p className="diagnostics__error" data-el="diagnostics-error">
          {t('diagnostics.loadError', { message: error.message })}
        </p>
      )}

      <section className="diagnostics__section" data-el="diagnostics-snapshot">
        <h3>{t('diagnostics.snapshotSection')}</h3>
        <pre data-el="diagnostics-snapshot-text">
          {snapshot ? formatSnapshot(snapshot) : '—'}
        </pre>
      </section>

      <section className="diagnostics__section" data-el="diagnostics-events">
        <h3>{t('diagnostics.eventLogSection')}</h3>
        {events.length === 0 ? (
          <p data-el="diagnostics-empty-log">{t('diagnostics.emptyLog')}</p>
        ) : (
          <pre data-el="diagnostics-events-text">{formatEventLog(events)}</pre>
        )}
      </section>
    </div>
  );
}
