import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event';
import { PhysicalPosition, availableMonitors, currentMonitor } from '@tauri-apps/api/window';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import type { AssistantEntry, AssistantReply } from '../../types/assistant';
import type { ProjectSpace } from '../../types';
import type { CaseRef } from './AssistantWindow';
import { getAssistantMiniPlace, setAssistantMiniPlace } from '../../utils/prefs';

/**
 * Mini Humcase: the assistant in its own small window, on top of every application. The main
 * window and the mini window pass the conversation to each other with events; one of them shows
 * it at a time. A reply still on its way when the conversation moves is forwarded when it lands.
 */
export const MINI_LABEL = 'assistant_mini';

/** What the mini window needs from the main window: the conversation and the context. */
export type MiniScope = {
  openCase: CaseRef | null;
  space: ProjectSpace | 'unassigned' | undefined;
  scopeLabel: string;
  keepSpace: ProjectSpace;
};
export type MiniSeed = MiniScope & { entries: AssistantEntry[]; why: 'pinned' | 'behind' };
/** A reply (or its error) for an exchange that moved to the other window while it was asked. */
export type EntryLanded = { id: number; reply?: AssistantReply; error?: string };
/** What the mini window asks the main window to do. */
export type MiniRequest = { kind: 'open-case' | 'germinate'; id: string } | { kind: 'changed' } | { kind: 'connect-ai' };

const EVENTS = {
  seed: 'humcase:assistant-mini-seed',
  scope: 'humcase:assistant-mini-scope',
  back: 'humcase:assistant-mini-back',
  landed: 'humcase:assistant-entry-landed',
  request: 'humcase:assistant-mini-request',
  ready: 'humcase:assistant-mini-ready',
  recall: 'humcase:assistant-mini-recall',
} as const;

const on = <T,>(name: string, handler: (payload: T) => void): Promise<UnlistenFn> => listen<T>(name, event => handler(event.payload));

export const mini = {
  seed: (seed: MiniSeed) => emit(EVENTS.seed, seed),
  scope: (scope: MiniScope) => emit(EVENTS.scope, scope),
  /** The mini window gives the conversation back; `open`: the assistant stays open in the app. */
  back: (entries: AssistantEntry[], open: boolean) => emit(EVENTS.back, { entries, open }),
  landed: (landed: EntryLanded) => emit(EVENTS.landed, landed),
  request: (request: MiniRequest) => emit(EVENTS.request, request),
  ready: () => emit(EVENTS.ready),
  /** Humcase is back in front: what went away on its own comes back. */
  recall: () => emit(EVENTS.recall),
  onSeed: (handler: (seed: MiniSeed) => void) => on<MiniSeed>(EVENTS.seed, handler),
  onScope: (handler: (scope: MiniScope) => void) => on<MiniScope>(EVENTS.scope, handler),
  onBack: (handler: (back: { entries: AssistantEntry[]; open: boolean }) => void) => on(EVENTS.back, handler),
  onLanded: (handler: (landed: EntryLanded) => void) => on<EntryLanded>(EVENTS.landed, handler),
  onRequest: (handler: (request: MiniRequest) => void) => on<MiniRequest>(EVENTS.request, handler),
  onReady: (handler: () => void) => on(EVENTS.ready, handler),
  onRecall: (handler: () => void) => on(EVENTS.recall, handler),
};

const MARGIN = 24;

/** Shows the mini window at its remembered place (the bottom right of the screen the first time). */
export async function showMini(): Promise<void> {
  const window = await WebviewWindow.getByLabel(MINI_LABEL);
  if (!window) return;
  const place = getAssistantMiniPlace();
  const monitors = await availableMonitors().catch(() => []);
  const size = await window.outerSize();
  // A place on a screen that is no longer there falls back to the bottom right.
  const onScreen = place && monitors.some(m => place.x >= m.position.x && place.y >= m.position.y
    && place.x + 80 <= m.position.x + m.size.width && place.y + 40 <= m.position.y + m.size.height);
  if (place && onScreen) {
    await window.setPosition(new PhysicalPosition(place.x, place.y));
  } else {
    const monitor = await currentMonitor().catch(() => null) ?? monitors[0];
    if (monitor) {
      const margin = Math.round(MARGIN * monitor.scaleFactor);
      await window.setPosition(new PhysicalPosition(
        monitor.position.x + monitor.size.width - size.width - margin,
        monitor.position.y + monitor.size.height - size.height - margin * 3,
      ));
    }
  }
  await window.show();
}

export async function hideMini(): Promise<void> {
  const window = await WebviewWindow.getByLabel(MINI_LABEL);
  await window?.hide();
}

/** The mini window remembers where the person put it. */
export function rememberMiniPlace(x: number, y: number) { setAssistantMiniPlace({ x, y }); }
