import { callCommand } from '../native/command-client';

/**
 * diagnostics 命令 seam 的前端薄投影（Phase C）。
 * 类型与 crates/core/src/diagnostics.rs 的 serde 输出一一对应（camelCase）。
 */

export interface DatabaseDiagnostics {
  journalMode: string;
  appliedMigrations: number[];
  integrity: string;
  tableCounts: Record<string, number>;
}

export interface DiagnosticSnapshot {
  version: string;
  generatedAt: string;
  platform: string;
  sqliteVersion: string;
  database: DatabaseDiagnostics;
}

export interface LogEntry {
  ts: string;
  level: string;
  name: string;
  fields: Record<string, string>;
}

export interface DiagnosticBundle {
  snapshot: DiagnosticSnapshot;
  events: LogEntry[];
}

export const diagnosticsApi = {
  getSnapshot: (version: string) =>
    callCommand<DiagnosticSnapshot>('diagnostics_snapshot', { version }),
  getEventLog: (limit?: number) =>
    callCommand<string>('diagnostics_event_log', { limit: limit ?? null }),
};

/// 把结构化快照格式化为可读文本（用于复制/导出）。
export function formatSnapshot(s: DiagnosticSnapshot): string {
  const lines: string[] = [];
  lines.push(`version:         ${s.version}`);
  lines.push(`generatedAt:     ${s.generatedAt}`);
  lines.push(`platform:        ${s.platform}`);
  lines.push(`sqliteVersion:   ${s.sqliteVersion}`);
  lines.push('database:');
  lines.push(`  journalMode:        ${s.database.journalMode}`);
  lines.push(`  integrity:          ${s.database.integrity}`);
  lines.push(
    `  appliedMigrations:  ${s.database.appliedMigrations.join(', ') || '(none)'}`,
  );
  lines.push('  tableCounts:');
  const tables = Object.entries(s.database.tableCounts).sort((a, b) =>
    a[0].localeCompare(b[0]),
  );
  if (tables.length === 0) {
    lines.push('    (none)');
  } else {
    for (const [t, c] of tables) {
      lines.push(`    ${t}: ${c}`);
    }
  }
  return lines.join('\n');
}

/// 把事件日志格式化为可读文本。
export function formatEventLog(entries: LogEntry[]): string {
  if (entries.length === 0) {
    return '(no events)';
  }
  return entries
    .map((e) => {
      const fields = Object.entries(e.fields)
        .map(([k, v]) => `${k}=${v}`)
        .join(' ');
      const head = `[${e.ts}] ${e.level} ${e.name}`;
      return fields ? `${head} ${fields}` : head;
    })
    .join('\n');
}

/// 组装一份可复制/导出的完整诊断文本（快照 + 事件）。
export function formatDiagnosticBundle(snapshot: DiagnosticSnapshot, events: LogEntry[]): string {
  return `# InsightForge Diagnostics

## Snapshot
${formatSnapshot(snapshot)}

## Recent events
${formatEventLog(events)}
`;
}

/// 解析后端返回的事件日志 JSON 字符串（命令返回的是序列化后的数组）。
export function parseEventLog(json: string): LogEntry[] {
  if (!json) return [];
  try {
    const value = JSON.parse(json);
    if (Array.isArray(value)) {
      return value as LogEntry[];
    }
    return [];
  } catch {
    return [];
  }
}

