/**
 * Work to save before Humcase quits on its own (an update closes and restarts it). Closing the
 * window only hides it, so the editors already flush on `onCloseRequested`; an update quits the
 * process without that event, so they also register here and the update waits for them.
 */
type Saver = () => Promise<unknown> | void;

const savers = new Set<Saver>();

/** Registers a saver; returns the function that removes it. */
export function onBeforeExit(saver: Saver): () => void {
  savers.add(saver);
  return () => { savers.delete(saver); };
}

/**
 * Runs every saver and waits for them, at most `timeoutMs`. Never throws: a failed write stays
 * in the editor's own safeguard (the escrow of failed writes), which survives the restart.
 */
export async function saveBeforeExit(timeoutMs = 5000): Promise<void> {
  const all = Promise.allSettled([...savers].map(saver => {
    try { return Promise.resolve(saver()); } catch (e) { return Promise.reject(e); }
  }));
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<void>(resolve => { timer = setTimeout(resolve, timeoutMs); });
  await Promise.race([all, late]);
  clearTimeout(timer);
}
