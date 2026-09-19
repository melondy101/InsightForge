import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/**
 * 前端与命令层之间的唯一一条通道（ADR 0002 §6）。
 *
 * 各业务 facade 只依赖 [`CommandClient`] 这一个接口，不再各自复制 `invoke` 与错误收窄：
 * 换 runtime 或写测试时换的是通道实现，不是 8 份调用点。
 *
 * 三个 adapter：
 * - 桌面：`tauriCommandClient`，经 Tauri invoke 直达 Rust 命令层；
 * - 网页：`createHttpCommandClient`，把同一批命令名 POST 给同步服务端；
 * - 测试：`createFakeCommandClient`，在内存里回答并记录调用。
 *
 * 「Tauri 专有 API 只能出现在这个文件里」是这条 seam 的立身之本：
 * 任何视图直接 import `@tauri-apps/*`，浏览器 runtime 就会在那一行崩。
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
  /**
   * 应用版本。桌面取自 Tauri 运行时，网页取自构建期注入值——
   * 诊断快照需要它，但视图不该知道它从哪来。
   */
  appVersion(): Promise<string>;
  /**
   * 订阅 runtime 的事件通道，返回取消订阅函数。
   *
   * 桌面是 Tauri 事件，网页是服务端推的事件流；两边的 payload 形状一致。
   * 这条也在 seam 里面：视图直接 import `@tauri-apps/api/event`，
   * 浏览器 runtime 就会在挂载那一行崩。
   */
  subscribe(channel: string, handler: (payload: unknown) => void): () => void;
}

export interface CommandCall {
  command: string;
  args: CommandArgs;
}

export type FakeCommandClient = CommandClient & {
  /** 按调用顺序记录，供断言「facade 到底发了哪条命令」。 */
  readonly calls: CommandCall[];
  /** 测试里主动推一条事件，等价于 Rust 侧 `emit`。 */
  emit(channel: string, payload: unknown): void;
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
  async appVersion() {
    return getVersion();
  },
  subscribe(channel, handler) {
    let unlisten: UnlistenFn | null = null;
    let closed = false;
    void listen(channel, (event) => handler(event.payload)).then((fn) => {
      // 组件可能已经卸载：注册比卸载慢时，立刻反向取消，别留悬空监听。
      if (closed) {
        fn();
        return;
      }
      unlisten = fn;
    });
    return () => {
      closed = true;
      unlisten?.();
    };
  },
};

/**
 * 内存假实现：让测试经同一条 seam 驱动真实 facade，
 * 而不是伪造 `@tauri-apps/api/core` 模块。
 */
export function createFakeCommandClient(
  handlers: Record<string, (args: CommandArgs) => unknown> = {},
  version = '0.0.0-test',
): FakeCommandClient {
  const calls: CommandCall[] = [];
  const subscribers = new Map<string, Set<(payload: unknown) => void>>();
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
    async appVersion() {
      return version;
    },
    subscribe(channel, handler) {
      const set = subscribers.get(channel) ?? new Set();
      set.add(handler);
      subscribers.set(channel, set);
      return () => {
        set.delete(handler);
      };
    },
    emit(channel, payload) {
      for (const handler of subscribers.get(channel) ?? []) {
        handler(payload);
      }
    },
  };
}

export interface HttpCommandClientOptions {
  /** 同步服务端根地址，如 `https://sync.example.test`。 */
  baseUrl: string;
  /** 构建期注入的应用版本（网页没有 Tauri 运行时可问）。 */
  version: string;
  /** 账号体系未定（ADR 0002 后续），所以只留这一个凭据注入点。 */
  token?: () => string | null | Promise<string | null>;
  /** 请求超时；到点 abort 并报 `timeout`，不让网页端的按钮永远转圈。 */
  timeoutMs?: number;
  fetchImpl?: typeof fetch;
  /** 事件流工厂，默认浏览器 EventSource；测试里注入假的。 */
  eventSource?: (url: string) => EventStream;
}

/** [`EventSource`] 的最小子集，便于注入与卸载。 */
export interface EventStream {
  onmessage: ((event: { data: string }) => void) | null;
  close: () => void;
}

const STATUS_CODES: Record<number, string> = {
  400: 'bad_request',
  401: 'unauthorized',
  403: 'forbidden',
  404: 'unknown_command',
  409: 'conflict',
  429: 'rate_limited',
};

