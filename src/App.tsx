import { useState } from 'react';
import { useTranslation } from 'react-i18next';

import { SettingsView } from './settings/SettingsView';
import { SourcesView } from './sources/SourcesView';
import { WorkspaceView } from './workspace/WorkspaceView';

type View = 'workspace' | 'sources' | 'settings';

/**
 * 桌面工作区壳：侧栏导航 + 视图区。
 * 「工作台」视图为可调三栏（05 号票）；「来源管理」视图由 02 号票交付；
 * 「设置」视图由 08 号票交付（AI BYOK、总开关）。
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
        ) : view === 'settings' ? (
          <section className="pane pane--settings" data-el="pane-settings">
            <SettingsView />
          </section>
        ) : (
          <WorkspaceView />
        )}
      </div>
    </div>
  );
}
