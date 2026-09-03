import { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { DraftsView } from './drafts/DraftsView';
import { DiagnosticsView } from './diagnostics/DiagnosticsView';
import { ReferencesView } from './references/ReferencesView';
import { SearchView } from './search/SearchView';
import { SettingsView } from './settings/SettingsView';
import { SourcesView } from './sources/SourcesView';
import { ThoughtsView } from './thoughts/ThoughtsView';
import { useSources } from './sources/useSources';
import { useDrafts } from './drafts/useDrafts';
import { AppStateContext, AppActionsContext, DraftsModelContext } from './AppStateContext';
import { WorkspaceView } from './workspace/WorkspaceView';

type View =
  | 'workspace'
  | 'sources'
  | 'search'
  | 'settings'
  | 'drafts'
  | 'references'
  | 'thoughts'
  | 'diagnostics';

/**
 * 桌面工作区壳：侧栏导航 + 视图区。
 * 单一共享状态源：App 调用一次 useDrafts() / useSources()，经 context 下发；
 * DraftsView 等子视图从 context 消费同一份状态，不再各自创建独立 hook。
 */
export function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<View>('workspace');
  const { sources: appSources, actions: sourceActions } = useSources();
  const draftsModel = useDrafts();

  const defaultSource = useMemo(() => appSources[0] ?? null, [appSources]);

  const appState = useMemo(
    () => ({ sources: appSources, activeDraft: draftsModel.activeDraft }),
    [appSources, draftsModel.activeDraft],
  );

  const appActions = useMemo(
    () => ({
      refreshSources: async () => {
        await sourceActions.refresh();
      },
      refreshDrafts: async () => {
        await draftsModel.actions.refresh();
      },
      openDraft: async (filename: string) => {
        await draftsModel.actions.open(filename);
      },
    }),
    [sourceActions, draftsModel.actions],
  );

  return (
    <AppStateContext.Provider value={appState}>
      <AppActionsContext.Provider value={appActions}>
        <DraftsModelContext.Provider value={draftsModel}>
          <div className="workspace" data-el="workspace">
            <header className="workspace__titlebar" data-el="workspace-titlebar">
              {t('shell.appTitle')}
            </header>
            <div className="workspace__panes">
              <aside className="pane pane--sidebar" data-el="pane-sidebar" aria-label={t('shell.sidebar')}>
                <nav className="sidebar__nav" data-el="sidebar-nav">
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-workspace"
                    onClick={() => setView('workspace')}
                  >
                    {t('sources.nav.workspace')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-sources"
                    onClick={() => setView('sources')}
                  >
                    {t('sources.nav.sources')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-drafts"
                    onClick={() => setView('drafts')}
                  >
                    {t('drafts.nav.drafts')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-references"
                    onClick={() => setView('references')}
                  >
                    {t('references.nav.references')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-thoughts"
                    onClick={() => setView('thoughts')}
                  >
                    {t('thoughts.nav.thoughts')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-search"
                    onClick={() => setView('search')}
                  >
                    {t('search.nav.search')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-settings"
                    onClick={() => setView('settings')}
                  >
                    {t('settings.nav.settings')}
                  </button>
                  <button
                    type="button"
                    className="sidebar__nav-item"
                    data-el="nav-diagnostics"
                    onClick={() => setView('diagnostics')}
                  >
                    {t('diagnostics.nav.diagnostics')}
                  </button>
                </nav>
              </aside>
              {view === 'sources' ? (
                <section className="pane pane--sources" data-el="pane-sources">
                  <SourcesView />
                </section>
              ) : view === 'search' ? (
                <section className="pane pane--search" data-el="pane-search">
                  {defaultSource ? (
                    <SearchView sourceId={defaultSource.id} sourceName={defaultSource.name} />
                  ) : (
                    <div className="search" data-el="search-view">
                      <h2 data-el="search-title">{t('search.title')}</h2>
                      <p data-el="search-empty">{t('search.emptySource')}</p>
                    </div>
                  )}
                </section>
              ) : view === 'settings' ? (
                <section className="pane pane--settings" data-el="pane-settings">
                  <SettingsView />
                </section>
              ) : view === 'drafts' ? (
                <section className="pane pane--drafts" data-el="pane-drafts">
                  <DraftsView />
                </section>
              ) : view === 'references' ? (
                <section className="pane pane--references" data-el="pane-references">
                  {draftsModel.activeDraft ? (
                    <ReferencesView draftId={draftsModel.activeDraft.id} />
                  ) : (
                    <div className="references-view" data-el="references-view">
                      <h3>{t('references.title')}</h3>
                      <p data-el="references-empty">{t('references.emptyDraft')}</p>
                    </div>
                  )}
                </section>
              ) : view === 'thoughts' ? (
                <section className="pane pane--thoughts" data-el="pane-thoughts">
                  <ThoughtsView />
                </section>
              ) : view === 'diagnostics' ? (
                <section className="pane pane--diagnostics" data-el="pane-diagnostics">
                  <DiagnosticsView />
                </section>
              ) : (
                <WorkspaceView />
              )}
            </div>
          </div>
        </DraftsModelContext.Provider>
      </AppActionsContext.Provider>
    </AppStateContext.Provider>
  );
}
