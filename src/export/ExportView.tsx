import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useExport } from './useExport';

const ACCENT_COLORS = [
  { name: 'Blue', value: '#0550ae' },
  { name: 'Green', value: '#1a7f37' },
  { name: 'Purple', value: '#8250df' },
  { name: 'Orange', value: '#d18616' },
];

export function ExportView({ content, title }: { content: string; title: string }) {
  const { t } = useTranslation();
  const { exporting, error, actions } = useExport();
  const [accentColor, setAccentColor] = useState(ACCENT_COLORS[0].value);

  const handleExport = async () => {
    await actions.export(content, { title: title || 'Export', accentColor });
  };

  return (
    <section className="export-view" data-el="export-view">
      <h3>{t('export.title')}</h3>

      {error && (
        <div className="export-view__error" data-el="export-error">
          <span>{error}</span>
          <button type="button" onClick={actions.dismissError}>
            {t('export.dismiss')}
          </button>
        </div>
      )}

      <div className="export-view__options" data-el="export-options">
        <label className="export-view__option">
          <span>{t('export.accentColor')}:</span>
          <select value={accentColor} onChange={(e) => setAccentColor(e.target.value)}>
            {ACCENT_COLORS.map((color) => (
              <option key={color.value} value={color.value}>
                {color.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      <div className="export-view__actions">
        <button
          type="button"
          onClick={handleExport}
          disabled={exporting || !content}
          data-el="export-button"
        >
          {exporting ? t('export.exporting') : t('export.exportButton')}
        </button>
      </div>

      <p className="export-view__hint" data-el="export-hint">
        {t('export.hint')}
      </p>
    </section>
  );
}
