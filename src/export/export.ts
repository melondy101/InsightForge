/**
 * 通用 HTML 导出（12 号票）：
 * - 输入 Markdown，输出带 Clean Reading 主题的完整 HTML 文件
 * - remark-parse → remark-gfm → remark-rehype → rehype-stringify
 * - rehype-sanitize 消毒：禁用内嵌 HTML，仅允许 https 图片
 */

import { unified } from 'unified';
import remarkParse from 'remark-parse';
import remarkGfm from 'remark-gfm';
import remarkRehype from 'remark-rehype';
import rehypeSanitize from 'rehype-sanitize';
import rehypeStringify from 'rehype-stringify';

export interface ExportOptions {
  readonly title: string;
  readonly accentColor: string;
}

const CLEAN_READING_THEME = `
/* Clean Reading Theme */
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  font-size: 16px;
  line-height: 1.8;
  color: #1a1a1a;
  max-width: 720px;
  margin: 0 auto;
  padding: 40px 20px;
}

h1, h2, h3, h4, h5, h6 {
  line-height: 1.3;
  margin-top: 1.5em;
  margin-bottom: 0.5em;
  font-weight: 600;
}

h1 { font-size: 2em; border-bottom: 2px solid #e0e0e0; padding-bottom: 0.3em; }
h2 { font-size: 1.5em; border-bottom: 1px solid #e0e0e0; padding-bottom: 0.3em; }
h3 { font-size: 1.25em; }
h4 { font-size: 1.1em; }

p { margin: 0 0 1em 0; }

a { color: #0550ae; text-decoration: none; }
a:hover { text-decoration: underline; }

blockquote {
  margin: 1em 0;
  padding: 0.5em 1em;
  border-left: 4px solid #d0d0d0;
  background: #f9f9f9;
  color: #444;
}

code {
  font-family: "SF Mono", Monaco, Inconsolata, "Fira Code", monospace;
  font-size: 0.9em;
  background: #f4f4f4;
  padding: 0.2em 0.4em;
  border-radius: 3px;
}

pre {
  background: #f4f4f4;
  padding: 1em;
  overflow-x: auto;
  border-radius: 4px;
}

pre code {
  background: none;
  padding: 0;
}

img {
  max-width: 100%;
  height: auto;
  display: block;
  margin: 1em auto;
}

table {
  width: 100%;
  border-collapse: collapse;
  margin: 1em 0;
  font-size: 0.95em;
}

th, td {
  border: 1px solid #d0d0d0;
  padding: 8px 12px;
  text-align: left;
}

th {
  background: #f4f4f4;
  font-weight: 600;
}

tr:nth-child(even) {
  background: #fafafa;
}

ul, ol {
  margin: 0 0 1em 1.5em;
  padding-left: 1em;
}

li { margin-bottom: 0.3em; }

hr {
  border: none;
  border-top: 1px solid #e0e0e0;
  margin: 2em 0;
}
`;

function buildHtml(title: string, body: string, accentColor: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(title)}</title>
  <style>
    :root {
      --accent: ${accentColor};
    }
    ${CLEAN_READING_THEME}
    h1, h2, h3 { color: var(--accent); }
    a { color: var(--accent); }
    blockquote { border-left-color: var(--accent); }
  </style>
</head>
<body>
  ${body}
</body>
</html>`;
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

/**
 * Validate that an image URL is publicly accessible.
 * Only allows https URLs. Performs a HEAD request to verify accessibility.
 */
export async function validateImageUrl(url: string): Promise<boolean> {
  if (!url.startsWith('https://')) {
    return false;
  }

  try {
    await fetch(url, { method: 'HEAD', mode: 'no-cors' });
    // no-cors returns opaque response; we consider it accessible if no network error
    return true;
  } catch {
    return false;
  }
}

/**
 * Convert Markdown to Clean Reading HTML.
 * Throws if any image URL is inaccessible.
 */
export async function markdownToCleanHtml(markdown: string, options: ExportOptions): Promise<string> {
  // First pass: extract image URLs and validate them
  const imageUrlRegex = /!\[.*?\]\((.*?)\)/g;
  const imageUrls: string[] = [];
  let match;

  while ((match = imageUrlRegex.exec(markdown)) !== null) {
    imageUrls.push(match[1]);
  }

  // Validate all image URLs
  const invalidImages: string[] = [];
  for (const url of imageUrls) {
    if (!url.startsWith('https://')) {
      invalidImages.push(url);
    } else {
      const ok = await validateImageUrl(url);
      if (!ok) {
        invalidImages.push(url);
      }
    }
  }

  if (invalidImages.length > 0) {
    throw new Error(`Inaccessible images blocked export: ${invalidImages.join(', ')}`);
  }

  // Second pass: convert markdown to HTML
  const processor = unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkRehype, { allowDangerousHtml: false })
    .use(rehypeSanitize)
    .use(rehypeStringify);

  const result = await processor.process(markdown);

  const body = String(result.value);
  return buildHtml(options.title, body, options.accentColor);
}

/**
 * Generate a filename for the exported HTML.
 */
export function generateExportFilename(title: string): string {
  const slug = title
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-')
    .slice(0, 50);
  return `${slug || 'export'}.html`;
}
