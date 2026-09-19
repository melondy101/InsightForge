export interface SnippetPart {
  text: string;
  marked: boolean;
}

const OPEN = '<mark>';
const CLOSE = '</mark>';

/**
 * FTS5 `snippet(..., '<mark>', '</mark>', ...)` 返回的是用户原文本身，只在命中词两侧加标记。
 * 因此除标记外的一切都必须按字面文本渲染，否则知识库文件里的 HTML 会在 renderer 里执行。
 */
export function splitSnippet(snippet: string): SnippetPart[] {
  const parts: SnippetPart[] = [];
  let cursor = 0;
  let marked = false;

  while (cursor < snippet.length) {
    const open = snippet.indexOf(OPEN, cursor);
    const close = snippet.indexOf(CLOSE, cursor);
    const next = nextMarker(open, close);
    if (next === null) {
      parts.push({ text: snippet.slice(cursor), marked });
      break;
    }
    const [index, length] = next;
    if (index > cursor) parts.push({ text: snippet.slice(cursor, index), marked });
    marked = length === OPEN.length;
    cursor = index + length;
  }

  return parts.filter((part) => part.text !== '');
}

// 标记都是纯 ASCII，在它们的边界上切分不会拆到代理对，所以按索引切片是安全的。
function nextMarker(open: number, close: number): [number, number] | null {
  if (open < 0 && close < 0) return null;
  if (open < 0) return [close, CLOSE.length];
  if (close < 0) return [open, OPEN.length];
  return open < close ? [open, OPEN.length] : [close, CLOSE.length];
}
