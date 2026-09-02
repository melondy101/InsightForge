/** IME 提交语义探针 v3：组合后用 insertText(commit) 提交（应替换组合文本、只上一份）。 */
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

const tail = async () => JSON.stringify(String(await evaluate('window.__ifEditor.getValue()')));

// 1. composition('pinyin') → composition('中文上屏') → insertText('中文上屏')
await send('Input.imeSetComposition', { text: 'zhong wen shang ping', selectionStart: 20, selectionEnd: 20 });
await new Promise((ok) => setTimeout(ok, 150));
console.log('preEdit:', await tail());
await send('Input.imeSetComposition', { text: '中文上屏', selectionStart: 4, selectionEnd: 4 });
await new Promise((ok) => setTimeout(ok, 150));
console.log('candidate:', await tail());
await send('Input.insertText', { text: '中文上屏' });
await new Promise((ok) => setTimeout(ok, 150));
console.log('after insertText(commit):', await tail());

// 2. 紧接普通 insertText，验证光标连续
await send('Input.insertText', { text: '，然后继续。' });
await new Promise((ok) => setTimeout(ok, 150));
console.log('after continue:', await tail());
console.log('cursor:', await evaluate(`(() => { const s = window.__ifEditor.getSelection(); const len = window.__ifEditor.getValue().length; return { len, cursorAtEnd: Math.max(s.anchor, s.head) === len }; })()`));
ws.close();
