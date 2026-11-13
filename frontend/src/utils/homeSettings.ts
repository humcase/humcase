/** Sent when a setting the Home cards read changes (work profile, nudges, To tidy). */
export const HOME_SETTINGS_CHANGED_EVENT = 'humcase:home-settings-changed';

export function notifyHomeSettingsChanged() {
  window.dispatchEvent(new CustomEvent(HOME_SETTINGS_CHANGED_EVENT));
}

/** Calls `reload` whenever those settings change; returns the cleanup. */
export function onHomeSettingsChanged(reload: () => void): () => void {
  window.addEventListener(HOME_SETTINGS_CHANGED_EVENT, reload);
  return () => window.removeEventListener(HOME_SETTINGS_CHANGED_EVENT, reload);
}
