import { useCallback, useState } from 'react';

import { markdownToWeChatHtml, markdownToPlainText } from './clipboard';
import { exportApi } from './api';
import { toCommandError } from '../native/command-client';

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
      const html = await markdownToWeChatHtml(markdown, { title, accentColor });
      await exportApi.writeClipboard({ html, text: markdownToPlainText(markdown) });
    } catch (err) {
      setError(toCommandError(err).message);
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
