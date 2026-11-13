/**
 * Preference and escrow keys used before 0.10.0, under the product's former name, and the key
 * that replaces each one. Imported first by each window entry point, before any module reads
 * `localStorage`.
 */
export const FORMER_STORAGE_KEYS: ReadonlyArray<readonly [former: string, current: string]> = [
  ['devnotes-workspace-theme', 'humcase-workspace-theme'],
  ['devnotes-spaces-v1', 'humcase-spaces-v1'],
  ['devnotes_ecritures_en_attente', 'humcase-pending-writes'],
  ['devnotes_quick_capture_state', 'humcase-quick-capture-state'],
];

/**
 * Copies each former key to its current name when the current one does not exist yet. The
 * former value is never removed: an escrowed draft is never lost, even if the copy fails
 * halfway (storage full, blocked storage).
 */
export function carryOverFormerStorage(storage: Storage | undefined = globalThis.localStorage): void {
  if (!storage) return;
  for (const [former, current] of FORMER_STORAGE_KEYS) {
    try {
      const value = storage.getItem(former);
      if (value !== null && storage.getItem(current) === null) storage.setItem(current, value);
    } catch {
      // Storage unavailable: the application starts with its defaults, as before.
    }
  }
}

carryOverFormerStorage();
