import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { SourcesView } from './sources/SourcesView';

type View = 'workspace' | 'sources';

/**
 * 桌面工作区壳：三栏布局 + 侧栏导航。
 * 「来源管理」视图由 02 号票交付；工作台三栏仍是占位骨架。
 * 每个交互区域带 data-el 定位属性（CONTEXT.md §七）。
 */
export function App() {
  const { t } = useTranslation();
  const [view, setView] = useState<View>('workspace');

  return (
    <div className="workspace" data-el="workspace">
      <header className="workspace__titlebar" data-el="workspace-titlebar">
        {t('shell.appTitle')}
      </header>
      <div className="workspace__panes">
        <aside className="pane pane--sidebar" data-el="pane-sidebar">
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
          </nav>
        </aside>
        {view === 'sources' ? (
          <section className="pane pane--sources" data-el="pane-sources">
            <SourcesView />
          </section>
        ) : (
          <>
            <section className="pane pane--editor" data-el="pane-editor">
              {t('shell.editor')}
            </section>
            <aside className="pane pane--evidence-shelf" data-el="pane-evidence-shelf">
              {t('shell.evidenceShelf')}
            </aside>
          </>
        )}
      </div>
    </div>
  );
}
