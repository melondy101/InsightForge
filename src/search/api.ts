import { callCommand } from '../native/command-client';

/** search 命令 seam 的前端薄投影（ARCHITECTURE §4）。 */

export interface EvidenceCard {
  fragmentId: number;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  snippet: string;
  bm25: number;
  startLine: number;
  endLine: number;
}

export interface FragmentContext {
  fragmentId: number;
  sourceId: string;
  sourceName: string;
  relPath: string;
  headingPath: string;
  text: string;
  startLine: number;
  endLine: number;
}

export const searchApi = {
  query: (sourceId: string, query: string, limit?: number) =>
    callCommand<EvidenceCard[]>('search_query', { sourceId, query, limit }),
  openContext: (sourceId: string, fragmentId: number) =>
    callCommand<FragmentContext | null>('search_open_context', { sourceId, fragmentId }),
};
