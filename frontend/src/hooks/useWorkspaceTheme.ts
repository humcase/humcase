import { useSyncExternalStore } from 'react';

function readTheme(): 'light' | 'dark' {
  try { return localStorage.getItem('humcase-workspace-theme') === 'light' ? 'light' : 'dark'; }
  catch { return 'dark'; }
}

function subscribe(onChange: () => void) {
  window.addEventListener('storage', onChange);
  window.addEventListener('focus', onChange);
  return () => { window.removeEventListener('storage', onChange); window.removeEventListener('focus', onChange); };
}

/** The capture window follows the main window's preference, without writing a preference. */
export function useWorkspaceTheme() { return useSyncExternalStore(subscribe, readTheme, () => 'dark' as const); }
