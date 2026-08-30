import { describe, expect, it } from 'vitest';

import {
  aiStatusView,
  apiKeyView,
  draftFromSnapshot,
  settingsErrorView,
  testResultView,
} from './view';

describe('apiKeyView —— 已配置状态遮蔽（永不明文回显）', () => {
  it('未配置且无草稿：不显示已配置徽标、无清除按钮、不能保存 Key', () => {
    const view = apiKeyView(false, '');
    expect(view.badgeVisible).toBe(false);
    expect(view.clearVisible).toBe(false);
    expect(view.canSaveKey).toBe(false);
    expect(view.placeholderKey).toBe('settings.apiKeyPlaceholder');
  });

  it('未配置但正在输入：可以保存 Key', () => {
    expect(apiKeyView(false, 'sk-abc').canSaveKey).toBe(true);
  });

  it('已配置且无草稿：显示已配置徽标与清除按钮，输入框只是重录入口', () => {
    const view = apiKeyView(true, '');
    expect(view.badgeVisible).toBe(true);
    expect(view.clearVisible).toBe(true);
    expect(view.canSaveKey).toBe(false);
    expect(view.placeholderKey).toBe('settings.apiKeyPlaceholderConfigured');
  });

  it('已配置且正在输入：徽标仍在（描述已存状态），重录可保存', () => {
    const view = apiKeyView(true, 'sk-new');
    expect(view.badgeVisible).toBe(true);
    expect(view.clearVisible).toBe(true);
    expect(view.canSaveKey).toBe(true);
  });

  it('空白草稿视同无草稿', () => {
    expect(apiKeyView(false, '   ').canSaveKey).toBe(false);
  });
});

describe('aiStatusView —— 总开关状态渲染', () => {
  it('开关关闭：显示已关闭，无论配置是否齐备', () => {
    expect(
      aiStatusView({ enabled: false, baseUrl: null, model: null, apiKeyConfigured: false }).key,
    ).toBe('settings.aiStatus.off');
    expect(
      aiStatusView({
        enabled: false,
        baseUrl: 'https://api.example.com/v1',
        model: 'm',
        apiKeyConfigured: true,
      }).key,
    ).toBe('settings.aiStatus.off');
  });

  it('开关开启且配置齐备：显示已开启', () => {
    expect(
      aiStatusView({
        enabled: true,
        baseUrl: 'https://api.example.com/v1',
        model: 'm',
        apiKeyConfigured: true,
      }).key,
    ).toBe('settings.aiStatus.on');
  });

  it('开关开启但缺任一配置：显示配置不完整', () => {
    const complete = { enabled: true, baseUrl: 'b', model: 'm', apiKeyConfigured: true };
    expect(aiStatusView({ ...complete, baseUrl: null }).key).toBe('settings.aiStatus.incomplete');
    expect(aiStatusView({ ...complete, model: null }).key).toBe('settings.aiStatus.incomplete');
    expect(aiStatusView({ ...complete, apiKeyConfigured: false }).key).toBe(
      'settings.aiStatus.incomplete',
    );
  });
});

describe('draftFromSnapshot —— 表单草稿随快照回填（重开设置不失已存配置）', () => {
  it('快照含 base/model：草稿回填为已存值（重开设置可看到已保存配置）', () => {
    expect(
      draftFromSnapshot({
        baseUrl: 'http://127.0.0.1:18787/v1',
        model: 'mock-model',
        apiKeyConfigured: true,
        enabled: false,
      }),
    ).toEqual({ baseUrl: 'http://127.0.0.1:18787/v1', model: 'mock-model' });
  });

  it('快照为 null（尚未读到）：草稿为空串而非 null', () => {
    expect(draftFromSnapshot(null)).toEqual({ baseUrl: '', model: '' });
  });

  it('快照字段为 null（未配置项）：对应草稿为空串', () => {
    expect(
      draftFromSnapshot({
        baseUrl: null,
        model: 'm1',
        apiKeyConfigured: false,
        enabled: false,
      }),
    ).toEqual({ baseUrl: '', model: 'm1' });
  });
});

describe('testResultView —— 连接测试结果投影', () => {
  it('成功：ok 语气', () => {
    expect(testResultView({ ok: true, httpStatus: 200, detail: null })).toEqual({
      tone: 'ok',
      key: 'settings.testResult.ok',
    });
  });

  it('HTTP 失败：带状态码参数', () => {
    expect(testResultView({ ok: false, httpStatus: 401, detail: 'bad' })).toEqual({
      tone: 'fail',
      key: 'settings.testResult.failStatus',
      params: { status: 401 },
    });
  });

  it('传输层失败：无状态码，展示脱敏诊断信息', () => {
    expect(testResultView({ ok: false, httpStatus: null, detail: 'connection refused' })).toEqual({
      tone: 'fail',
      key: 'settings.testResult.failTransport',
      params: { message: 'connection refused' },
    });
  });
});

describe('settingsErrorView —— 错误码到 i18n 键的映射', () => {
  it('已知错误码映射到 settings.error.*', () => {
    expect(settingsErrorView({ code: 'ai_disabled', message: 'x' }).key).toBe(
      'settings.error.ai_disabled',
    );
    expect(settingsErrorView({ code: 'ai_no_api_key', message: 'x' }).key).toBe(
      'settings.error.ai_no_api_key',
    );
    expect(settingsErrorView({ code: 'invalid_base_url', message: 'x' }).key).toBe(
      'settings.error.invalid_base_url',
    );
    expect(settingsErrorView({ code: 'secret_store_error', message: 'x' }).key).toBe(
      'settings.error.secret_store_error',
    );
  });

  it('未知错误码回退到 generic 并携带原始消息', () => {
    const view = settingsErrorView({ code: 'mystery', message: 'boom' });
    expect(view.key).toBe('settings.error.generic');
    expect(view.params).toEqual({ message: 'boom' });
  });
});
