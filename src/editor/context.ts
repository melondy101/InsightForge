/**
 * 编辑器上下文提取（纯函数，不依赖任何编辑器实例）。
 *
 * 输入：LF 行尾的文档字符串 + 光标/选区偏移量（UTF-16 码元）。
 * 输出：选区或光标所在段落、前后相邻段落、标题大纲（含层级路径）。
 * 供 09 号票的 EgressBundle 使用；EditorAdapter 负责从编辑器状态取数喂给它。
 *
 * 术语（CONTEXT.md）：段落/标题为通用 Markdown 结构；「片段 Fragment」在桌面版中
 * 指索引分块，与本模块的「段落 paragraph」无关。
 */

export interface EditorSelection {
  /** 选区锚点（文档偏移量）。 */
  anchor: number;
  /** 选区头部（文档偏移量）；与 anchor 相等即纯光标。 */
  head: number;
}

export interface ParagraphSpan {
  /** 起始偏移（含）。 */
  start: number;
  /** 结束偏移（不含，即段落末行的行尾换行符之前）。 */
  end: number;
  /** 段落文本（不含末尾换行）。 */
  text: string;
}

export interface OutlineEntry {
  /** 标题层级 1–6。 */
  level: number;
  /** 标题文本（去掉 # 与空白后）。 */
  text: string;
  /** 祖先标题文本路径（不含自身），如 ['第一章', '第一节']。 */
  path: string[];
  start: number;
  end: number;
}

export interface ExtractedContext {
  before: ParagraphSpan | null;
  current: ParagraphSpan | null;
  after: ParagraphSpan | null;
  outline: OutlineEntry[];
}

interface LineInfo {
  /** 行起始偏移。 */
  start: number;
  /** 行结束偏移（换行符之前）。 */
  end: number;
  /** 是否空行（空或全空白）。 */
  blank: boolean;
}

function mapLines(doc: string): LineInfo[] {
  const lines: LineInfo[] = [];
  let start = 0;
  for (;;) {
    const nl = doc.indexOf('\n', start);
    const end = nl === -1 ? doc.length : nl;
    lines.push({ start, end, blank: doc.slice(start, end).trim().length === 0 });
    if (nl === -1) {
      break;
    }
    start = nl + 1;
  }
  return lines;
}

/** 连续非空行构成一个段落块；返回 [firstLine, lastLine] 的行号区间（含两端）。 */
function blockRangeAt(lines: LineInfo[], lineIndex: number): [number, number] {
  let first = lineIndex;
  let last = lineIndex;
  while (first > 0 && !lines[first - 1].blank) {
    first -= 1;
  }
  while (last < lines.length - 1 && !lines[last + 1].blank) {
    last += 1;
  }
  return [first, last];
}

function spanOf(lines: LineInfo[], doc: string, firstLine: number, lastLine: number): ParagraphSpan {
  return {
    start: lines[firstLine].start,
    end: lines[lastLine].end,
    text: doc.slice(lines[firstLine].start, lines[lastLine].end),
  };
}

/**
 * 选区触碰的行号区间。
 * 行归属用「包含关系」：偏移 o 属于满足 start <= o <= end 的行（行首偏移属于本行，
 * 不属于上一行的换行符）。纯光标取单行；选区取首尾两个字符所在行之间的所有行。
 */
function touchedLines(lines: LineInfo[], from: number, to: number): [number, number] | null {
  const first = lineAtOffset(lines, from);
  const last = from < to ? lineAtOffset(lines, to - 1) : first;
  return [first, last];
}

const ATX_HEADING = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?[ \t]*$/;

export function extractEditorContext(doc: string, selection: EditorSelection): ExtractedContext {
  const lines = mapLines(doc);
  const from = Math.min(selection.anchor, selection.head);
  const to = Math.max(selection.anchor, selection.head);

  const outline = extractOutline(lines, doc);

  let current: ParagraphSpan | null = null;
  let before: ParagraphSpan | null = null;
  let after: ParagraphSpan | null = null;

  const touched = touchedLines(lines, from, to);
  if (touched) {
    const [firstLine, lastLine] = touched;
    const firstNonBlank = findLine(lines, firstLine, lastLine, 'nonblank');
    const lastNonBlank = findLine(lines, lastLine, firstLine, 'nonblank');
    if (firstNonBlank !== null && lastNonBlank !== null) {
      const [firstBlock] = blockRangeAt(lines, firstNonBlank);
      const [, lastBlock] = blockRangeAt(lines, lastNonBlank);
      current = spanOf(lines, doc, firstBlock, lastBlock);
      before = firstBlock > 0 ? nearestBlock(lines, doc, firstBlock - 1, -1) : null;
      after = lastBlock < lines.length - 1 ? nearestBlock(lines, doc, lastBlock + 1, 1) : null;
    }
  }

  if (!current) {
    // 光标在空白处（空行、文档首尾空白）：current 为 null，取两侧最近的段落块。
    const cursorLine = lineAtOffset(lines, from);
    before = cursorLine > 0 ? nearestBlock(lines, doc, cursorLine - 1, -1) : null;
    after = cursorLine < lines.length - 1 ? nearestBlock(lines, doc, cursorLine + 1, 1) : null;
  }

  return { before, current, after, outline };
}

function findLine(
  lines: LineInfo[],
  from: number,
  to: number,
  kind: 'nonblank',
): number | null {
  const step = from <= to ? 1 : -1;
  for (let i = from; step > 0 ? i <= to : i >= to; i += step) {
    if (kind === 'nonblank' && !lines[i].blank) {
      return i;
    }
  }
  return null;
}

function lineAtOffset(lines: LineInfo[], offset: number): number {
  for (let i = 0; i < lines.length; i += 1) {
    if (offset <= lines[i].end) {
      return i;
    }
  }
  return lines.length - 1;
}

function nearestBlock(lines: LineInfo[], doc: string, startLine: number, direction: -1 | 1): ParagraphSpan | null {
  let i = startLine;
  while (i >= 0 && i < lines.length && lines[i].blank) {
    i += direction;
  }
  if (i < 0 || i >= lines.length) {
    return null;
  }
  const [first, last] = blockRangeAt(lines, i);
  return spanOf(lines, doc, first, last);
}

function extractOutline(lines: LineInfo[], doc: string): OutlineEntry[] {
  const outline: OutlineEntry[] = [];
  const stack: { level: number; text: string }[] = [];
  let inFence = false;
  let fenceMarker = '';
  for (const line of lines) {
    const raw = doc.slice(line.start, line.end);
    const fence = inFence ? null : matchFence(raw);
    if (fence) {
      inFence = true;
      fenceMarker = fence;
      continue;
    }
    if (inFence) {
      if (fenceMarker !== '' && matchFence(raw) === fenceMarker) {
        inFence = false;
      }
      continue;
    }
    const heading = ATX_HEADING.exec(raw);
    if (heading) {
      const level = heading[1].length;
      while (stack.length > 0 && stack[stack.length - 1].level >= level) {
        stack.pop();
      }
      const text = (heading[2] ?? '').trim();
      outline.push({
        level,
        text,
        path: stack.map((entry) => entry.text),
        start: line.start,
        end: line.end,
      });
      stack.push({ level, text });
    }
  }
  return outline;
}

/** 匹配代码栅栏（``` 或 ~~~，行首最多 3 个空格）；返回栅栏标记（≥3 个同字符），不匹配返回 null。 */
function matchFence(raw: string): string | null {
  const m = /^ {0,3}(`{3,}|~{3,})/.exec(raw);
  return m ? m[1] : null;
}
