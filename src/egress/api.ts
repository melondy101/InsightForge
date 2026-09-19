import { callCommand } from '../native/command-client';

/**
 * egress 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/egress.rs 的 serde 输出一一对应。
 */

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
  /** `ai` = 为 AI 出站；`sync` = 为多设备同步出站（ADR 0002 §5）。 */
  kind: 'ai' | 'sync';
  model: string;
  createdAt: string;
  fragmentCount: number;
  fragmentIds: string[];
  status: string;
}

export const egressApi = {
  build: (draftContent: string, draftId: string, searchQuery?: { sourceId: string; query: string; limit: number }) =>
    callCommand<EgressBundle>('egress_build', {
      draftContent,
      draftId,
      searchQuery: searchQuery ? [searchQuery.sourceId, searchQuery.query, searchQuery.limit] : null,
    }),
  auditLog: (model: string, fragmentCount: number, fragmentIds: string[], status: string) =>
    callCommand<AuditRecord>('audit_log', { model, fragmentCount, fragmentIds, status }),
  list: (limit: number) => callCommand<AuditRecord[]>('audit_list', { limit }),
};
