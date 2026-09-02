import { createContext, useContext } from 'react';
import type { SourceSummary } from './sources/api';
import type { DraftFile } from './drafts/api';
import type { DraftsModel } from './drafts/useDrafts';

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

/**
 * 共享草稿模型上下文：App 调用一次 useDrafts() 后经此下发，
 * DraftsView 与其他视图消费同一份状态，避免各自独立 hook 导致不同步。
 */
export const DraftsModelContext = createContext<DraftsModel | null>(null);

export function useAppState() {
  return useContext(AppStateContext);
}

export function useAppActions() {
  return useContext(AppActionsContext);
}

export function useDraftsModel(): DraftsModel {
  const model = useContext(DraftsModelContext);
  if (!model) {
    throw new Error('useDraftsModel must be used within a DraftsModelContext.Provider');
  }
  return model;
}
