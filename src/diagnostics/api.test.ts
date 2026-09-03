import { describe, expect, it } from 'vitest';

import {
  formatDiagnosticBundle,
  formatEventLog,
  formatSnapshot,
  parseEventLog,
  type DiagnosticSnapshot,
  type LogEntry,
} from './api';

const sampleSnapshot: DiagnosticSnapshot = {
  version: '1.2.3',
  generatedAt: '2026-01-01T00:00:00Z',
  platform: 'windows x86_64',
  sqliteVersion: '3.53.2',
  database: {
    journalMode: 'wal',
    appliedMigrations: [1, 2, 3],
    integrity: 'ok',
    tableCounts: { draft: 2, sources: 1 },
  },
};

const sampleEvents: LogEntry[] = [
  {
    ts: '2026-01-01T00:00:00Z',
    level: 'info',
    name: 'app.startup',
    fields: { sqlite: '3.53.2', journal: 'wal', migrations: '3' },
  },
  {
    ts: '2026-01-01T00:00:01Z',
    level: 'info',
    name: 'source.backfill_completed',
    fields: { source: 'abc', indexed_files: '5', total_files: '5' },
  },
];

describe('diagnostics formatters', () => {
  it('formatSnapshot renders version/platform/sqlite and table counts', () => {
    const text = formatSnapshot(sampleSnapshot);
    expect(text).toContain('version:         1.2.3');
    expect(text).toContain('platform:        windows x86_64');
    expect(text).toContain('sqliteVersion:   3.53.2');
    expect(text).toContain('appliedMigrations:  1, 2, 3');
    expect(text).toContain('draft: 2');
    expect(text).toContain('sources: 1');
  });

  it('formatEventLog renders each event with level, name and fields', () => {
    const text = formatEventLog(sampleEvents);
    expect(text).toContain('[2026-01-01T00:00:00Z] info app.startup');
    expect(text).toContain('sqlite=3.53.2');
    expect(text).toContain('source.backfill_completed');
    expect(text).toContain('indexed_files=5');
  });

  it('formatEventLog handles empty list', () => {
    expect(formatEventLog([])).toBe('(no events)');
  });

  it('formatDiagnosticBundle combines snapshot and events', () => {
    const text = formatDiagnosticBundle(sampleSnapshot, sampleEvents);
    expect(text).toContain('## Snapshot');
    expect(text).toContain('## Recent events');
    expect(text).toContain('version:         1.2.3');
    expect(text).toContain('source.backfill_completed');
  });

  it('parseEventLog parses the backend JSON string and ignores garbage', () => {
    expect(parseEventLog('')).toEqual([]);
    expect(parseEventLog('not json')).toEqual([]);
    const json = JSON.stringify(sampleEvents);
    expect(parseEventLog(json)).toEqual(sampleEvents);
  });
});
