import { useTranslation } from 'react-i18next';
import { Tray } from '@phosphor-icons/react';
import { EmptyState } from '../ui/EmptyState';
import { NewCaseButton, PageHeader } from './PageHeader';

/** The Inbox of a space before its first case: the page says what it is for and offers to start. */
export function InboxPreview({ title, onCreate }: { title: string; onCreate: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="workspace-scroll">
      <main className="workspace-page workspace-home">
        <PageHeader eyebrow={t('workspace.app.freeCapture')} title={title} count={0} countLabel={t('workspace.notes.count', { count: 0 })}
          description={t('workspace.app.inboxIntro')} action={<NewCaseButton onClick={onCreate} />} />
        <EmptyState icon={<Tray />} title={t('workspace.app.inboxEmptyTitle')} description={t('workspace.app.inboxEmptyDesc')} />
      </main>
    </div>
  );
}
