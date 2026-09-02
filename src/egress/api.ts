import { invoke } from '@tauri-apps/api/core';

/**
 * egress 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/egress.rs 的 serde 输出一一对应。
 */

export type EgressErrorShape = {
  code: string;
  message: string;
};

export interface EgressFragment {
  kind: string;
  label: string;
  text: string;
}

export interface EgressBundle {
  fragmentCount: number;
  fragments: EgressFragment[];
}

export interface AuditRecord {
  id: string;
  model: string;
  createdAt: string;
  fragmentCount: number;
  fragmentIds: string[];
  status: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function toEgressError(err: unknown): EgressErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toEgressError(err);
  }
}

export const egressApi = {
  build: (draftContent: string, draftId: string, searchQuery?: { sourceId: string; query: string; limit: number }) =>
    call<EgressBundle>('egress_build', {
      draftContent,
      draftId,
      searchQuery: searchQuery ? [searchQuery.sourceId, searchQuery.query, searchQuery.limit] : null,
    }),
  auditLog: (model: string, fragmentCount: number, fragmentIds: string[], status: string) =>
    call<AuditRecord>('audit_log', { model, fragmentCount, fragmentIds, status }),
  list: (limit: number) => call<AuditRecord[]>('audit_list', { limit }),
};
