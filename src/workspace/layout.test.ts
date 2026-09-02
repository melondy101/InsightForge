import { describe, expect, it } from 'vitest';

import { CENTER_MIN_RATIO, SIDE_PANE_MIN_WIDTH, applyDividerDrag, clampPaneWidths } from './layout';

/**
 * 三栏布局约束（spec：正文宽度下限 35%）。
 * 纯函数锁定两条可观察行为：拖拽分隔条永远拖不破 35% 下限；
 * 窗口尺寸变化后三栏仍然满足约束（总和=总宽）。
 */
describe('applyDividerDrag（拖拽分隔条）', () => {
  const initial = { left: 300, center: 500, right: 200 };
  const total = 1000;

  it('中栏宽度不得低于总宽 35%：向右拖左分隔条在中栏触线时停住', () => {
    const next = applyDividerDrag(initial, total, 'left', 250);
    expect(next.center).toBe(total * CENTER_MIN_RATIO);
    expect(next).toEqual({ left: 450, center: 350, right: 200 });
  });

  it('右分隔条向左拖把中栏压到 35% 以下时停住（对称）', () => {
    const next = applyDividerDrag(initial, total, 'right', -400);
    expect(next.center).toBe(total * CENTER_MIN_RATIO);
    expect(next.right).toBe(total - 300 - total * CENTER_MIN_RATIO);
  });

  it('右分隔条向右拖把右栏压到下限时停住（中栏至多吃掉侧栏余量）', () => {
    const next = applyDividerDrag(initial, total, 'right', 400);
    expect(next.right).toBe(SIDE_PANE_MIN_WIDTH);
    expect(next.center).toBe(total - 300 - SIDE_PANE_MIN_WIDTH);
  });

  it('大幅过拖不会产生负值或越界', () => {
    const next = applyDividerDrag(initial, total, 'left', 600);
    expect(next.center).toBe(total * CENTER_MIN_RATIO);
    expect(next.left).toBe(total * (1 - CENTER_MIN_RATIO) - 200);
  });

  it('侧栏宽度下限为 SIDE_PANE_MIN_WIDTH（分隔条保持可抓取）', () => {
    const next = applyDividerDrag(initial, total, 'left', -500);
    expect(next.left).toBe(SIDE_PANE_MIN_WIDTH);
    expect(next.center).toBe(total - SIDE_PANE_MIN_WIDTH - 200);
  });

  it('拖拽前后三栏宽度之和恒等于总宽', () => {
    for (const delta of [-600, -137, 0, 89, 600]) {
      const next = applyDividerDrag(initial, total, 'left', delta);
      expect(next.left + next.center + next.right).toBe(total);
    }
  });
});

describe('clampPaneWidths（窗口尺寸变化后的再约束）', () => {
  it('中栏被压到 35% 以下时抬回下限，较大的侧栏让出宽度', () => {
    const next = clampPaneWidths({ left: 550, center: 250, right: 200 }, 1000);
    expect(next).toEqual({ left: 450, center: 350, right: 200 });
  });

  it('较小的侧栏优先保持，让位顺序从大侧栏开始', () => {
    const next = clampPaneWidths({ left: 300, center: 250, right: 450 }, 1000);
    expect(next).toEqual({ left: 300, center: 350, right: 350 });
  });

  it('窗口变宽时侧栏保持像素宽，中栏吸收余量', () => {
    const next = clampPaneWidths({ left: 300, center: 500, right: 200 }, 1200);
    expect(next).toEqual({ left: 300, center: 700, right: 200 });
  });

  it('窗口为 0 时不产生 NaN', () => {
    expect(clampPaneWidths({ left: 10, center: 20, right: 30 }, 0)).toEqual({ left: 0, center: 0, right: 0 });
  });

  it('无论输入如何，输出总和等于总宽且中栏不低于 35%', () => {
    const cases: { widths: { left: number; center: number; right: number }; total: number }[] = [
      { widths: { left: -50, center: 800, right: 900 }, total: 1000 },
      { widths: { left: 1, center: 1, right: 1 }, total: 100 },
      { widths: { left: 480, center: 10, right: 10 }, total: 500 },
    ];
    for (const { widths, total } of cases) {
      const next = clampPaneWidths(widths, total);
      expect(next.left + next.center + next.right).toBe(total);
      expect(next.center).toBeGreaterThanOrEqual(total * CENTER_MIN_RATIO - 1e-9);
      expect(next.left).toBeGreaterThanOrEqual(0);
      expect(next.right).toBeGreaterThanOrEqual(0);
    }
  });
});
