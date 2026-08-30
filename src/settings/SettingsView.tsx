import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import {
  settingsApi,
  type AiSampleReply,
  type AiSettings,
  type CommandErrorShape,
  type TestConnectionReport,
} from './api';
import { aiStatusView, apiKeyView, settingsErrorView, testResultView } from './view';

/** 样例 AI 命令的固定最小提示词（演示守门语义用）。 */
const SAMPLE_PROMPT = 'ping';

type TestDisplay =
  | { kind: 'report'; report: TestConnectionReport }
  | { kind: 'error'; error: CommandErrorShape }
  | null;

/**
 * 设置视图（CONTEXT.md §六）：AI（BYOK、模型、总开关）。
 * API Key 输入框永不明文回显——保存后只显示「已配置」徽标与清除按钮。
 */
export function SettingsView() {
  const { t } = useTranslation();
  const [ai, setAi] = useState<AiSettings | null>(null);
  const [baseUrl, setBaseUrl] = useState('');
  const [model, setModel] = useState('');
  const [apiKeyDraft, setApiKeyDraft] = useState('');
  const [busy, setBusy] = useState(false);
  const [testing, setTesting] = useState(false);
  const [sampling, setSampling] = useState(false);
  const [savedFlash, setSavedFlash] = useState(false);
  const [error, setError] = useState<CommandErrorShape | null>(null);
  const [testDisplay, setTestDisplay] = useState<TestDisplay>(null);
  const [sampleReply, setSampleReply] = useState<AiSampleReply | null>(null);
  const [sampleError, setSampleError] = useState<CommandErrorShape | null>(null);

  const refresh = useCallback(async () => {
    try {
      setAi(await settingsApi.get());
    } catch (err) {
      setError(err as CommandErrorShape);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const keyView = apiKeyView(ai?.apiKeyConfigured ?? false, apiKeyDraft);
  const status = ai
    ? aiStatusView(ai)
    : null;

  const run = async (action: () => Promise<AiSettings>) => {
    setBusy(true);
    setError(null);
    try {
      setAi(await action());
      setSavedFlash(true);
      window.setTimeout(() => setSavedFlash(false), 2000);
    } catch (err) {
      setError(err as CommandErrorShape);
    } finally {
      setBusy(false);
    }
  };

  const handleSave = () =>
    void run(async () => {
      let next = await settingsApi.setAi(
        baseUrl.trim().length > 0 ? baseUrl.trim() : null,
        model.trim().length > 0 ? model.trim() : null,
      );
      if (apiKeyDraft.trim().length > 0) {
        next = await settingsApi.setApiKey(apiKeyDraft.trim());
        setApiKeyDraft('');
      }
      setBaseUrl(next.baseUrl ?? '');
      setModel(next.model ?? '');
      return next;
    });

  const handleClearKey = () => {
    setApiKeyDraft('');
    void run(async () => {
      const next = await settingsApi.clearApiKey();
      setBaseUrl(next.baseUrl ?? '');
      setModel(next.model ?? '');
      return next;
    });
  };

  const handleToggle = () => {
    if (!ai) {
      return;
    }
    void run(() => settingsApi.setAiEnabled(!ai.enabled));
  };

  const handleTest = async () => {
    setTesting(true);
    setTestDisplay(null);
    try {
      setTestDisplay({ kind: 'report', report: await settingsApi.testConnection() });
    } catch (err) {
      setTestDisplay({ kind: 'error', error: err as CommandErrorShape });
    } finally {
      setTesting(false);
    }
  };

  const handleSample = async () => {
    setSampling(true);
    setSampleReply(null);
    setSampleError(null);
    try {
      setSampleReply(await settingsApi.aiSample(SAMPLE_PROMPT));
    } catch (err) {
      setSampleError(err as CommandErrorShape);
    } finally {
      setSampling(false);
    }
  };

  return (
    <div className="settings" data-el="settings-view">
      <h2 data-el="settings-title">{t('settings.title')}</h2>
      {error ? (
        <p className="settings__error" data-el="settings-error" role="alert">
          {t(settingsErrorView(error).key, settingsErrorView(error).params)}
          <button type="button" data-el="settings-error-dismiss" onClick={() => setError(null)}>
            {t('settings.dismissError')}
          </button>
        </p>
      ) : null}

      <section className="settings__section" data-el="settings-ai-section">
        <h3>{t('settings.aiSectionTitle')}</h3>

        <div className="settings__status-row" data-el="settings-ai-status-row">
          <span data-el="settings-ai-status">{status ? t(status.key) : ''}</span>
          <label className="settings__toggle" data-el="settings-ai-toggle-label">
            <input
              type="checkbox"
              data-el="settings-ai-toggle"
              checked={ai?.enabled ?? false}
              disabled={busy || !ai}
              onChange={handleToggle}
            />
            <span>{t('settings.enabledLabel')}</span>
          </label>
        </div>
        <p className="settings__hint" data-el="settings-ai-enabled-hint">
          {t('settings.enabledHint')}
        </p>

        <label className="settings__field">
          <span>{t('settings.baseUrlLabel')}</span>
          <input
            data-el="settings-ai-base"
            value={baseUrl}
            placeholder={t('settings.baseUrlPlaceholder')}
            onChange={(event) => setBaseUrl(event.target.value)}
          />
        </label>
        <label className="settings__field">
          <span>{t('settings.modelLabel')}</span>
          <input
            data-el="settings-ai-model"
            value={model}
            placeholder={t('settings.modelPlaceholder')}
            onChange={(event) => setModel(event.target.value)}
          />
        </label>
        <label className="settings__field">
          <span>{t('settings.apiKeyLabel')}</span>
          <input
            type="password"
            data-el="settings-ai-key"
            value={apiKeyDraft}
            placeholder={t(keyView.placeholderKey)}
            autoComplete="off"
            onChange={(event) => setApiKeyDraft(event.target.value)}
          />
        </label>
        <div className="settings__key-state" data-el="settings-key-state">
          {keyView.badgeVisible ? (
            <span className="settings__badge" data-el="settings-key-configured">
              {t('settings.apiKeyConfigured')}
            </span>
          ) : (
            <span className="settings__badge settings__badge--muted" data-el="settings-key-missing">
              {t('settings.apiKeyNotConfigured')}
            </span>
          )}
          {keyView.clearVisible ? (
            <button
              type="button"
              data-el="settings-key-clear"
              disabled={busy}
              onClick={handleClearKey}
            >
              {t('settings.clearKeyButton')}
            </button>
          ) : null}
        </div>

        <div className="settings__actions">
          <button type="button" data-el="settings-ai-save" disabled={busy} onClick={handleSave}>
            {t('settings.saveButton')}
          </button>
          <button
            type="button"
            data-el="settings-test-btn"
            disabled={testing || busy}
            onClick={() => void handleTest()}
          >
            {testing ? t('settings.testing') : t('settings.testButton')}
          </button>
          <button
            type="button"
            data-el="settings-sample-btn"
            disabled={sampling || busy}
            onClick={() => void handleSample()}
          >
            {t('settings.sampleButton')}
          </button>
          {savedFlash ? (
            <span className="settings__flash" data-el="settings-saved-flash">
              {t('settings.saved')}
            </span>
          ) : null}
        </div>

        <div data-el="settings-test-result" role="status">
          {testDisplay ? (
            testDisplay.kind === 'report' ? (
              <p
                className={
                  testResultView(testDisplay.report).tone === 'ok'
                    ? 'settings__result settings__result--ok'
                    : 'settings__result settings__result--fail'
                }
              >
                {t(
                  testResultView(testDisplay.report).key,
                  testResultView(testDisplay.report).params,
                )}
                {testDisplay.report.detail ? (
                  <span className="settings__detail"> {t('settings.testResult.detail', { detail: testDisplay.report.detail })}</span>
                ) : null}
              </p>
            ) : (
              <p className="settings__result settings__result--fail">
                {t(settingsErrorView(testDisplay.error).key, settingsErrorView(testDisplay.error).params)}
              </p>
            )
          ) : null}
        </div>

        <div data-el="settings-sample-result" role="status">
          {sampleReply ? (
            <p className="settings__result settings__result--ok">
              {t('settings.sampleReply', { content: sampleReply.content })}
            </p>
          ) : sampleError ? (
            <p className="settings__result settings__result--fail">
              {t(settingsErrorView(sampleError).key, settingsErrorView(sampleError).params)}
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}
