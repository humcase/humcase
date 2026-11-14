/**
 * Work to save before Humcase quits on its own (an update closes and restarts it). Closing the
 * window only hides it, so the editors already save on `onCloseRequested`; an update quits the
 * process without that event and without unmounting, so they also register here and the update
 * installs only once they all answered.
 */
type Saver = () => Promise<unknown> | void;

const savers = new Set<Saver>();

/** Registers a saver; returns the function that removes it. */
export function onBeforeExit(saver: Saver): () => void {
  savers.add(saver);
  return () => { savers.delete(saver); };
}

/**
 * Runs every saver and waits for them, at most `timeoutMs` (above SQLite's 5 s wait for a lock
 * held by an agent). True when every saver finished; false when one failed or is still running,
 * and then the caller must not quit: the write may still be under way.
 */
export async function saveBeforeExit(timeoutMs = 15000): Promise<boolean> {
  const all = Promise.allSettled([...savers].map(saver => {
    try { return Promise.resolve(saver()); } catch (e) { return Promise.reject(e); }
  })).then(results => results.every(result => result.status === 'fulfilled'));
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<boolean>(resolve => { timer = setTimeout(() => resolve(false), timeoutMs); });
  const saved = await Promise.race([all, late]);
  clearTimeout(timer);
  return saved;
}
