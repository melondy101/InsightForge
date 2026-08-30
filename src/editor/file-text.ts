/**
 * 文件文本归一化（纯函数）——Markdown round-trip 的地基。
 *
 * CodeMirror 6 文档内部统一使用 LF；文件层的 BOM 与 CRLF/CR/LF 行尾差异
 * 由本模块在「打开」时记录、在「序列化」时还原，保证编辑器不悄悄改写用户文件
 * （CONTEXT.md 铁律 4：无损 Markdown）。
 *
 * 行尾风格取文件中第一个行分隔符（与常见编辑器的自动检测一致）；
 * 混合行尾的文件在未修改时按原样逐字节往返，一旦有编辑则统一为检测到的风格。
 */

/** 行尾风格：CRLF（Windows）、LF（Unix）、CR（经典 Mac）；null = 文件中无换行。 */
export type LineEnding = 'crlf' | 'lf' | 'cr';

export interface FileTextTraits {
  /** 文件是否以 BOM（U+FEFF）开头。 */
  bom: boolean;
  /** 检测到的行尾风格。 */
  eol: LineEnding | null;
}

const BOM = '\uFEFF';

/**
 * 文件文本 → 编辑器文本：剥掉 BOM，把 CRLF/CR 全部归一为 LF，
 * 同时记录可还原的特征。
 */
export function analyzeFileText(raw: string): { text: string; traits: FileTextTraits } {
  const bom = raw.startsWith(BOM);
  const body = bom ? raw.slice(BOM.length) : raw;
  const eol = detectLineEnding(body);
  const text = body.replace(/\r\n?/g, '\n');
  return { text, traits: { bom, eol } };
}

const LINE_ENDINGS: Record<LineEnding, string> = {
  crlf: '\r\n',
  lf: '\n',
  cr: '\r',
};

/** 编辑器文本 → 文件文本：按特征还原行尾与 BOM。 */
export function restoreFileText(text: string, traits: FileTextTraits): string {
  const eol = traits.eol ? LINE_ENDINGS[traits.eol] : '\n';
  const body = text.replace(/\n/g, eol);
  return traits.bom ? BOM + body : body;
}

function detectLineEnding(body: string): LineEnding | null {
  const crlf = body.indexOf('\r\n');
  const lf = body.indexOf('\n');
  const cr = /\r/.exec(body);
  if (crlf !== -1 && (lf === -1 || crlf + 1 === lf) && (cr === null || crlf <= cr.index)) {
    return 'crlf';
  }
  if (cr && (lf === -1 || cr.index < lf)) {
    return 'cr';
  }
  if (lf !== -1) {
    return 'lf';
  }
  return null;
}
