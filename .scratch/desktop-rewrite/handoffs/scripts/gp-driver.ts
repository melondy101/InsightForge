/**
 * Golden Path UI 驱动（bun + CDP）：
 * 经 WebView2 远程调试口在真实窗口上按选择器驱动真实用户操作
 * （点击 / 输入 / 对话框 / 截图 / 状态断言），逐步输出「预期 vs 实际」日志。
 *
 * 用法：bun gp-driver.ts <steps.json> <log-prefix>
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

interface Step {
  action: 'click' | 'type' | 'screenshot' | 'eval' | 'wait' | 'waitFor' | 'log';
  selector?: string;
  text?: string;
  file?: string;
  expr?: string;
  expect?: string;
  ms?: number;
  timeoutMs?: number;
  note?: string;
}

const [stepsPath, prefix = 'gp'] = process.argv.slice(2);
if (!stepsPath) {
  console.error('usage: bun gp-driver.ts <steps.json> [log-prefix]');
  process.exit(2);
}
const steps: Step[] = JSON.parse(await Bun.file(stepsPath).text());

const cdp = await fetch('http://127.0.0.1:9222/json')
  .then((r) => r.json() as Promise<{ type: string; webSocketDebuggerUrl: string }[]>)
  .then((targets) => targets.find((t) => t.type === 'page'));
if (!cdp) {
  console.error('no CDP page target found on :9222');
  process.exit(1);
}

const ws = new WebSocket(cdp.webSocketDebuggerUrl);
await new Promise<void>((ok, err) => {
  ws.onopen = () => ok();
  ws.onerror = (e) => err(new Error('ws error'));
});

let nextId = 1;
const pending = new Map<number, { ok: (v: unknown) => void; fail: (e: Error) => void }>();
const events: { method: string; params: Record<string, unknown> }[] = [];

ws.onmessage = (message) => {
  const data = JSON.parse(String(message.data));
  if (data.id !== undefined && pending.has(data.id)) {
    const entry = pending.get(data.id)!;
    pending.delete(data.id);
    if (data.error) {
      entry.fail(new Error(JSON.stringify(data.error)));
    } else {
      entry.ok(data.result);
    }
  } else if (data.method) {
    events.push({ method: data.method, params: data.params ?? {} });
  }
};

function send(method: string, params: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
  const id = nextId++;
  return new Promise((ok, fail) => {
    pending.set(id, { ok, fail });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

await send('Runtime.enable');
await send('Page.enable');

async function evaluate(expression: string): Promise<unknown> {
  const result = await send('Runtime.evaluate', {
    expression,
    returnByValue: true,
    awaitPromise: true,
    userGesture: true,
  });
  if (result.exceptionDetails !== undefined) {
    throw new Error(`evaluate failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return (result.result as { value?: unknown }).value;
}

async function screenshot(file: string): Promise<string> {
  const result = await send('Page.captureScreenshot', { format: 'png' });
  const data = (result.data as string) ?? '';
  const out = resolve(file);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, Buffer.from(data, 'base64'));
  return out;
}

async function waitFor(expr: string, timeoutMs: number): Promise<unknown> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const value = await evaluate(expr);
    if (value) {
      return value;
    }
    if (Date.now() > deadline) {
      return value;
    }
    await new Promise((ok) => setTimeout(ok, 100));
  }
}

function clickWithDialogSupport(selector: string): Promise<unknown> {
  // window.confirm 会阻塞 evaluate：先把「点按钮」丢出去，再等对话框事件处理。
  const clickPromise = evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return false;
    el.click();
    return true;
  })()`);
  const dialogWatch = (async () => {
    const deadline = Date.now() + 5000;
    for (;;) {
      const index = events.findIndex((e) => e.method === 'Page.javascriptDialogOpening');
      if (index >= 0) {
        const dialog = events.splice(index, 1)[0];
        await send('Page.handleJavaScriptDialog', { accept: true });
        return `dialog(${String(dialog.params.message ?? '')}) accepted`;
      }
      if (Date.now() > deadline) {
        return 'no-dialog';
      }
      await new Promise((ok) => setTimeout(ok, 50));
    }
  })();
  return Promise.all([clickPromise, dialogWatch]).then(([clicked, dialog]) => ({ clicked, dialog }));
}

const logLines: string[] = [];
function log(line: string) {
  const line2 = `[${prefix}] ${new Date().toISOString()} ${line}`;
  console.log(line2);
  logLines.push(line2);
}

let index = 0;
for (const step of steps) {
  index += 1;
  const tag = `step-${String(index).padStart(2, '0')}:${step.action}${step.selector ?? ''}`;
  try {
    if (step.action === 'click') {
      const result = await clickWithDialogSupport(step.selector!);
      log(`${tag} -> ${JSON.stringify(result)}${step.note ? ` (${step.note})` : ''}`);
    } else if (step.action === 'type') {
      await evaluate(`(() => {
        const el = document.querySelector(${JSON.stringify(step.selector!)});
        if (!el) return false;
        el.focus();
        document.execCommand('selectAll', false, null);
        document.execCommand('insertText', false, ${JSON.stringify(step.text ?? '')});
        return true;
      })()`);
      const actual = await evaluate(`document.querySelector(${JSON.stringify(step.selector!)})?.value ?? null`);
      log(`${tag} -> filled, actual value=${JSON.stringify(actual)} (expected ${JSON.stringify(step.text)})`);
    } else if (step.action === 'screenshot') {
      const out = await screenshot(step.file!);
      log(`${tag} -> saved ${out}`);
    } else if (step.action === 'eval') {
      const value = await evaluate(step.expr!);
      log(`${tag} -> ${JSON.stringify(value)}${step.expect !== undefined ? ` (expected ${step.expect})` : ''}${step.note ? ` (${step.note})` : ''}`);
    } else if (step.action === 'wait') {
      await new Promise((ok) => setTimeout(ok, step.ms ?? 300));
      log(`${tag} -> waited ${step.ms ?? 300}ms`);
    } else if (step.action === 'waitFor') {
      const value = await waitFor(step.expr!, step.timeoutMs ?? 15000);
      log(`${tag} -> ${JSON.stringify(value)} (truthy=${Boolean(value)})${step.note ? ` (${step.note})` : ''}`);
    } else if (step.action === 'log') {
      log(`${tag} -> ${step.note ?? ''}`);
    }
  } catch (err) {
    log(`${tag} -> ERROR: ${err instanceof Error ? err.message : String(err)}`);
  }
}

ws.close();
await Bun.write(`${prefix}-steps.log`, logLines.join('\n') + '\n');
console.log(`[gp] driver done, log at ${prefix}-steps.log`);
