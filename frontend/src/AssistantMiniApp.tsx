import '@fontsource/dm-mono/400.css';
import '@fontsource/dm-mono/500.css';
import './workspace-theme.css';
import './components/ui/kit.css';
import './foundations.css';
import './game-object.css';
import { useEffect, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { AssistantWindow, type EntryLanded } from './components/assistant/AssistantWindow';
import { hideMini, mini, rememberMiniPlace, type MiniScope } from './components/assistant/miniBridge';
import { useWorkspaceTheme } from './hooks/useWorkspaceTheme';
import type { AssistantEntry } from './types/assistant';

/**
 * Mini Humcase: the assistant's window when it is pinned or when Humcase went behind. It holds the
 * conversation the main window handed over, and gives it back when it returns into the app.
 */
export default function AssistantMiniApp() {
  const theme = useWorkspaceTheme();
  const [scope, setScope] = useState<MiniScope | null>(null);
  const [seed, setSeed] = useState<{ key: number; entries: AssistantEntry[] } | null>(null);
  const [landed, setLanded] = useState<(EntryLanded & { key: number }) | null>(null);
  const [shown, setShown] = useState(false);
  const entries = useRef<AssistantEntry[]>([]);
  const key = useRef(0);
  // Given back once: a late recall must not hand over an old conversation again.
  const shownRef = useRef(false);

  // Back into the app: the conversation goes with it.
  const giveBack = (open: boolean) => {
    if (!shownRef.current) return;
    shownRef.current = false;
    void mini.back(entries.current, open);
    setShown(false);
    void hideMini();
  };
  const giveBackRef = useRef(giveBack);
  useEffect(() => { giveBackRef.current = giveBack; });

  useEffect(() => {
    const stops = [
      mini.onSeed(next => {
        key.current += 1;
        setScope(next);
        setSeed({ key: key.current, entries: next.entries });
        shownRef.current = true;
        setShown(true);
      }),
      mini.onScope(setScope),
      mini.onRecall(() => giveBackRef.current(true)),
      mini.onLanded(next => { key.current += 1; setLanded({ ...next, key: key.current }); }),
    ];
    void mini.ready();
    // Where the person puts it, it comes back.
    const window = getCurrentWindow();
    const moved = window.onMoved(({ payload }) => rememberMiniPlace(payload.x, payload.y));
    return () => { for (const stop of [...stops, moved]) void stop.then(unlisten => unlisten()); };
  }, []);

  return (
    <div className="workspace-app assistant-mini" data-theme={theme} style={{ height: '100dvh' }}>
      {scope && (
        <AssistantWindow
          variant="mini"
          open={shown}
          onOpenChange={open => { if (!open) giveBack(false); }}
          space={scope.space}
          scopeLabel={scope.scopeLabel}
          openCase={scope.openCase}
          keepSpace={scope.keepSpace}
          seed={seed}
          landed={landed}
          onSnapshot={next => { entries.current = next; }}
          onLanded={next => void mini.landed(next)}
          onPin={() => giveBack(true)}
          onOpenCase={id => void mini.request({ kind: 'open-case', id })}
          onChanged={() => void mini.request({ kind: 'changed' })}
          onGerminate={id => void mini.request({ kind: 'germinate', id })}
          onConnectAi={() => void mini.request({ kind: 'connect-ai' })}
        />
      )}
    </div>
  );
}
