import i18n from '../i18n';
import type { ProjectSpace } from '../types';

/**
 * Optional suggestions: only activities created in SQLite organize projects.
 * Getters: the list follows the current language at the time it is read.
 */
export const ACTIVITY_STARTERS: Record<ProjectSpace, string[]> = {
  get work() {
    return [
      i18n.t('notes.starters.development'), i18n.t('notes.starters.marketing'), i18n.t('notes.starters.sales'),
      i18n.t('notes.starters.clients'), i18n.t('notes.starters.learning'),
    ];
  },
  get private() {
    return [
      i18n.t('notes.starters.travel'), i18n.t('notes.starters.outings'), i18n.t('notes.starters.journal'),
      i18n.t('notes.starters.home'), i18n.t('notes.starters.creation'), i18n.t('notes.starters.learning'),
    ];
  },
};
