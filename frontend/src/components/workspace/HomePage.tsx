import { useTranslation } from 'react-i18next';
import { Button } from '../ui/Button';
import { WorkspaceHome, type WorkspaceHomeProps } from './WorkspaceHome';

/**
 * The Home page: a wait while the space loads, then the projects to sort into a space, the home
 * itself and, until the person has some, the invitation to create activities.
 */
export function HomePage({ ready, unassignedCount, onViewUnassigned, inviteActivities, onCustomize, ...home }: Omit<WorkspaceHomeProps, 'children'> & {
  ready: boolean;
  /** Projects without a space, announced when the page does not already show them (0: none). */
  unassignedCount: number;
  onViewUnassigned: () => void;
  inviteActivities: boolean;
  onCustomize: () => void;
}) {
  const { t } = useTranslation();
  return (
    <div className="workspace-scroll">
      {ready ? <>
        {unassignedCount > 0 && (
          <div className="workspace-unassigned-banner">
            <span>{t('workspace.app.unassignedBanner', { count: unassignedCount })}</span>
            <Button variant="secondary" size="sm" onClick={onViewUnassigned}>{t('workspace.app.viewUnassigned')}</Button>
          </div>
        )}
        <WorkspaceHome {...home}>
          {inviteActivities && (
            <div className="activity-invitation">
              <div><h2>{t('workspace.app.areasInviteTitle')}</h2><p>{t('workspace.app.areasInviteDesc')}</p></div>
              <Button type="button" variant="secondary" onClick={onCustomize}>{t('workspace.app.customize')}</Button>
            </div>
          )}
        </WorkspaceHome>
      </> : <div className="workspace-loading" role="status">{t('workspace.app.loadingSpace')}</div>}
    </div>
  );
}
