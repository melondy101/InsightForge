import { useCallback, useState } from 'react';

import { markdownToWeChatHtml, markdownToPlainText } from './clipboard';

export interface ClipboardExportModel {
  exporting: boolean;
  error: string | null;
  actions: {
    exportToClipboard: (markdown: string, title: string, accentColor: string) => Promise<void>;
    dismissError: () => void;
  };
}

export function useClipboardExport(): ClipboardExportModel {
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const exportToClipboard = useCallback(async (
    markdown: string,
    title: string,
    accentColor: string,
  ) => {
    setExporting(true);
    setError(null);

    try {
      // Generate WeChat-compatible HTML
      const html = await markdownToWeChatHtml(markdown, { title, accentColor });

      // Generate plain text fallback
      const plainText = markdownToPlainText(markdown);

      // Write to clipboard via Tauri command
      if (window.__TAURI__) {
        await window.__TAURI__.invoke('clipboard_write_html', { html });
      }

      // Also write plain text to clipboard (as fallback)
      if (navigator.clipboard && navigator.clipboard.writeText) {
        await navigator.clipboard.writeText(plainText);
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Clipboard export failed';
      setError(message);
    } finally {
      setExporting(false);
    }
  }, []);

  const actions = {
    exportToClipboard,
    dismissError: () => setError(null),
  };

  return { exporting, error, actions };
}
