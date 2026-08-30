/** 拖拽探针：检查 CDP Input.dispatchMouseEvent 是否产生 pointer 事件并驱动分隔条。 */
const cdp = await fetch('http://127.0.0.1:9222/json')
  .then((r) => r.json() as Promise<{ type: string; webSocketDebuggerUrl: string }[]>)
  .then((targets) => targets.find((t) => t.type === 'page'));
const ws = new WebSocket(cdp!.webSocketDebuggerUrl);
await new Promise<void>((ok, err) => {
  ws.onopen = () => ok();
  ws.onerror = () => err(new Error('ws error'));
});
let nextId = 1;
const pending = new Map<number, { ok: (v: unknown) => void; fail: (e: Error) => void }>();
ws.onmessage = (message) => {
  const data = JSON.parse(String(message.data));
  if (data.id !== undefined && pending.has(data.id)) {
    const entry = pending.get(data.id)!;
    pending.delete(data.id);
    data.error ? entry.fail(new Error(JSON.stringify(data.error))) : entry.ok(data.result);
  }
};
function send(method: string, params: Record<string, unknown> = {}): Promise<Record<string, unknown>> {
  const id = nextId++;
  return new Promise((ok, fail) => {
    pending.set(id, { ok, fail });
    ws.send(JSON.stringify({ id, method, params }));
  });
}
async function evaluate(expression: string): Promise<unknown> {
  const result = await send('Runtime.evaluate', { expression, returnByValue: true, awaitPromise: true, userGesture: true });
  if (result.exceptionDetails !== undefined) {
    throw new Error(JSON.stringify(result.exceptionDetails));
  }
  return (result.result as { value?: unknown }).value;
}

// 1. 在分隔条上装事件计数器
await evaluate(`(() => {
  window.__probe = { down: 0, move: 0, up: 0, rect: null };
  const el = document.querySelector('[data-el="divider-left"]');
  for (const type of ['pointerdown', 'pointermove', 'pointerup']) {
    el.addEventListener(type, () => { window.__probe[type.replace('pointer', '')] += 1; });
  }
  const r = el.getBoundingClientRect();
  window.__probe.rect = { x: r.x, y: r.y, w: r.width, h: r.height };
  return window.__probe.rect;
})()`);

const rect = (await evaluate(`window.__probe.rect`)) as { x: number; y: number; w: number; h: number };
console.log('divider rect', rect);
const startX = rect.x + rect.w / 2;
const startY = rect.y + rect.h / 2;

// 2. CDP 鼠标序列
await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: startX, y: startY, button: 'left', buttons: 1, clickCount: 1 });
for (let i = 1; i <= 10; i += 1) {
  await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: startX + i * 10, y: startY, buttons: 1, button: 'left' });
  await new Promise((ok) => setTimeout(ok, 30));
}
await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: startX + 100, y: startY, button: 'left', buttons: 0, clickCount: 1 });
await new Promise((ok) => setTimeout(ok, 200));

console.log('probe events:', await evaluate('window.__probe.down + "/" + window.__probe.move + "/" + window.__probe.up'));
console.log('center ratio now:', await evaluate(`(() => { const c = document.querySelector('[data-el="workspace-pane-center"]').getBoundingClientRect().width; const t = document.querySelector('[data-el="workspace-panes"]').getBoundingClientRect().width; return Math.round(c / t * 1000) / 1000; })()`));
ws.close();
