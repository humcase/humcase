import i18n from '../i18n';
import type { NoteStatus } from '../types';

/**
 * Status labels: **single source** of translation for the interface.
 *
 * The status remains stored in English (`open`, `in_progress`, `resolved`): it is an
 * internal value (database, MCP, JSON). The interface must never display it raw, otherwise
 * we got "in progress" next to "Résolu". Before, each surface translated in its own way
 * (`replace('_', ' ')` in the list, English titles in the Kanban).
 */
export const STATUS_LABELS: Record<NoteStatus, string> = {
  // Getters: the language can change after import, so the label is read on every access.
  get open() { return i18n.t('notes.status.open'); },
  get in_progress() { return i18n.t('notes.status.in_progress'); },
  get resolved() { return i18n.t('notes.status.resolved'); },
};

/** Display order of statuses (list filters, Kanban columns). */
export const ORDERED_STATUSES: NoteStatus[] = ['open', 'in_progress', 'resolved'];

/** Label of a status in the current language; returns the raw value if unknown (defensive). */
export function statusLabel(status: string): string {
  return STATUS_LABELS[status as NoteStatus] ?? status;
}

/** Whether a stored filter value is a status (the filter is empty when none is chosen). */
export function isNoteStatus(value: string): value is NoteStatus {
  return (ORDERED_STATUSES as string[]).includes(value);
}
