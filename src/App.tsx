import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';

import { DraftsView } from './drafts/DraftsView';
import { ReferencesView } from './references/ReferencesView';
import { SearchView } from './search/SearchView';
import { SettingsView } from './settings/SettingsView';
import { SourcesView } from './sources/SourcesView';
import { ThoughtsView } from './thoughts/ThoughtsView';
import { useSources } from './sources/useSources';
import { useDrafts } from './drafts/useDrafts';
import { WorkspaceView } from './workspace/WorkspaceView';

type View = 'workspace' | 'sources' | 'search' | 'settings' | 'drafts' | 'references' | 'thoughts';

/**
 * 桌面工作区壳：侧栏导航 + 视图区。
 * 「工作台」视图为可调三栏（05 号票）；「来源管理」视图由 02 号票交付；
 * 「检索」视图由 03 号票交付；「设置」视图由 08 号票交付（AI BYOK、总开关）。
 * 「草稿」视图由 06 号票交付（原子保存 + 冲突检测）。
 * 「引用」视图由 07 号票交付（证据架与不可变引用快照）。
 * 「思考卡」视图由 11 号票交付（作者资产，不随索引重建而重建）。
 * 每个交互区域带 data-el 定位属性（CONTEXT.md §七）。
 */
export function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<View>('workspace');
  const { sources } = useSources();
  const { activeDraft } = useDrafts();

  const defaultSource = useMemo(() => sources[0] ?? null, [sources]);

  return (
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
            {activeDraft ? (
              <ReferencesView draftId={activeDraft.id} />
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
        ) : (
          <WorkspaceView />
        )}
      </div>
    </div>
  );
}
