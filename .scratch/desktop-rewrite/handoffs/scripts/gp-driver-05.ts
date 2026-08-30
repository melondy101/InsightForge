/**
 * Golden Path UI 驱动（ticket 05 · bun + CDP）。
 *
 * 基于 02 号票的 gp-driver.ts（attempt 1 遗留、attempt 2 收编）扩展：
 * - 已知缺陷修复：启动后先等应用真正挂载（app root + 指定元素出现）再驱动，
 *   避免「[vite] connected 尚未完成就被驱动」的竞态。
 * - 新增 action：drag（CDP 鼠标按下/移动/释放，驱动真实指针事件）、
 *   insertText（CDP 文本提交，等价 IME 上屏路径）、
 *   composition（Input.imeSetComposition 组合窗模拟 + 提交）。
 *
 * 用法：bun gp-driver-05.ts <steps.json> <log-prefix> [outDir]
 */

import { mkdirSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

interface Step {
  action:
    | 'click'
    | 'type'
    | 'screenshot'
    | 'eval'
    | 'wait'
    | 'waitFor'
    | 'log'
    | 'drag'
    | 'insertText'
    | 'composition'
    | 'expectValueEqualsInserted';
  selector?: string;
  text?: string;
  file?: string;
  expr?: string;
  expect?: string;
  ms?: number;
  timeoutMs?: number;
  note?: string;
  dx?: number;
  dy?: number;
  steps?: number;
  preEdit?: string;
  commit?: string;
}

const [stepsPath, prefix = 'gp', outDir = '.'] = process.argv.slice(2);
if (!stepsPath) {
  console.error('usage: bun gp-driver-05.ts <steps.json> [log-prefix] [outDir]');
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

/**
 * 挂载守卫（修复 02 驱动已知缺陷）：等应用真正渲染出目标元素再执行步骤，
 * 避免 vite dev server 尚未完成连接/挂载就被驱动的竞态。
 */
async function waitMounted(timeoutMs = 45000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    try {
      const ready = await evaluate(
        "Boolean(document.querySelector('[data-el=\"workspace-panes\"]')) && document.readyState === 'complete'",
      );
      if (ready) {
        return true;
      }
    } catch {
      // 页面尚未就绪（CDP 早连上、vite 还没注入），继续等。
    }
    if (Date.now() > deadline) {
      return false;
    }
    await new Promise((ok) => setTimeout(ok, 200));
  }
}

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
  const out = resolve(outDir, file);
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

async function centerOf(selector: string): Promise<{ x: number; y: number }> {
  const rect = (await evaluate(`(() => {
    const el = document.querySelector(${JSON.stringify(selector)});
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`)) as { x: number; y: number } | null;
  if (!rect) {
    throw new Error(`element not found: ${selector}`);
  }
  return rect;
}

async function drag(selector: string, dx: number, steps = 20): Promise<void> {
  const start = await centerOf(selector);
  await send('Input.dispatchMouseEvent', {
    type: 'mousePressed',
    x: start.x,
    y: start.y,
    button: 'left',
    buttons: 1,
    clickCount: 1,
  });
  for (let i = 1; i <= steps; i += 1) {
    const x = start.x + (dx * i) / steps;
    await send('Input.dispatchMouseEvent', {
      type: 'mouseMoved',
      x,
      y: start.y,
      button: 'left',
      buttons: 1,
    });
    await new Promise((ok) => setTimeout(ok, 20));
  }
  await send('Input.dispatchMouseEvent', {
    type: 'mouseReleased',
    x: start.x + dx,
    y: start.y,
    button: 'left',
    buttons: 0,
    clickCount: 1,
  });
}

/** 聚焦 CM contenteditable 并把光标移到文末（imeSetComposition/insertText 的前置条件）。 */
async function focusEditorAtEnd(): Promise<unknown> {
  const focused = await evaluate(
    `(() => { const c = document.querySelector('.cm-content'); c.focus(); return document.activeElement === c; })()`,
  );
  await evaluate(
    'window.__ifEditor.setSelection({ anchor: window.__ifEditor.getValue().length, head: window.__ifEditor.getValue().length })',
  );
  return focused;
}

let insertedText = '';

function log(line: string) {
  const line2 = `[${prefix}] ${new Date().toISOString()} ${line}`;
  console.log(line2);
  logLines.push(line2);
}

const logLines: string[] = [];

const mounted = await waitMounted();
log(`mount-guard -> app mounted=${mounted}（等 [data-el="workspace-panes"] 出现）`);

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
    } else if (step.action === 'drag') {
      await drag(step.selector!, step.dx ?? 0, step.steps ?? 12);
      log(`${tag} -> dragged ${step.dx ?? 0}px${step.note ? ` (${step.note})` : ''}`);
    } else if (step.action === 'insertText') {
      await focusEditorAtEnd();
      await send('Input.insertText', { text: step.text ?? '' });
      insertedText += step.text ?? '';
      const length = await evaluate('window.__ifEditor ? window.__ifEditor.getValue().length : -1');
      log(`${tag} -> inserted ${step.text?.length ?? 0} chars, doc length=${JSON.stringify(length)} (expected ${insertedText.length})`);
    } else if (step.action === 'composition') {
      // 组合窗模拟（探针实证的 WebView2 CDP 提交序列）：
      // 拼音 preEdit 上屏（组合中）→ 候选确认替换组合文本 → insertText(commit) 结束组合并上屏一份。
      await focusEditorAtEnd();
      await send('Input.imeSetComposition', {
        text: step.preEdit ?? '',
        selectionStart: (step.preEdit ?? '').length,
        selectionEnd: (step.preEdit ?? '').length,
      });
      await send('Input.imeSetComposition', {
        text: step.commit ?? '',
        selectionStart: (step.commit ?? '').length,
        selectionEnd: (step.commit ?? '').length,
      });
      await send('Input.insertText', { text: step.commit ?? '' });
      insertedText += step.commit ?? '';
      const value = await evaluate('window.__ifEditor ? window.__ifEditor.getValue() : null');
      log(`${tag} -> composition commit=${JSON.stringify(step.commit)}, doc tail=${JSON.stringify(String(value).slice(-16))} (expected tail ${JSON.stringify(insertedText.slice(-16))})`);
    } else if (step.action === 'expectValueEqualsInserted') {
      const value = (await evaluate('window.__ifEditor ? window.__ifEditor.getValue() : null')) as string | null;
      const ok = value === insertedText;
      log(`${tag} -> ${ok ? 'PASS' : 'FAIL'}: doc ${value?.length} chars vs inserted ${insertedText.length} chars${ok ? '' : ` (doc tail=${JSON.stringify(value?.slice(-40))}, inserted tail=${JSON.stringify(insertedText.slice(-40))})`}${step.note ? ` (${step.note})` : ''}`);
    } else if (step.action === 'log') {
      log(`${tag} -> ${step.note ?? ''}`);
    }
  } catch (err) {
    log(`${tag} -> ERROR: ${err instanceof Error ? err.message : String(err)}`);
  }
}

ws.close();
const logPath = resolve(outDir, `${prefix}-steps.log`);
await Bun.write(logPath, logLines.join('\n') + '\n');
console.log(`[gp] driver done, log at ${logPath}`);
