/** True when both records hold the same values, field by field (`===`). */
export function sameRecord<T extends object>(a: T, b: T): boolean {
  if (a === b) return true;
  const keys = Object.keys(a) as (keyof T)[];
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every(key => a[key] === b[key]);
}

/**
 * A fresh read, keeping the objects of the previous one that did not change: a case read again with
 * the same values is the same object, and an unchanged list is the same array. The views key their
 * caches (previews, positions, derived lists) on these identities, so a reread that changed nothing
 * costs no render.
 */
export function reuseUnchanged<T extends { id: string }>(previous: readonly T[], next: T[]): T[] {
  if (previous.length === 0) return next;
  const byId = new Map(previous.map(item => [item.id, item]));
  let changed = previous.length !== next.length;
  const merged = next.map((item, index) => {
    const old = byId.get(item.id);
    if (old && sameRecord(old, item)) {
      if (previous[index] !== old) changed = true;
      return old;
    }
    changed = true;
    return item;
  });
  return changed ? merged : (previous as T[]);
}
