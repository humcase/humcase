import { useCallback, useEffect, useRef, useState } from 'react';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { MINI_LABEL, mini, showMini, type MiniRequest, type MiniScope } from '../components/assistant/miniBridge';
import type { EntryLanded } from '../components/assistant/AssistantWindow';
import type { AssistantEntry } from '../types/assistant';
import { getAssistantStayOnTop, setAssistantStayOnTop } from '../utils/prefs';

// Focus moves between windows in two steps (blur, then focus elsewhere): the check waits for it.
const SETTLE_MS = 250;

/**
 * The main window's side of Mini Humcase: hands the conversation over when the person pins the
 * assistant, or when Humcase goes behind with "Stay on top" on; takes it back when the mini window
 * returns it, or when Humcase comes back to the front after going behind.
 */
export function useAssistantMini({ open, setOpen, scope, blocked, onRequest }: {
  open: boolean;
  setOpen: (open: boolean) => void;
  scope: MiniScope;
  /** The sticky-note mode: the assistant is not shown, it does not move either. */
  blocked: boolean;
  onRequest: (request: MiniRequest) => void;
}) {
  const [away, setAway] = useState<null | 'pinned' | 'behind'>(null);
  const [seed, setSeed] = useState<{ key: number; entries: AssistantEntry[] } | null>(null);
  const [landed, setLanded] = useState<(EntryLanded & { key: number }) | null>(null);
  const [stayOnTop, setStayOnTopState] = useState(getAssistantStayOnTop);
  const entries = useRef<AssistantEntry[]>([]);
  const key = useRef(0);
  const ready = useRef(false);
  const live = useRef({ open, away, stayOnTop, blocked, scope, onRequest });
  useEffect(() => { live.current = { open, away, stayOnTop, blocked, scope, onRequest }; });

  const sendAway = useCallback(async (why: 'pinned' | 'behind') => {
    if (!ready.current) return;
    setAway(why);
    await mini.seed({ ...live.current.scope, entries: entries.current, why });
    await showMini();
  }, []);

  useEffect(() => {
    const stops = [
      mini.onReady(() => { ready.current = true; }),
      mini.onBack(({ entries: back, open: stayOpen }) => {
        key.current += 1;
        setSeed({ key: key.current, entries: back });
        setAway(null);
        live.current.away = null;
        setOpen(stayOpen);
      }),
      mini.onLanded(next => { key.current += 1; setLanded({ ...next, key: key.current }); }),
      mini.onRequest(request => live.current.onRequest(request)),
    ];
    // The mini window may have loaded first: it says it is ready again when asked.
    void WebviewWindow.getByLabel(MINI_LABEL).then(window => { if (window) ready.current = true; });
    // The page's own focus says when Humcase goes behind or comes back (the window's focus
    // events go through the page too).
    let timer: ReturnType<typeof setTimeout> | null = null;
    const changed = (focused: boolean) => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(async () => {
        const now = live.current;
        if (focused) {
          // Back to the front: what went away on its own comes back.
          if (now.away === 'behind') void mini.recall();
          return;
        }
        if (!now.stayOnTop || !now.open || now.away || now.blocked || document.hasFocus()) return;
        // Focus went to another window of Humcase (quick capture): not behind.
        const capture = await WebviewWindow.getByLabel('quick_capture');
        if (await capture?.isFocused()) return;
        void sendAway('behind');
      }, SETTLE_MS);
    };
    const onBlur = () => changed(false);
    const onFocus = () => changed(true);
    window.addEventListener('blur', onBlur);
    window.addEventListener('focus', onFocus);
    return () => {
      if (timer) clearTimeout(timer);
      window.removeEventListener('blur', onBlur);
      window.removeEventListener('focus', onFocus);
      for (const stop of stops) void stop.then(unlisten => unlisten());
    };
  }, [sendAway, setOpen]);

  // Away, the mini window follows the open case and the space.
  useEffect(() => { if (away) void mini.scope(scope); }, [away, scope]);

  const changeStayOnTop = (on: boolean) => { setStayOnTopState(on); setAssistantStayOnTop(on); };

  return {
    away,
    seed,
    landed,
    stayOnTop,
    changeStayOnTop,
    pin: () => void sendAway('pinned'),
    onSnapshot: (next: AssistantEntry[]) => { entries.current = next; },
    /** A reply landed in the main window while the conversation is away: the mini window shows it. */
    onLanded: (next: EntryLanded) => { if (live.current.away) void mini.landed(next); },
  };
}
