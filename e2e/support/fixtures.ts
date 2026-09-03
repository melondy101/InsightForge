import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { test as base } from '@playwright/test';

/**
 * 隔离夹具：每个 E2E 测试拥有独立的临时数据目录，互不污染。
 * 通过 `INSIGHTFORGE_E2E_DATA_DIR` 注入应用（Tauri 启动前读取该环境变量
 * 作为 app_data_dir），避免多个用例共享同一数据库/草稿目录造成串扰。
 */
export interface IsolatedFixtures {
  dataDir: string;
}

export const test = base.extend<IsolatedFixtures>({
  dataDir: async ({}, use) => {
    const dir = mkdtempSync(join(tmpdir(), 'insightforge-e2e-'));
    process.env.INSIGHTFORGE_E2E_DATA_DIR = dir;
    await use(dir);
    rmSync(dir, { recursive: true, force: true });
    delete process.env.INSIGHTFORGE_E2E_DATA_DIR;
  },
});

export { expect } from '@playwright/test';
