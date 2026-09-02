/**
 * 与 Rust `scan::DEFAULT_IGNORE_GLOBS` 对应的默认忽略规则（仅用于添加表单展示）。
 * 若 Rust 侧调整默认规则，请同步此处。
 */
export const DEFAULT_IGNORE_GLOBS: readonly string[] = [
  '.git',
  'node_modules',
  'target',
  'dist',
  'build',
  'out',
  '.obsidian',
  '.DS_Store',
  'Thumbs.db',
  'desktop.ini',
  '~$*',
  '*.tmp',
  '*.swp',
  '*.bak',
];
