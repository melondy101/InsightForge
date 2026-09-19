import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { App } from './App';
import { selectCommandClient, setCommandClient } from './native/command-client';
import './i18n';
import './styles.css';
import { applyTheme, getTheme } from './ui/theme';

applyTheme(getTheme());
// 命令通道在渲染前选定：桌面走 Tauri 桥，网页走同步服务的 HTTP 通道。
setCommandClient(selectCommandClient());

const rootElement = document.getElementById('root');
if (!rootElement) {
  throw new Error('missing #root element in index.html');
}

createRoot(rootElement).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
