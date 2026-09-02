/**
 * 索引进度的纯计算（AGENTS.md：纯函数优先）。
 */

/** 已处理/总数 → 百分比（0–100 的整数；总数无效时为 0）。 */
export function progressPercent(processed: number, total: number): number {
  if (!Number.isFinite(processed) || !Number.isFinite(total) || total <= 0 || processed <= 0) {
    return 0;
  }
  return Math.min(100, Math.round((processed / total) * 100));
}
