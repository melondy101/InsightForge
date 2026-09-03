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
import {
  IconWorkspace, IconDraft, IconReference, IconThought,
  IconSource, IconSearch, IconSettings, IconEgress,
  IconSun, IconMoon,
} from './ui/icons';

type View =
  | 'workspace'
  | 'sources'
  | 'search'
  | 'settings'
  | 'drafts'
  | 'references'
  | 'thoughts'
  | 'diagnostics';

function BrandMark() {
  return (
    <svg className="brand__mark" viewBox="0 0 26 26" fill="none" aria-hidden="true">
      <rect x="2" y="2" width="22" height="22" rx="6" stroke="currentColor" strokeWidth="1.6" />
      <path d="M8 18l4-8 4 8" stroke="var(--ember)" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx="18" cy="8" r="2" fill="var(--ember)" />
    </svg>
  );
}

function ThemeToggle() {
  const [isDark, setIsDark] = useState(true);
  return (
    <button
      type="button"
      className="iconbtn"
      data-el="theme-toggle"
      aria-label={isDark ? 'Switch to light' : 'Switch to dark'}
      onClick={() => setIsDark(v => !v)}
    >
      {isDark ? <IconSun /> : <IconMoon />}
    </button>
  );
}

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

  const navItemClass = (v: View) => 'navitem' + (view === v ? ' is-active' : '');

  return (
    <AppStateContext.Provider value={appState}>
      <AppActionsContext.Provider value={appActions}>
        <DraftsModelContext.Provider value={draftsModel}>
          <div className="workspace" data-el="workspace">
            <header className="titlebar" data-el="workspace-titlebar">
              <div className="brand">
                <BrandMark />
                <div>
                  <div className="brand__name">{t('shell.appTitle')}</div>
                  <div className="brand__sub">Local · Anti-Ghostwriting Bench</div>
                </div>
              </div>
              <div className="titlebar__meta">
                <span className="docpath">
                  <span className="dot" />
                  {draftsModel.activeDraft?.filename ?? '—'}
                </span>
                <span className="saved">Saved</span>
                <ThemeToggle />
              </div>
            </header>
            <div className="workspace__panes">
              <aside className="pane pane--sidebar" data-el="pane-sidebar" aria-label={t('shell.sidebar')}>
                <nav className="rail" data-el="sidebar-nav">
                  <div className="navgroup">
                    <div className="eyebrow">{t('shell.groupWriting')}</div>
                    <button type="button" className={navItemClass('workspace')} data-el="nav-workspace" onClick={() => setView('workspace')}>
                      <IconWorkspace /> {t('sources.nav.workspace')}
                    </button>
                    <button type="button" className={navItemClass('drafts')} data-el="nav-drafts" onClick={() => setView('drafts')}>
                      <IconDraft /> {t('drafts.nav.drafts')}
                    </button>
                    <button type="button" className={navItemClass('references')} data-el="nav-references" onClick={() => setView('references')}>
                      <IconReference /> {t('references.nav.references')}
                    </button>
                    <button type="button" className={navItemClass('thoughts')} data-el="nav-thoughts" onClick={() => setView('thoughts')}>
                      <IconThought /> {t('thoughts.nav.thoughts')}
                    </button>
                  </div>
                  <div className="navgroup">
                    <div className="eyebrow">{t('shell.groupSources')}</div>
                    <button type="button" className={navItemClass('sources')} data-el="nav-sources" onClick={() => setView('sources')}>
                      <IconSource /> {t('sources.nav.sources')}
                    </button>
                    <button type="button" className={navItemClass('search')} data-el="nav-search" onClick={() => setView('search')}>
                      <IconSearch /> {t('search.nav.search')}
                    </button>
                  </div>
                  <div className="navgroup">
                    <div className="eyebrow">{t('shell.groupSystem')}</div>
                    <button type="button" className={navItemClass('settings')} data-el="nav-settings" onClick={() => setView('settings')}>
                      <IconSettings /> {t('settings.nav.settings')}
                    </button>
                    <button type="button" className={navItemClass('diagnostics')} data-el="nav-diagnostics" onClick={() => setView('diagnostics')}>
                      <IconEgress /> {t('diagnostics.nav.diagnostics')}
                    </button>
                  </div>
                  <div className="rail__foot">
                    <ThemeToggle />
                    <div className="ai-pill">
                      <span className="led" />
                      <span>{t('shell.aiReady')}</span>
                    </div>
                    <div className="rail__version">{t('shell.version', { version: '0.1.0' })}</div>
                  </div>
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
