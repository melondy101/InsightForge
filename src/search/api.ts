import { invoke } from '@tauri-apps/api/core';

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

export interface SearchErrorShape {
  code: string;
  message: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

export function toSearchError(err: unknown): SearchErrorShape {
  if (isRecord(err) && typeof err.code === 'string' && typeof err.message === 'string') {
    return { code: err.code, message: err.message };
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toSearchError(err);
  }
}

export const searchApi = {
  query: (sourceId: string, query: string, limit?: number) =>
    call<EvidenceCard[]>('search_query', { sourceId, query, limit }),
  openContext: (sourceId: string, fragmentId: number) =>
    call<FragmentContext | null>('search_open_context', { sourceId, fragmentId }),
};
