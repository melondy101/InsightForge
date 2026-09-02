// C14 CDP Golden Path — 20 runs using await_index_committed
// Launches fresh Tauri dev with --remote-debugging-port=9222 and drives CDP.
//
// USAGE:
//   1. In separate terminal: WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222" bun run tauri dev
//   2. In this terminal: bun run .scratch/golden-path/c14-index-search-contract/cdp-golden-path.mjs
//
// REQUIREMENTS:
//   - Tauri dev must be running with debugging enabled
//   - WebView2 is at http://127.0.0.1:9222
//   - App is on its first screen (Sources view visible)

const ENDPOINT = 'http://127.0.0.1:9222';
const RUN_COUNT = 20;
const FIXTURE_DIR_BASE = `D:/Develop/wt/InsightForge/c14-index-search-contract/.scratch/golden-path/c14-index-search-contract/fixtures-${Date.now()}`;
const RESULTS_DIR = `D:/Develop/wt/InsightForge/c14-index-search-contract/.scratch/golden-path/c14-index-search-contract`;

const fs = await import('fs');
const path = await import('path');
const os = await import('os');

// ---- CDP helpers ----

async function connect() {
  for (let attempt = 0; attempt < 30; attempt++) {
    try {
      const targets = await fetch(`${ENDPOINT}/json`).then(r => r.json());
      const page = targets.find(t => t.type === 'page');
      if (page) {
        return { page, ws: null };
      }
    } catch {
      // not ready yet
    }
    await new Promise(r => setTimeout(r, 1000));
  }
  throw new Error('WebView2 not available at 9222 after 30s');
}

function openWs(page) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    const timeout = setTimeout(() => { ws.close(); reject(new Error('WS timeout')); }, 10000);
    ws.addEventListener('open', () => { clearTimeout(timeout); resolve(ws); });
    ws.addEventListener('error', (e) => { clearTimeout(timeout); reject(e); });
  });
}

function send(ws, method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = Math.floor(Math.random() * 1e6);
    const handler = (ev) => {
      const m = JSON.parse(ev.data);
      if (m.id === id) {
        ws.removeEventListener('message', handler);
        if (m.error) reject(new Error(JSON.stringify(m.error)));
        else resolve(m);
      }
    };
    ws.addEventListener('message', handler);
    ws.send(JSON.stringify({ id, method, params }));
  });
}

async function ev(ws, expr, opts = {}) {
  const r = await send(ws, 'Runtime.evaluate', {
    expression: expr,
    returnByValue: true,
    awaitPromise: opts.awaitPromise ?? false,
    userGesture: opts.userGesture ?? false,
  });
  if (r.result?.exceptionDetails) {
    return { ok: false, err: r.result.exceptionDetails.description || r.result.exceptionDetails.text };
  }
  return { ok: true, v: r.result?.result?.value };
}

// ---- main ----

