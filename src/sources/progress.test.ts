import { describe, expect, it } from 'vitest';

import { progressPercent } from './progress';

describe('progressPercent', () => {
  it('按已处理/总数折算为整数百分比', () => {
    expect(progressPercent(0, 10)).toBe(0);
    expect(progressPercent(1, 4)).toBe(25);
    expect(progressPercent(2, 3)).toBe(67);
    expect(progressPercent(10, 10)).toBe(100);
  });

  it('总数非正或输入非有限数时返回 0，绝不返回 NaN', () => {
    expect(progressPercent(5, 0)).toBe(0);
    expect(progressPercent(5, -1)).toBe(0);
    expect(progressPercent(Number.NaN, 10)).toBe(0);
    expect(progressPercent(1, Number.POSITIVE_INFINITY)).toBe(0);
  });

  it('已处理超过总数时封顶 100', () => {
    expect(progressPercent(12, 10)).toBe(100);
  });
});
