import { callCommand } from '../native/command-client';

/**
 * thoughts 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/thoughts.rs 的 serde 输出一一对应。
 */

export interface ThoughtCard {
  id: string;
  idea: string;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  draftId: string;
  draftFilename: string;
  createdAt: string;
  updatedAt: string;
}

export interface UpsertThoughtCardInput {
  id?: string;
  idea: string;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  draftId: string;
  draftFilename: string;
}

export const thoughtsApi = {
  upsert: (input: UpsertThoughtCardInput) =>
    callCommand<ThoughtCard>('thoughts_upsert', input as unknown as Record<string, unknown>),
  list: () => callCommand<ThoughtCard[]>('thoughts_list'),
  listByDraft: (draftId: string) =>
    callCommand<ThoughtCard[]>('thoughts_list_by_draft', { draftId }),
  listBySource: (sourceId: string) =>
    callCommand<ThoughtCard[]>('thoughts_list_by_source', { sourceId }),
  delete: (id: string) => callCommand<void>('thoughts_delete', { id }),
};