async function main() {
  fs.mkdirSync(FIXTURE_DIR_BASE, { recursive: true });
  fs.mkdirSync(`${RESULTS_DIR}/screenshots`, { recursive: true });

  console.log(`[C14] Waiting for WebView2 at ${ENDPOINT}...`);
  const { page } = await connect();
  console.log(`[C14] Connected. WebView2 title: ${page.title}`);

  const ws = await openWs(page);
  console.log('[C14] CDP WebSocket connected');

  await send(ws, 'Runtime.enable');
  await send(ws, 'Page.enable');

  // Install event listener (one-shot for all 20 iterations)
  const evSetup = await ev(ws, `(() => {
    window.__c14_evs = [];
    const handler = window.__TAURI_INTERNALS__.transformCallback((e) => { window.__c14_evs.push({ ts: Date.now(), payload: e.payload }); }, false);
    return window.__TAURI_INTERNALS__.invoke('plugin:event|listen', { event: 'sources://progress', target: { kind: 'Any' }, handler }).then(() => true);
  })()`, { awaitPromise: true });
  console.log(`[C14] Event listener: ${evSetup.ok ? 'OK' : evSetup.err}`);

  // Verify the await_index_committed command is available
  const cmdCheck = await ev(ws, `typeof window.__TAURI_INTERNALS__.invoke('sources_await_index_committed')`, { awaitPromise: false });
  console.log(`[C14] sources_await_index_committed available: ${cmdCheck.ok ? cmdCheck.v : cmdCheck.err}`);

  await new Promise(r => setTimeout(r, 500));

  const results = [];
  const runStart = Date.now();

  for (let i = 0; i < RUN_COUNT; i++) {
    const FIXTURE_DIR = `${FIXTURE_DIR_BASE}/run-${i}`;
    fs.mkdirSync(FIXTURE_DIR, { recursive: true });
    const token = `c14gp${i}_${Date.now()}`;
    fs.writeFileSync(path.join(FIXTURE_DIR, 'doc.md'), `# Doc ${i}\nUnique token: ${token}\nSome content here.\n`);

    const runStartMs = Date.now();

    // Add source
    const addR = await ev(ws, `window.__TAURI_INTERNALS__.invoke('sources_add', { root: ${JSON.stringify(FIXTURE_DIR)}, name: 'c14gp-${i}' }).then(r => JSON.stringify(r)).catch(e => 'ERR:' + (e?.message || JSON.stringify(e)))`, { awaitPromise: true });
    let sid = null, addErr = null;
    try { const parsed = JSON.parse(addR.v); if (parsed.id) sid = parsed.id; else addErr = addR.v; } catch { addErr = addR.v; }

    if (!sid) {
      results.push({ i, pass: false, reason: 'add_failed', detail: addErr, elapsedMs: Date.now() - runStartMs });
      continue;
    }

    // **Core contract: await_index_committed** — blocks until FTS committed
    const awaitR = await ev(ws, `window.__TAURI_INTERNALS__.invoke('sources_await_index_committed', { sourceId: ${JSON.stringify(sid)} }).then(r => JSON.stringify(r)).catch(e => 'ERR:' + (e?.message || JSON.stringify(e)))`, { awaitPromise: true });
    let indexStatus = null, awaitErr = null;
    try { indexStatus = JSON.parse(awaitR.v); } catch { awaitErr = awaitR.v; }

    if (!indexStatus || !indexStatus.searchable) {
      results.push({ i, pass: false, reason: 'await_not_searchable', detail: awaitErr || indexStatus, elapsedMs: Date.now() - runStartMs });
      await ev(ws, `window.__TAURI_INTERNALS__.invoke('sources_remove', { id: ${JSON.stringify(sid)} }).catch(()=>{})`, { awaitPromise: true }).catch(() => {});
      continue;
    }

    // Immediately search (no sleep, no polling)
    const searchR = await ev(ws, `window.__TAURI_INTERNALS__.invoke('search_query', { sourceId: ${JSON.stringify(sid)}, query: ${JSON.stringify(token)}, limit: 5 }).then(r => JSON.stringify(r)).catch(e => 'ERR:' + (e?.message || JSON.stringify(e)))`, { awaitPromise: true });
    let cards = -1, searchErr = null;
    try { const parsed = JSON.parse(searchR.v); cards = Array.isArray(parsed) ? parsed.length : -1; } catch { searchErr = searchR.v; }

    // Take screenshot every 5th run
    if (i % 5 === 0) {
      const screenshot = await send(ws, 'Page.captureScreenshot', { format: 'png' }).catch(() => null);
      if (screenshot?.result?.data) {
        fs.writeFileSync(`${RESULTS_DIR}/screenshots/run-${i}.png`, Buffer.from(screenshot.result.data, 'base64'));
      }
    }

    const pass = cards > 0;
    results.push({
      i, pass, sid, token,
      fragmentCount: indexStatus.fragmentCount,
      fileCount: indexStatus.fileCount,
      searchable: indexStatus.searchable,
      cards, searchErr,
      elapsedMs: Date.now() - runStartMs,
    });

    // Cleanup
    await ev(ws, `window.__TAURI_INTERNALS__.invoke('sources_remove', { id: ${JSON.stringify(sid)} }).catch(()=>{})`, { awaitPromise: true }).catch(() => {});
    await new Promise(r => setTimeout(r, 100));
  }

  const totalMs = Date.now() - runStart;
  const passed = results.filter(r => r.pass);
  const failed = results.filter(r => !r.pass);

  console.log('\n=== C14 CDP Golden Path Results ===');
  console.log(JSON.stringify({
    run_count: RUN_COUNT,
    passed: passed.length,
    failed: failed.length,
    totalMs,
    avgMs: Math.round(totalMs / RUN_COUNT),
    failures: failed.map(f => ({ i: f.i, reason: f.reason, detail: f.searchErr || f.detail })),
    console_logs: [],
    console_errors: [],
  }, null, 2));

  // Write results
  fs.writeFileSync(`${RESULTS_DIR}/results.json`, JSON.stringify({ results, summary: { run_count: RUN_COUNT, passed: passed.length, failed: failed.length, totalMs, avgMs: Math.round(totalMs / RUN_COUNT), failures: failed.map(f => ({ i: f.i, reason: f.reason, detail: f.searchErr || f.detail })) } }, null, 2));

  ws.close();
  process.exit(failed.length > 0 ? 1 : 0);
}

main().catch(e => { console.error('[C14] Fatal:', e); process.exit(1); });
