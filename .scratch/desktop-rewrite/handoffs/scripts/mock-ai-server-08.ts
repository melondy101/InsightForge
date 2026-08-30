/**
 * 本地 mock AI 端点（ticket 08 Golden Path 专用）。
 *
 * 只监听 127.0.0.1（无真实外部网络）；按 OpenAI 兼容形状应答
 * POST {base}/chat/completions → 200 {"choices":[{"message":{...}}]}。
 * 每个请求记录：方法、路径、Authorization 头（ Golden Path 只用假 key
 * `sk-test-fake`，日志留档作「Key 只经 Authorization 头」的佐证）、
 * 请求体中的 model 与 max_tokens。
 */

const PORT = 18787;
const LOG: string[] = [];

function log(line: string) {
  const entry = `[mock-ai] ${new Date().toISOString()} ${line}`;
  console.log(entry);
  LOG.push(entry);
}

Bun.serve({
  hostname: '127.0.0.1',
  port: PORT,
  idleTimeout: 30,
  async fetch(request) {
    const url = new URL(request.url);
    if (request.method !== 'POST' || url.pathname !== '/v1/chat/completions') {
      log(`${request.method} ${url.pathname} -> 404`);
      return new Response('not found', { status: 404 });
    }
    const auth = request.headers.get('authorization') ?? '(missing)';
    const body = await request.text();
    let model = '(unreadable)';
    let maxTokens = '(unreadable)';
    let content = '(unreadable)';
    try {
      const parsed = JSON.parse(body) as {
        model?: string;
        max_tokens?: number;
        messages?: { content?: string }[];
      };
      model = parsed.model ?? '(none)';
      maxTokens = String(parsed.max_tokens ?? '(none)');
      content = parsed.messages?.[0]?.content ?? '(none)';
    } catch {
      // 保持占位值即可，仍返回 200 形状正确的响应。
    }
    log(
      `POST /v1/chat/completions -> 200 | authorization=${auth} | model=${model} | max_tokens=${maxTokens} | prompt=${JSON.stringify(content)}`,
    );
    await Bun.write(
      Bun.fileURLToPath(new URL('08-mock-ai-server-requests.log', import.meta.url)),
      LOG.join('\n') + '\n',
    );
    return Response.json({
      id: 'chatcmpl-mock-08',
      object: 'chat.completion',
      model,
      choices: [
        {
          index: 0,
          message: { role: 'assistant', content: 'mock-pong' },
          finish_reason: 'stop',
        },
      ],
      usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 },
    });
  },
});

log(`mock OpenAI-compatible endpoint listening on http://127.0.0.1:${PORT}/v1`);
