import { callCommand } from '../native/command-client';

/**
 * references 命令 seam 的前端薄投影（ARCHITECTURE §4）。
 * 类型与 crates/core/src/references.rs 的 serde 输出一一对应。
 */

export interface Reference {
  id: string;
  draftId: string;
  fragmentId: number;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  snippet: string;
  fullText: string;
  startLine: number;
  endLine: number;
  adopted: boolean;
  sourceDeleted: boolean;
  createdAt: string;
}

export const referencesApi = {
  create: (draftId: string, fragmentId: number) =>
    callCommand<Reference>('references_create', { draftId, fragmentId }),
  list: (draftId: string) => callCommand<Reference[]>('references_list', { draftId }),
  adopt: (id: string) => callCommand<void>('references_adopt', { id }),
  markSourceDeleted: (sourceId: string) =>
    callCommand<void>('references_mark_source_deleted', { sourceId }),
};
