import { CheckCircle, CircleDashed, Gavel, Question, Warning } from '@phosphor-icons/react';
import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { CaseRef, Note, ProjectSummary } from '../../types';
import { api } from '../../services/tauriApi';
import { formatShortDate } from '../../utils/datetime';
import { TYPE_ICONS } from '../../utils/notePresentation';
import { Button } from '../ui/Button';
import { Count } from '../ui/Badge';
import { EmptyState } from '../ui/EmptyState';
import './tidy.css';
import './projectSummary.css';

function SummaryList({ id, icon, title, cases, empty, note, onOpen, quote = false }: {
  id: string;
  icon: ReactNode;
  title: string;
  cases: CaseRef[];
  empty: string;
  note?: string;
  onOpen: (id: string) => void;
  /** Show each case's first line (decisions: what was decided). */
  quote?: boolean;
}) {
  const { t } = useTranslation();
  return (
    <section className="project-summary__card" aria-labelledby={id} data-wide={quote || undefined}>
      <h2 id={id} className="project-summary__title">{icon}{title}<Count value={cases.length} />{note && <span className="project-summary__note">{note}</span>}</h2>
      {cases.length === 0 ? <p className="project-summary__empty">{empty}</p> : <ul className="project-summary__list">{cases.map(value => {
        const Icon = TYPE_ICONS[value.nature];
        const label = value.title || t('common.untitled');
        return <li key={value.id}>
          <div className="project-summary__line">
            <span className="project-summary__date">{formatShortDate(value.date)}</span>
            <button type="button" className="tidy-link" onClick={() => onOpen(value.id)} title={t('workspace.tidy.open', { title: label })}>
              {!quote && Icon && <Icon size={15} aria-hidden="true" />}<span>{label}</span>
            </button>
          </div>
          {quote && value.excerpt && <p className="project-summary__quote">{value.excerpt}</p>}
        </li>;
      })}</ul>}
    </section>
  );
}

/**
 * A project's summary: the decisions in force, what is in progress, the questions still open,
 * what was resolved lately, and the cases that may go against a decision. Read from the cases
 * each time it opens; agents read the same (`get_project_summary`).
 */
export function ProjectSummaryView({ projectId, notes, onNote, onTidy }: {
  projectId: string;
  /** The cases of the space: a title opens its case; a change reads the summary again. */
  notes: Note[];
  onNote: (note: Note) => void;
  onTidy: () => void;
}) {
  const { t } = useTranslation();
  const [summary, setSummary] = useState<ProjectSummary | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.projectSummary?.(projectId))
      .then(next => { if (active) { setSummary(next ?? null); setFailed(false); } })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; };
  }, [projectId, notes]);
  const open = (id: string) => { const note = notes.find(candidate => candidate.id === id); if (note) onNote(note); };

  if (failed) return <div role="alert" className="ui-dialog__error">{t('workspace.summary.failed')}</div>;
  if (!summary || summary.project_id !== projectId) return <div className="workspace-loading" role="status">{t('workspace.summary.loading')}</div>;
  const nothing = summary.decisions.length + summary.in_progress.length + summary.open_questions.length
    + summary.recently_resolved.length + summary.to_check.length === 0;
  return (
    <div className="project-summary">
      <p className="project-summary__intro">{t('workspace.summary.intro')}</p>
      {nothing ? <EmptyState title={t('workspace.summary.emptyTitle')} description={t('workspace.summary.emptyDescription')} /> : <>
        {summary.to_check.length > 0 && <section className="project-summary__check" aria-label={t('workspace.summary.toCheckLabel')}>
          <Warning size={18} aria-hidden="true" className="project-summary__warning" />
          <span>{t('workspace.summary.toCheck', { count: summary.to_check.length })}</span>
          <Button type="button" variant="secondary" size="sm" onClick={onTidy}>{t('workspace.summary.review')}</Button>
        </section>}
        <SummaryList id="summary-decisions" icon={<Gavel size={17} aria-hidden="true" />} title={t('workspace.summary.decisions')} cases={summary.decisions}
          empty={t('workspace.summary.noDecisions')} onOpen={open} quote
          note={summary.replaced_count > 0 ? t('workspace.summary.replaced', { count: summary.replaced_count }) : undefined} />
        <div className="project-summary__grid">
          <SummaryList id="summary-progress" icon={<CircleDashed size={17} aria-hidden="true" />} title={t('workspace.summary.inProgress')} cases={summary.in_progress} empty={t('workspace.summary.noneInProgress')} onOpen={open} />
          <SummaryList id="summary-questions" icon={<Question size={17} aria-hidden="true" />} title={t('workspace.summary.questions')} cases={summary.open_questions} empty={t('workspace.summary.noQuestions')} onOpen={open} />
          <SummaryList id="summary-resolved" icon={<CheckCircle size={17} aria-hidden="true" />} title={t('workspace.summary.resolved')} cases={summary.recently_resolved} empty={t('workspace.summary.noneResolved')} onOpen={open} />
        </div>
      </>}
    </div>
  );
}
