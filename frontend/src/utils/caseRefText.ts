import type { TFunction } from 'i18next';
import type { CaseRef, Project } from '../types';

/** A project as the person names it: an Inbox by its space, never its technical name. */
export function projectLabel(project: Pick<Project, 'name' | 'is_inbox' | 'space'>, t: TFunction): string {
  if (!project.is_inbox) return project.name;
  return t('common.inboxOf', { space: t(project.space === 'private' ? 'common.space.private' : 'common.space.work') });
}

/** The project a case is in, as the person names it: an Inbox by its space, never its technical name. */
export function caseProjectName(ref: CaseRef, t: TFunction): string {
  if (!ref.inbox_space) return ref.project_name;
  return t('common.inboxOf', { space: t(ref.inbox_space === 'private' ? 'common.space.private' : 'common.space.work') });
}
