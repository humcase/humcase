import { beforeEach, describe, expect, it } from 'vitest';
import { carryOverFormerStorage, FORMER_STORAGE_KEYS } from './legacyStorage';

describe('carryOverFormerStorage', () => {
  beforeEach(() => localStorage.clear());

  it('copies every former key to its current name and keeps the former value', () => {
    for (const [former] of FORMER_STORAGE_KEYS) localStorage.setItem(former, `value of ${former}`);
    carryOverFormerStorage();
    for (const [former, current] of FORMER_STORAGE_KEYS) {
      expect(localStorage.getItem(current)).toBe(`value of ${former}`);
      expect(localStorage.getItem(former)).toBe(`value of ${former}`);
    }
  });

  it('never overwrites a current value', () => {
    localStorage.setItem('devnotes_ecritures_en_attente', '[{"old":true}]');
    localStorage.setItem('humcase-pending-writes', '[{"new":true}]');
    carryOverFormerStorage();
    expect(localStorage.getItem('humcase-pending-writes')).toBe('[{"new":true}]');
  });

  it('leaves absent keys absent', () => {
    carryOverFormerStorage();
    for (const [, current] of FORMER_STORAGE_KEYS) expect(localStorage.getItem(current)).toBeNull();
  });
});
