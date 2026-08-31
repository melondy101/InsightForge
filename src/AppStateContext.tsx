import { createContext, useContext } from 'react';
import type { SourceSummary } from './sources/api';
import type { DraftFile } from './drafts/api';

export interface AppState {
  sources: SourceSummary[];
  activeDraft: DraftFile | null;
}

export interface AppStateActions {
  refreshSources: () => Promise<void>;
  refreshDrafts: () => Promise<void>;
  openDraft: (filename: string) => Promise<void>;
}

export const AppStateContext = createContext<AppState>({
  sources: [],
  activeDraft: null,
});

export const AppActionsContext = createContext<AppStateActions>({
  refreshSources: async () => {},
  refreshDrafts: async () => {},
  openDraft: async () => {},
});

export function useAppState() {
  return useContext(AppStateContext);
}

export function useAppActions() {
  return useContext(AppActionsContext);
}
