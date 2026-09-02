/**
 * 三栏工作台布局约束（纯函数）。
 *
 * spec 草稿写作约定：中栏（正文编辑器）宽度无论怎么拖都不低于总宽 35%。
 * 拖拽语义（applyDividerDrag）：拖动的分隔条移动两侧中的「侧栏 + 中栏」一对，
 * 中栏触线时分隔条恰好停在边界（过拖量被原样吸收，不波及另一侧）。
 * 窗口 resize 语义（clampPaneWidths）：中栏抬回下限时从最大的侧栏开始让位；
 * 窗口变宽时侧栏保持像素宽、中栏吸收余量（写作区尽量大）。
 */

/** 中栏宽度下限（占总宽比例）。 */
export const CENTER_MIN_RATIO = 0.35;

/** 侧栏宽度下限（px）：保证分隔条始终可抓取。空间不足时让位于 35% 硬约束。 */
export const SIDE_PANE_MIN_WIDTH = 48;

export interface PaneWidths {
  left: number;
  center: number;
  right: number;
}

export type DividerSide = 'left' | 'right';

export function applyDividerDrag(widths: PaneWidths, total: number, side: DividerSide, deltaPx: number): PaneWidths {
  if (total <= 0) {
    return { left: 0, center: 0, right: 0 };
  }
  const centerMin = total * CENTER_MIN_RATIO;
  const sideMin = Math.min(SIDE_PANE_MIN_WIDTH, Math.floor((total - centerMin) / 2));

  const other = side === 'left' ? widths.right : widths.left;
  const candidateCenter = side === 'left' ? widths.center - deltaPx : widths.center + deltaPx;
  const centerMax = Math.max(centerMin, total - Math.max(other, 0) - sideMin);
  const center = clamp(candidateCenter, centerMin, centerMax);

  const sideBudget = total - center;
  const dragged = clamp(sideBudget - Math.max(other, 0), sideMin, sideBudget);
  return side === 'left'
    ? { left: dragged, center, right: Math.max(other, 0) }
    : { left: Math.max(other, 0), center, right: dragged };
}

export function clampPaneWidths(widths: PaneWidths, total: number): PaneWidths {
  if (total <= 0) {
    return { left: 0, center: 0, right: 0 };
  }
  const centerMin = total * CENTER_MIN_RATIO;
  const left = Math.max(widths.left, 0);
  const right = Math.max(widths.right, 0);

  let center = clamp(widths.center, centerMin, total);
  const sideBudget = total - center;

  let nextLeft = left;
  let nextRight = right;
  const sideSum = left + right;
  if (sideSum > sideBudget) {
    // 让位顺序：先削减较大的侧栏，再削减较小的。
    const largerIsLeft = left >= right;
    const larger = Math.max(left, right);
    const smaller = Math.min(left, right);
    const takeFromLarger = Math.min(larger - smaller, sideSum - sideBudget);
    const overflow = sideSum - sideBudget - takeFromLarger;
    if (largerIsLeft) {
      nextLeft = larger - takeFromLarger - overflow;
      nextRight = smaller;
    } else {
      nextLeft = smaller;
      nextRight = larger - takeFromLarger - overflow;
    }
  } else if (sideSum < sideBudget) {
    // 窗口变宽：侧栏保持像素宽，中栏吸收余量。
    center += sideBudget - sideSum;
  }
  return { left: nextLeft, center, right: nextRight };
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}
