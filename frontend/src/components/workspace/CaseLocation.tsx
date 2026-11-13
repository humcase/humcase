import type { CSSProperties } from 'react';
import { useTranslation } from 'react-i18next';
import { projectLabel } from '../../utils/caseRefText';
import type { Note, Project } from '../../types';

/**
 * Where a case is: its project (with its color), then its parent cases from the top, so a case
 * shown outside its place (brief, shortcuts) still says where it lives.
 */
export function CaseLocation({ note, byId, projectById }: {
  note: Note;
  /** Visible cases by id, to climb the parents (a parent missing from it ends the path). */
  byId: ReadonlyMap<string, Note>;
  projectById: ReadonlyMap<string, Project>;
}) {
  const { t } = useTranslation();
  const project = projectById.get(note.project_id);
  const path: string[] = [];
  const seen = new Set<string>();
  for (let parent = note.parent_id ? byId.get(note.parent_id) : undefined; parent && !seen.has(parent.id); parent = parent.parent_id ? byId.get(parent.parent_id) : undefined) {
    seen.add(parent.id);
    path.unshift(parent.title || t('common.untitled'));
  }
  return <span className="case-location">
    <span className="case-location__dot" style={{ '--case-location-dot': project?.color || 'var(--workspace-text-muted)' } as CSSProperties} aria-hidden="true" />
    {[project ? projectLabel(project, t) : t('workspace.home.noteFallback'), ...path].join(' › ')}
  </span>;
}
