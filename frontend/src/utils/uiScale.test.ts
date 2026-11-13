import { describe, expect, it } from 'vitest';
import { uiZoomFor } from './uiScale';

describe('Interface scale', () => {
  it('scales up on a 2560 px screen displayed at 100 %', () => expect(uiZoomFor(2560, 1)).toBe(1.2));
  it('does not touch a 1920 screen nor a screen already scaled by Windows', () => {
    expect(uiZoomFor(1920, 1)).toBe(1);
    expect(uiZoomFor(2560, 1.25)).toBe(1);
  });
  it('caps on very large screens', () => expect(uiZoomFor(3840, 1)).toBe(1.5));
});
