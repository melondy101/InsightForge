import { invoke } from '@tauri-apps/api/core';

export interface CommandErrorShape {
  code: string;
  message: string;
}

export function isCommandError(value: unknown): value is CommandErrorShape {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as CommandErrorShape).code === 'string' &&
    typeof (value as CommandErrorShape).message === 'string'
  );
}

/** 把 invoke 的 rejection 规整为稳定结构，避免到处做类型收窄。 */
export function toCommandError(err: unknown): CommandErrorShape {
  if (isCommandError(err)) {
    return err;
  }
  return { code: 'unknown', message: err instanceof Error ? err.message : String(err) };
}

export async function callCommand<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    throw toCommandError(err);
  }
}
