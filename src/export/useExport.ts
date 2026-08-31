import { useCallback, useState } from 'react';

import { markdownToCleanHtml, type ExportOptions, generateExportFilename } from './export';

export interface ExportModel {
  exporting: boolean;
  error: string | null;
  actions: {
    export: (markdown: string, options: ExportOptions) => Promise<string | null>;
    dismissError: () => void;
  };
}

export function useExport(): ExportModel {
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const exportMarkdown = useCallback(async (markdown: string, options: ExportOptions) => {
    setExporting(true);
    setError(null);
    try {
      const html = await markdownToCleanHtml(markdown, options);
      const filename = generateExportFilename(options.title);

      // Trigger download
      const blob = new Blob([html], { type: 'text/html' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = filename;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      return filename;
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Export failed';
      setError(message);
      return null;
    } finally {
      setExporting(false);
    }
  }, []);

  const actions = {
    export: exportMarkdown,
    dismissError: () => setError(null),
  };

  return { exporting, error, actions };
}
