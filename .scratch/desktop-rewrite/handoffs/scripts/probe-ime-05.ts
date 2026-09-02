/** IME 组合探针：测试 Input.imeSetComposition 与合成 CompositionEvent 两条路径。 */
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

// 聚焦编辑器内容元素
await evaluate(`document.querySelector('.cm-content').focus(); document.activeElement && document.activeElement.className`);

console.log('--- A: Input.imeSetComposition（CDP 原生组合） ---');
await send('Input.imeSetComposition', { text: 'pinyin', selectionStart: 6, selectionEnd: 6 });
await new Promise((ok) => setTimeout(ok, 300));
console.log('after preEdit, getValue:', JSON.stringify(await evaluate('window.__ifEditor.getValue()')));
console.log('composition node count:', await evaluate(`document.querySelectorAll('.cm-content [contenteditable] .cm-composition, .cm-specialChar, .cm-content *').length`));
await send('Input.imeSetComposition', { text: '拼音一', selectionStart: 3, selectionEnd: 3 });
await new Promise((ok) => setTimeout(ok, 300));
console.log('after update, getValue:', JSON.stringify(await evaluate('window.__ifEditor.getValue()')));
await send('Input.insertText', { text: '' });
await new Promise((ok) => setTimeout(ok, 300));
console.log('after commit attempt, getValue:', JSON.stringify(await evaluate('window.__ifEditor.getValue()')));

console.log('--- B: 合成 CompositionEvent + DOM 变更 ---');
const syntheticResult = await evaluate(`(() => {
  const content = document.querySelector('.cm-content');
  content.focus();
  // 光标移到文末
  const sel = window.getSelection();
  const range = document.createRange();
  range.selectNodeContents(content);
  range.collapse(false);
  sel.removeAllRanges();
  sel.addRange(range);
  content.dispatchEvent(new CompositionEvent('compositionstart', { data: '' }));
  const before = window.__ifEditor.getValue();
  // 模拟 IME 组合：在光标处插入组合文本节点
  const textNode = document.createTextNode('hebing chuan');
  content.appendChild(textNode);
  content.dispatchEvent(new CompositionEvent('compositionupdate', { data: 'hebing chuan' }));
  return { before };
})()`);
console.log('B before:', JSON.stringify(syntheticResult));
await new Promise((ok) => setTimeout(ok, 300));
console.log('B during composition, getValue:', JSON.stringify(await evaluate('window.__ifEditor.getValue()')));
await evaluate(`(() => {
  const content = document.querySelector('.cm-content');
  const sel = window.getSelection();
  // 候选确认：把组合串替换为最终文本（直接改 DOM，CM 的 MutationObserver 读取）
  const nodes = [];
  for (const node of content.childNodes) {
    if (node.nodeType === 3 && node.textContent === 'hebing chuan') {
      nodes.push(node);
    }
  }
  for (const node of nodes) {
    node.textContent = '合并串';
  }
  content.dispatchEvent(new CompositionEvent('compositionupdate', { data: '合并串' }));
  content.dispatchEvent(new CompositionEvent('compositionend', { data: '合并串' }));
})()`);
await new Promise((ok) => setTimeout(ok, 300));
console.log('B after end, getValue:', JSON.stringify(await evaluate('window.__ifEditor.getValue()')));
ws.close();
