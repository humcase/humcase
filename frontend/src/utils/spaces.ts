import i18n from '../i18n';
import type { Project } from '../types';

export type ProjectSpace = 'private' | 'work';
export type SpaceScope = ProjectSpace | 'all' | 'unassigned';
export type SpaceUsage = ProjectSpace | 'both';

const PREF_KEY = 'humcase-spaces-v1';

export interface SpacePreferences {
  usage: SpaceUsage;
  combined: boolean;
  scope: SpaceScope;
}

export function availableScopes(preferences: SpacePreferences): SpaceScope[] {
  if (preferences.usage === 'both') return preferences.combined ? ['private', 'work', 'all', 'unassigned'] : ['private', 'work', 'unassigned'];
  return [preferences.usage, 'unassigned'];
}

export function normalizeSpacePreferences(value: Partial<SpacePreferences>): SpacePreferences {
  const usage: SpaceUsage = value.usage === 'private' || value.usage === 'work' ? value.usage : 'both';
  const combined = value.combined === true && usage === 'both';
  const proposed = value.scope;
  const fallback: SpaceScope = usage === 'both' ? 'work' : usage;
  const scope = proposed && availableScopes({ usage, combined, scope: fallback }).includes(proposed) ? proposed : fallback;
  return { usage, combined, scope };
}

export function loadSpacePreferences(): SpacePreferences {
  try {
    const stored = localStorage.getItem(PREF_KEY);
    return normalizeSpacePreferences(stored ? JSON.parse(stored) as Partial<SpacePreferences> : {});
  } catch { return normalizeSpacePreferences({}); }
}

export function saveSpacePreferences(preferences: SpacePreferences): void {
  try { localStorage.setItem(PREF_KEY, JSON.stringify(preferences)); } catch { /* session-only */ }
}

export function projectInScope(project: Project, scope: SpaceScope): boolean {
  return scope === 'all' ? project.space === 'private' || project.space === 'work' : (scope === 'unassigned' ? project.space == null : project.space === scope);
}

export function spaceLabel(scope: SpaceScope): string {
  return i18n.t(`notes.space.${scope}`);
}