/** 服务端可能回非 JSON（反代 502 页面之类），所以先看形状再退到状态名。 */
function httpCommandError(status: number, body: string): CommandErrorShape {
  try {
    const parsed: unknown = JSON.parse(body);
    if (isCommandError(parsed)) {
      return parsed;
    }
  } catch {
    /* 落到下面的状态码映射 */
  }
  return {
    code: STATUS_CODES[status] ?? 'http_error',
    message: body.slice(0, 200) || `HTTP ${status}`,
  };
}

/**
 * 网页 runtime 的 adapter：命令名与参数原样 POST 到 `{baseUrl}/rpc/{command}`，
 * 服务端把同一批 Tauri 命令暴露成 HTTP（契约见 ADR 0002 §1）。
 * 失败一律收成 [`CommandErrorShape`]，视图层不必知道底下是 invoke 还是 fetch。
 */
export function createHttpCommandClient(options: HttpCommandClientOptions): CommandClient {
  const { baseUrl, version, token, timeoutMs = 15_000 } = options;
  const fetchImpl = options.fetchImpl ?? globalThis.fetch;
  const openStream =
    options.eventSource ??
    ((url: string) => new EventSource(url) as unknown as EventStream);
  const root = baseUrl.replace(/\/+$/, '');

  return {
    async call<T>(command: string, args: CommandArgs = {}): Promise<T> {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), timeoutMs);
      try {
        const credential = await token?.();
        const response = await fetchImpl(`${root}/rpc/${command}`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            ...(credential ? { authorization: `Bearer ${credential}` } : {}),
          },
          body: JSON.stringify(args),
          signal: controller.signal,
        });
        const body = await response.text();
        if (!response.ok) {
          throw httpCommandError(response.status, body);
        }
        return JSON.parse(body) as T;
      } catch (err) {
        if (isCommandError(err)) {
          throw err;
        }
        if (err instanceof Error && err.name === 'AbortError') {
          throw { code: 'timeout', message: `${command} 未在 ${timeoutMs}ms 内返回` };
        }
        if (err instanceof Error) {
          throw { code: 'network_unreachable', message: err.message };
        }
        throw toCommandError(err);
      } finally {
        clearTimeout(timer);
      }
    },
    async appVersion() {
      return version;
    },
    subscribe(channel, handler) {
      const stream = openStream(`${root}/events/${encodeURIComponent(channel)}`);
      stream.onmessage = (event) => {
        try {
          handler(JSON.parse(event.data) as unknown);
        } catch {
          // 心跳或非 JSON 帧：丢掉即可，不该让一条坏帧打断整个流。
        }
      };
      return () => stream.close();
    },
  };
}

interface RuntimeEnv {
  VITE_API_BASE_URL?: string;
  VITE_APP_VERSION?: string;
}

export interface RuntimeSelection {
  env?: RuntimeEnv;
  /** 桌面 Tauri 桥是否存在；默认探测当前页面的 `window.__TAURI_INTERNALS__`。 */
  hasTauriBridge?: boolean;
}

/**
 * 本次启动该用哪条通道：桌面 Tauri 桥优先，其次网页 HTTP 通道，
 * 都没有就是「未配置的浏览器」——宁可每条命令立刻报 `runtime_unconfigured`，
 * 也不要在用户点了半天之后才以难以理解的方式失败。
 */
export function selectCommandClient({
  env = import.meta.env,
  hasTauriBridge = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window,
}: RuntimeSelection = {}): CommandClient {
  if (hasTauriBridge) {
    return tauriCommandClient;
  }
  const version = env.VITE_APP_VERSION ?? '0.0.0-dev';
  if (env.VITE_API_BASE_URL) {
    return createHttpCommandClient({ baseUrl: env.VITE_API_BASE_URL, version });
  }
  return {
    async call<T>(command: string): Promise<T> {
      throw {
        code: 'runtime_unconfigured',
        message: `${command} 需要桌面运行时或 VITE_API_BASE_URL 指向的同步服务`,
      };
    },
    async appVersion() {
      return version;
    },
    subscribe() {
      return () => {};
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

/** 当前 runtime 的应用版本（诊断快照用）。 */
export function appVersion(): Promise<string> {
  return active.appVersion();
}

/** 订阅当前 runtime 的事件通道，返回取消订阅函数。 */
export function subscribeEvents<T>(
  channel: string,
  handler: (payload: T) => void,
): () => void {
  return active.subscribe(channel, (payload) => handler(payload as T));
}
