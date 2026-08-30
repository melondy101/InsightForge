import { useTranslation } from 'react-i18next';

/**
 * 桌面工作区占位壳：三栏布局（01 号票只交付骨架，无任何业务功能）。
 * 每个交互区域带 data-el 定位属性（CONTEXT.md §七）。
 */
export function App() {
  const { t } = useTranslation();

  return (
    <div className="workspace" data-el="workspace">
      <header className="workspace__titlebar" data-el="workspace-titlebar">
        {t('shell.appTitle')}
      </header>
      <div className="workspace__panes">
        <aside className="pane pane--sidebar" data-el="pane-sidebar">
          {t('shell.sidebar')}
        </aside>
        <section className="pane pane--editor" data-el="pane-editor">
          {t('shell.editor')}
        </section>
        <aside className="pane pane--evidence-shelf" data-el="pane-evidence-shelf">
          {t('shell.evidenceShelf')}
        </aside>
      </div>
    </div>
  );
}
