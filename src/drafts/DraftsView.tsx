import { useTranslation } from 'react-i18next';
import { useDraftsModel } from '../AppStateContext';
import { errorText } from '../ui/error-text';

function DraftsListView({ drafts, onOpen }: { drafts: { filename: string; updatedAt: string }[]; onOpen: (f: string) => void }) {
  const { t } = useTranslation();
  return (
    <div className="drafts-list" data-el="drafts-list">
      <h3>{t('drafts.listTitle')}</h3>
      <ul>
        {drafts.map((d) => (
          <li key={d.filename}>
            <button type="button" onClick={() => onOpen(d.filename)}>
              {d.filename}
            </button>
            <span className="drafts-list__time">{d.updatedAt}</span>
          </li>
        ))}
        {drafts.length === 0 && <li className="drafts-list__empty">{t('drafts.empty')}</li>}
      </ul>
    </div>
  );
}

function ConflictDialog({
  conflict,
  onResolve,
}: {
  conflict: { message: string; local: string; external: string };
  onResolve: (choice: 'local' | 'external' | 'saveAs') => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="drafts-conflict" data-el="drafts-conflict">
      <div className="drafts-conflict__dialog" data-el="drafts-conflict-dialog">
        <h3>{t('drafts.conflictTitle')}</h3>
        <p>{conflict.message}</p>
        <div className="drafts-conflict__actions">
          <button
            type="button"
            data-el="drafts-conflict-local"
            onClick={() => onResolve('local')}
          >
            {t('drafts.conflictKeepLocal')}
          </button>
          <button
            type="button"
            data-el="drafts-conflict-external"
            onClick={() => onResolve('external')}
          >
            {t('drafts.conflictAdoptExternal')}
          </button>
          <button
            type="button"
            data-el="drafts-conflict-saveas"
            onClick={() => onResolve('saveAs')}
          >
            {t('drafts.conflictSaveAs')}
          </button>
        </div>
      </div>
    </div>
  );
}

export function DraftsView() {
  const { t } = useTranslation();
  const {
    drafts,
    activeDraft,
    content,
    status,
    conflict,
    error,
    actions,
  } = useDraftsModel();

  const handleCreate = async () => {
    const filename = prompt(t('drafts.promptFilename') as string) ?? '';
    if (filename.trim().length > 0) {
      await actions.create(filename.trim());
    }
  };

  const handleSave = async () => {
    await actions.save();
  };

  const handleDelete = async () => {
    if (activeDraft && confirm(t('drafts.confirmDelete') as string)) {
      await actions.delete();
    }
  };

  const listDrafts = drafts.map((d) => ({
    filename: d.filename,
    updatedAt: d.updatedAt,
  }));

  return (
    <section className="drafts-view" data-el="drafts-view">
      <header className="drafts-view__header">
        <h2>{t('drafts.title')}</h2>
        <div className="drafts-view__toolbar">
          <button type="button" data-el="drafts-new" onClick={handleCreate} disabled={status !== 'idle'}>
            {t('drafts.new')}
          </button>
          {activeDraft && (
            <>
              <button type="button" data-el="drafts-save" onClick={handleSave} disabled={status !== 'idle'}>
                {t('drafts.save')}
              </button>
              <button
                type="button"
                data-el="drafts-delete"
                onClick={handleDelete}
                disabled={status !== 'idle'}
              >
                {t('drafts.delete')}
              </button>
            </>
          )}
        </div>
      </header>

      {error && (
        <div className="drafts-view__error" data-el="drafts-error">
          <span>{errorText(t, error, 'drafts')}</span>
          <button type="button" onClick={actions.dismissError}>
            {t('drafts.dismiss')}
          </button>
        </div>
      )}

      {conflict && (
        <ConflictDialog
          conflict={{ message: conflict.message, local: conflict.local, external: conflict.external }}
          onResolve={actions.resolveConflict}
        />
      )}

      <div className="drafts-view__body">
        <DraftsListView
          drafts={listDrafts}
          onOpen={(filename) => void actions.open(filename)}
        />
        <div className="drafts-view__editor" data-el="drafts-editor">
          {activeDraft ? (
            <>
              <h3>{activeDraft.filename}</h3>
              <textarea
                className="drafts-editor__textarea"
                data-el="drafts-editor-textarea"
                value={content}
                onChange={(e) => actions.setContent(e.target.value)}
              />
            </>
          ) : (
            <p className="drafts-view__placeholder">{t('drafts.openPlaceholder')}</p>
          )}
        </div>
      </div>
    </section>
  );
}
