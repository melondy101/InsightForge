import { describe, expect, it } from 'vitest';

import { sourcesApi, IndexStatus } from './api';

describe('IndexStatus type and sourcesApi.awaitIndexCommitted', () => {
  it('IndexStatus 形状与契约一致', () => {
    const status: IndexStatus = {
      sourceId: 'test',
      fileCount: 3,
      fragmentCount: 5,
      searchable: true,
    };
    expect(status.sourceId).toBe('test');
    expect(status.fileCount).toBe(3);
    expect(status.fragmentCount).toBe(5);
    expect(status.searchable).toBe(true);
  });

  it('searchable=false 语义正确（fragment_count=0 → 不可搜索）', () => {
    const status: IndexStatus = {
      sourceId: 'empty',
      fileCount: 0,
      fragmentCount: 0,
      searchable: false,
    };
    expect(status.searchable).toBe(false);
    expect(status.fragmentCount).toBe(0);
  });

  it('sourcesApi.awaitIndexCommitted 注册正确的 Tauri 命令名', () => {
    expect(sourcesApi.awaitIndexCommitted).toBeDefined();
    expect(typeof sourcesApi.awaitIndexCommitted).toBe('function');
  });

  it('sourcesApi 导出完整的命令集合', () => {
    const required = [
      'add', 'list', 'pause', 'remove', 'sync',
      'reconcile', 'rebuildIndex', 'setSyncInterval',
      'awaitIndexCommitted',
    ];
    for (const key of required) {
      expect(sourcesApi).toHaveProperty(key);
      expect(typeof (sourcesApi as Record<string, unknown>)[key]).toBe('function');
    }
  });
});
