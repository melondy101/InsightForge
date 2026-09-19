import { invoke } from '@tauri-apps/api/core';

/**
 * 前端与 Rust 命令层之间的唯一一条通道（ADR 0002 §6）。
 *
 * 各业务 facade 只依赖 [`CommandClient`] 这一个接口，不再各自复制 `invoke` 与错误收窄：
 * 换 runtime 或写测试时换的是通道实现，不是 8 份调用点。
 */

/** 与 Rust 侧 `CommandError` 的 serde 输出一一对应。 */
export interface CommandErrorShape {
  code: string;
  message: string;
}

export type CommandArgs = Record<string, unknown>;

/** 一次命令调用：命令名 + 参数 → 结果；失败一律抛 [`CommandErrorShape`]。 */
export interface CommandClient {
  call<T>(command: string, args?: CommandArgs): Promise<T>;
}

export interface CommandCall {
  command: string;
  args: CommandArgs;
}

export type FakeCommandClient = CommandClient & {
  /** 按调用顺序记录，供断言「facade 到底发了哪条命令」。 */
  readonly calls: CommandCall[];
};

export function isCommandError(value: unknown): value is CommandErrorShape {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as CommandErrorShape).code === 'string' &&
    typeof (value as CommandErrorShape).message === 'string'
  );
}

/** 把 transport 的 rejection 规整为稳定结构，避免到处做类型收窄。 */
export function toCommandError(err: unknown): CommandErrorShape {
  if (isCommandError(err)) {
    return err;
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

/** 桌面 runtime 的 adapter：命令经 Tauri invoke 直达 Rust 命令层。 */
export const tauriCommandClient: CommandClient = {
  async call<T>(command: string, args?: CommandArgs): Promise<T> {
    try {
      return await invoke<T>(command, args);
    } catch (err) {
      throw toCommandError(err);
    }
  },
};

/**
 * 内存假实现：让测试经同一条 seam 驱动真实 facade，
 * 而不是伪造 `@tauri-apps/api/core` 模块。
 */
export function createFakeCommandClient(
  handlers: Record<string, (args: CommandArgs) => unknown> = {},
): FakeCommandClient {
  const calls: CommandCall[] = [];
  return {
    calls,
    async call<T>(command: string, args: CommandArgs = {}): Promise<T> {
      calls.push({ command, args });
      const handler = handlers[command];
      if (!handler) {
        throw { code: 'unknown_command', message: `no handler for command ${command}` };
      }
      return (await handler(args)) as T;
    },
  };
}

let active: CommandClient = tauriCommandClient;

/** 换掉本进程使用的命令通道，返回还原函数。 */
export function setCommandClient(client: CommandClient): () => void {
  const previous = active;
  active = client;
  return () => {
    active = previous;
  };
}

/** facade 层唯一的调用入口。 */
export function callCommand<T>(command: string, args?: CommandArgs): Promise<T> {
  return active.call<T>(command, args);
}
