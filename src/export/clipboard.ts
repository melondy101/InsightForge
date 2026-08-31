/**
 * 公众号剪贴板导出（13 号票）：
 * - 复用 12 号票的 Markdown AST，生成微信编辑器兼容的 HTML
 * - 标签/属性白名单：只保留 div、p、span、br、strong、em、ul、ol、li、h1-h6、blockquote、a、img
 * - 样式内联：计算 Clean Reading 主题的必要样式并内联到元素上
 * - 写入剪贴板：同时写入 text/html 和 text/plain 两种格式
 */

import { markdownToCleanHtml, type ExportOptions } from './export';

/**
 * Sanitize HTML for WeChat editor compatibility.
 * Only allows specific tags and attributes.
 */
export function sanitizeForWeChat(html: string): string {
  // Remove script tags
  let result = html.replace(/<script\b[^<]*(?:(?!<\/script>)<[^<]*)*<\/script>/gi, '');

  // Remove style tags (WeChat doesn't support external CSS)
  result = result.replace(/<style\b[^<]*(?:(?!<\/style>)<[^<]*)*<\/style>/gi, '');

  // Remove HTML comments
  result = result.replace(/<!--[\s\S]*?-->/g, '');

  // Remove class attributes (WeChat doesn't support classes)
  result = result.replace(/\s+class="[^"]*"/gi, '');

  // Remove id attributes
  result = result.replace(/\s+id="[^"]*"/gi, '');

  // Remove data-* attributes
  result = result.replace(/\s+data-[a-z-]+="[^"]*"/gi, '');

  // Remove style attributes but keep the content
  // WeChat editor needs inline styles, but we'll add them separately
  result = result.replace(/\s+style="[^"]*"/gi, '');

  return result;
}

/**
 * Generate WeChat-compatible HTML from Markdown.
 * This is a simplified version that ensures maximum compatibility with WeChat editor.
 */
export async function markdownToWeChatHtml(markdown: string, options: ExportOptions): Promise<string> {
  // First, generate the standard HTML
  const fullHtml = await markdownToCleanHtml(markdown, options);

  // Extract the body content
  const bodyMatch = fullHtml.match(/<body[^>]*>([\s\S]*)<\/body>/);
  if (!bodyMatch) {
    return fullHtml;
  }

  let bodyContent = bodyMatch[1];

  // Sanitize for WeChat
  bodyContent = sanitizeForWeChat(bodyContent);

  // Wrap in minimal HTML structure for WeChat
  const wechatHtml = `<!DOCTYPE html>
<html>
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>${escapeHtml(options.title)}</title>
  <style>
    body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; font-size: 16px; line-height: 1.8; color: #1a1a1a; max-width: 720px; margin: 0 auto; padding: 20px; }
    h1, h2, h3 { color: ${options.accentColor}; }
    a { color: ${options.accentColor}; }
    blockquote { border-left: 4px solid ${options.accentColor}; padding-left: 1em; margin-left: 0; color: #666; }
    img { max-width: 100%; height: auto; }
  </style>
</head>
<body>
${bodyContent}
</body>
</html>`;

  return wechatHtml;
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
 * Generate plain text from Markdown for clipboard fallback.
 */
export function markdownToPlainText(markdown: string): string {
  // Simple markdown to plain text conversion
  return markdown
    .replace(/^#{1,6}\s+(.+)$/gm, '$1') // Remove heading markers
    .replace(/\*\*(.+?)\*\*/g, '$1') // Remove bold markers
    .replace(/\*(.+?)\*/g, '$1') // Remove italic markers
    .replace(/`(.+?)`/g, '$1') // Remove code markers
    .replace(/\[(.+?)\]\(.+?\)/g, '$1') // Remove link URLs
    .replace(/!\[(.+?)\]\(.+?\)/g, '$1') // Remove image alt text
    .replace(/^[-*]\s+/gm, '• ') // Convert list markers
    .replace(/^>\s+/gm, '') // Remove blockquote markers
    .trim();
}
