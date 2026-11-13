import { SegmentedControl } from '../ui/SegmentedControl';
import { Button } from '../ui/Button';
import { NewCaseButton, PageHeader } from './PageHeader';
import { ArrowRight, Folder, Plus, Note as StickyNote, SquaresFour, CircleDashed } from '@phosphor-icons/react';
import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { Activity, Brief, CaseEvent, Note, NoteRevision, Project, ProjectSpace } from '../../types';
import { HomeBrief } from './HomeBrief';
import { CaseLocation } from './CaseLocation';
import { api } from '../../services/tauriApi';
import { DayTimeline } from './DayTimeline';
import { casesToFollow, dueState } from '../../utils/dueDates';
import { NoteCard } from './WorkspaceCards';
import { notePreview } from '../../utils/notePresentation';
import { formatRelative, formatShortDate, parseTimestamp } from '../../utils/datetime';
import { Count, DueBadge, PriorityBadge, StatusBadge } from '../ui/Badge';
import { Card } from '../ui/Card';
import { List, ListRow } from '../ui/List';
import { EmptyState } from '../ui/EmptyState';
import { useToday } from '../../hooks/useToday';
import { NatureReviewCard, NatureReviewDialog } from './NatureReview';
import { useNatureProposals } from '../../hooks/useNatureProposals';
import { TidyCard } from './Tidy';
import { useTidy } from '../../hooks/useTidy';
import { NudgesCard, ProfileCard } from './Profile';
import { useNudges, useProfileProposalCount, useWorkProfile } from '../../hooks/useProfile';
import { projectLabel } from '../../utils/caseRefText';
import { currentDateLocale } from '../../i18n';
import { WelcomeGuide } from './WelcomeGuide';
import { useAgentsSeen } from '../../hooks/useAgentsSeen';
import { getExampleProjectId, getWelcomeHidden, setWelcomeHidden } from '../../utils/prefs';

export interface WorkspaceHomeProps {
  projects: Project[];
  /** Every project of the space, Inboxes included, to say where a case is (`projects` by default). */
  locateProjects?: Project[];
  notes: Note[];
  onNote: (note: Note) => void;
  onCreateProject: () => void;
  onCreateNote: () => void;
  activities?: Activity[];
  activeActivityId?: string | null;
  onActivity?: (id: string | null) => void;
  children?: ReactNode;
  /** Number of live sub-cases per case identifier. */
  subCaseCounts?: ReadonlyMap<string, number>;
  /** Every visible case, sub-cases included, for the day, "Since your last visit" and "To follow up" (defaults to `notes`). */
  allNotes?: Note[];
  /** Shows every case to follow up (the Today view). */
  onSeeAllFollowup?: () => void;
  /** Cases changed from the home (natures to check): they are read again. */
  onNotesChanged?: () => void;
  /** Opens "To tidy" (its card shows when something is proposed). */
  onTidy?: () => void;
  /** Space whose work profile opens from the Home card. */
  profileSpace?: ProjectSpace;
  /** Spaces whose proposed lines the Home card counts (both in the combined view). */
  profileSpaces?: ProjectSpace[];
  /** Opens the work profile page. */
  onProfile?: () => void;
  /** A nudge's "Sprout": opens the case and proposes its steps. */
  onGerminate?: (id: string) => void;
  /** Opens Settings → Agents (first steps). Without it, the first steps are not offered. */
  onConnectAgent?: () => void;
  /** Projects and cases changed from the home (the first steps' example): they are read again. */
  onDataChanged?: () => void;
}

const byModification = (a: Note, b: Note) => b.updated_at.localeCompare(a.updated_at);

/**
 * Agent writes of the day for the timeline: every write of the v24 journal, plus the attributed
 * edits recorded before it (an edit recorded by both, within two seconds, counts once).
 */
function agentWrites(revisions: NoteRevision[], events: CaseEvent[]): NoteRevision[] {
  const time = (value: string) => parseTimestamp(value)?.getTime() ?? 0;
  const fromJournal: NoteRevision[] = events.map(event => ({ id: event.id, note_id: event.note_id, revision: 0, source: 'agent', fields: '{}', created_at: event.last_at }));
  const earlier = revisions.filter(revision => !events.some(event => event.note_id === revision.note_id
    && time(revision.created_at) >= time(event.created_at) - 2000 && time(revision.created_at) <= time(event.last_at) + 2000));
  return [...earlier, ...fromJournal];
}

/**
 * Resume home, from the most alive to the calmest: the day, what agents did since the last visit,
 * the case to pick up and what to follow, then the recent cases. A case is shown once, in the
 * highest place it appears. Projects are in the sidebar, reports in their case.
 * All labels stay factual: `updated_at` dates the note (not a session nor the report),
 * and a status or a report is a declaration by its author, not a validation.
 */
export function WorkspaceHome({ projects, locateProjects = projects, notes, onNote, onCreateProject, onCreateNote, activities = [], activeActivityId = null, onActivity, children, subCaseCounts, allNotes = notes, onSeeAllFollowup, onNotesChanged, onTidy, profileSpace = 'work', profileSpaces, onProfile, onGerminate, onConnectAgent, onDataChanged }: WorkspaceHomeProps) {
  const { t } = useTranslation();
  const today = useToday();
  const projectById = new Map(locateProjects.map(project => [project.id, project]));
  const projectName = (note: Note) => { const project = projectById.get(note.project_id); return project ? projectLabel(project, t) : t('workspace.home.noteFallback'); };
  const caseById = new Map(allNotes.map(note => [note.id, note]));
  const noteTitleLower = (note: Note) => note.title || t('workspace.home.untitledLower');
  const [followingOnly, setFollowingOnly] = useState(false);
  const [natureProposals, reloadNatureProposals] = useNatureProposals(new Set(allNotes.map(note => note.id)), allNotes, profileSpaces?.length === 1 ? profileSpaces[0] : null);
  const [reviewingNatures, setReviewingNatures] = useState(false);
  const tidy = useTidy(new Set(allNotes.map(note => note.id)), allNotes);
  const nudges = useNudges(new Set(allNotes.map(note => note.id)), allNotes);
  const profile = useWorkProfile(profileSpace, allNotes);
  const profileCount = useProfileProposalCount(profileSpaces ?? [profileSpace], profile.view);
  const openById = (id: string) => { const note = allNotes.find(candidate => candidate.id === id); if (note) onNote(note); };
  const [agentRevisions, setAgentRevisions] = useState<NoteRevision[]>([]);
  // First steps: until the person has written a case (the example's do not count) and an agent has
  // opened a session, or until they hide them.
  const [welcomeHidden, setWelcomeHiddenState] = useState(getWelcomeHidden);
  // Read again every few seconds while the first steps wait for an agent.
  const [waitingAgent, setWaitingAgent] = useState(false);
  const { agents } = useAgentsSeen(notes, waitingAgent ? 4000 : undefined);
  const exampleId = getExampleProjectId();
  const started = allNotes.some(note => note.project_id !== exampleId) && (agents?.length ?? 0) > 0;
  const welcome = Boolean(onConnectAgent) && !welcomeHidden && agents !== null && !started;
  const hideWelcome = () => { setWelcomeHidden(true); setWelcomeHiddenState(true); };
  const shouldWait = welcome && !(agents?.length);
  if (waitingAgent !== shouldWait) setWaitingAgent(shouldWait);
  const [agentRevisionsFailed, setAgentRevisionsFailed] = useState(false);

  // Agent writes of the day, reread when the notes change. A failure leaves the timeline without an
  // agent track rather than blocking the home, but it is announced: a failed read
  // must not pass for a day without agent writes.
  useEffect(() => {
    let active = true;
    const dayStart = new Date(); dayStart.setHours(0, 0, 0, 0);
    const since = dayStart.toISOString();
    Promise.all([api.listAgentRevisionsSince?.(since), api.listAgentEventsSince?.(since)])
      .then(([revisions, events]) => { if (active) { setAgentRevisions(agentWrites(revisions ?? [], events ?? [])); setAgentRevisionsFailed(false); } })
      .catch(() => { if (active) { setAgentRevisions([]); setAgentRevisionsFailed(true); } });
    return () => { active = false; };
  }, [notes]);

  // "Since your last visit" (v24), reread when the notes change or after a decision.
  const [brief, setBrief] = useState<Brief | null>(null);
  const [briefFailed, setBriefFailed] = useState(false);
  const [briefReloads, setBriefReloads] = useState(0);
  useEffect(() => {
    let active = true;
    Promise.resolve(api.getBrief?.())
      .then(result => { if (active) { setBrief(result ?? null); setBriefFailed(false); } })
      .catch(() => { if (active) { setBrief(null); setBriefFailed(true); } });
    return () => { active = false; };
  }, [notes, briefReloads]);
  const reloadBrief = () => setBriefReloads(count => count + 1);
  const toReviewIds = new Set(brief?.to_review.map(item => item.note_id) ?? []);
  // Cases the brief shows (results to review, agent writes not seen, reworks waiting): they are
  // not repeated below it. Once seen or decided, they leave the brief and come back below.
  const inBrief = new Set([...toReviewIds, ...(brief?.reworks_pending.map(item => item.note_id) ?? []), ...(brief?.changes.map(event => event.note_id) ?? [])]);

  const byUpdate = [...notes].sort(byModification);
  // The last edited case still to do: a resolved or archived case is not offered to pick up.
  const resumeNote = byUpdate.find(note => !inBrief.has(note.id) && note.status !== 'resolved' && !note.archived_at);
  // Same rule as the Today view, sub-cases included.
  const allToFollow = casesToFollow(allNotes);
  const followupNotes = allToFollow.filter(note => note.id !== resumeNote?.id && !inBrief.has(note.id));
  // The summary counts every case to follow outside the brief, the one to pick up included.
  const followingCount = allToFollow.filter(note => !inBrief.has(note.id)).length;
  const followupShown = followupNotes.slice(0, 4);
  // Neither the resume case, nor the cases to follow shown beside it, nor those of the brief.
  const shownAbove = new Set([...inBrief, ...(resumeNote ? [resumeNote.id] : []), ...followupShown.map(note => note.id)]);
  const recentNotes = byUpdate.filter(note => !shownAbove.has(note.id) && (!followingOnly || note.status !== 'resolved')).slice(0, 6);

  // `notesOfTheDay` also includes P0/P1 priorities: only real due dates are counted here.
  const dueCount = notes.filter(note => note.status !== 'resolved' && ['overdue', 'today'].includes(dueState(note.due_at) ?? '')).length;
  const visibleIds = new Set(allNotes.map(note => note.id));
  const toReviewCount = [...toReviewIds].filter(id => visibleIds.has(id)).length;
  const summary = [
    toReviewCount > 0 && t('workspace.home.summaryToReview', { count: toReviewCount }),
    followingCount > 0 && t('workspace.home.summaryFollowup', { count: followingCount }),
    dueCount > 0 && t('workspace.home.summaryDue', { count: dueCount }),
  ].filter(Boolean).join(' · ');

  return (
    <main className="workspace-page workspace-home">
      <PageHeader
        eyebrow={today.toLocaleDateString(currentDateLocale(), { weekday: 'long', day: 'numeric', month: 'long' })}
        title={welcome ? t('workspace.welcome.title') : t('workspace.home.title')}
        description={welcome ? <p>{t('workspace.welcome.intro')}</p> : <>
          <p>{notes.length ? summary || t('workspace.home.nothingActive') : t('workspace.home.ready')}</p>
          <p className="workspace-home__capture">{t('workspace.home.quickCapture')} <span className="ui-kbd-group"><kbd className="ui-kbd">Alt</kbd><kbd className="ui-kbd">{t('workspace.home.shiftKey')}</kbd><kbd className="ui-kbd">N</kbd></span></p>
        </>}
        action={welcome ? <>
          <Button type="button" variant="ghost" size="sm" onClick={hideWelcome}>{t('workspace.welcome.hide')}</Button>
          {/* Once the first case exists, step 1 no longer offers "New case": the header does. */}
          {allNotes.length > 0 && <NewCaseButton onClick={onCreateNote} />}
        </> : <NewCaseButton onClick={onCreateNote} />}
      />

      {welcome && onConnectAgent && <WelcomeGuide notes={allNotes} projects={locateProjects} agents={agents} space={profileSpace}
        onCreateNote={onCreateNote} onConnectAgent={onConnectAgent} onDataChanged={() => (onDataChanged ?? onNotesChanged)?.()} />}

      {activities.length > 0 && onActivity && <nav className="workspace-home-activities" aria-label={t('workspace.home.areasNav')}>
        <SegmentedControl variant="tabs" label={t('workspace.home.filterByArea')} value={activeActivityId ?? 'all'} onValueChange={value => onActivity(value === 'all' ? null : value)} items={[{ value: 'all', label: t('workspace.home.wholeSpace'), icon: <SquaresFour size={16} aria-hidden="true" /> }, ...activities.map(activity => ({ value: activity.id, label: activity.name }))]} />
      </nav>}

      {(!welcome || allNotes.length > 0) && <DayTimeline notes={allNotes} revisions={agentRevisions} revisionsFailed={agentRevisionsFailed} onNote={onNote} />}

      <NatureReviewCard count={natureProposals.length} onStart={() => setReviewingNatures(true)} />
      {onTidy && <TidyCard items={tidy.items} weeks={tidy.weeks} onStart={onTidy} />}
      {onGerminate && <NudgesCard nudges={nudges.nudges} onOpen={openById} onGerminate={onGerminate} onAnswered={nudges.reload} />}
      {onProfile && profile.settings?.propose !== false && <ProfileCard count={profileCount} onOpen={onProfile} />}
      {reviewingNatures && <NatureReviewDialog proposals={natureProposals} notes={allNotes} onNote={onNote}
        onClose={changed => { setReviewingNatures(false); reloadNatureProposals(); if (changed) onNotesChanged?.(); }} />}

      {/* Before any agent, "Since your last visit" has nothing to say: the first steps explain it. */}
      {!(welcome && !agents?.length) && <HomeBrief brief={brief} failed={briefFailed} notes={allNotes} projects={locateProjects} onNote={onNote} onReviewed={reloadBrief} onAcceptedAll={reloadBrief} />}

      <div className="workspace-home__layout workspace-home__layout--single">
        <div className="workspace-home__main">
          {resumeNote && <Card as="section" padding="none" className="workspace-home-band" aria-label={t('workspace.home.resumeLabel')}>
            <article className="workspace-resume" aria-labelledby="workspace-resume-heading">
              <div className="workspace-resume__eyebrow"><span className="workspace-resume__dot" style={{ backgroundColor: projectById.get(resumeNote.project_id)?.color || undefined }} aria-hidden="true" /><span>{projectName(resumeNote)}</span><span aria-hidden="true">·</span><span>{t('workspace.home.pickUp')}</span></div>
              <h2 id="workspace-resume-heading"><button type="button" onClick={() => onNote(resumeNote)} aria-label={t('workspace.home.openNote', { title: noteTitleLower(resumeNote) })}>{resumeNote.title || t('common.untitled')}</button></h2>
              <p>{notePreview(resumeNote) || t('workspace.home.emptyPreview')}</p>
              <div className="workspace-resume__footer">
                <span className="workspace-resume__meta"><StatusBadge status={resumeNote.status} /><time dateTime={parseTimestamp(resumeNote.updated_at)?.toISOString()} title={formatShortDate(resumeNote.updated_at)}>{t('workspace.home.editedAgo', { when: formatRelative(resumeNote.updated_at) })}</time></span>
                <Button type="button" variant="secondary" size="sm" className="workspace-resume__open" onClick={() => onNote(resumeNote)} aria-label={t('workspace.home.continueNote', { title: noteTitleLower(resumeNote) })}>{t('workspace.home.continue')}<ArrowRight size={16} aria-hidden="true" /></Button>
              </div>
            </article>

            <section className="workspace-followup" aria-labelledby="workspace-followup-heading">
              <div className="workspace-section__heading"><h2 id="workspace-followup-heading">{t('workspace.home.followupTitle')} <Count value={followupNotes.length} /></h2>{onSeeAllFollowup && followupNotes.length > followupShown.length && <Button type="button" variant="ghost" size="sm" onClick={onSeeAllFollowup}>{t('workspace.home.followupAll', { count: followupNotes.length })}<ArrowRight size={14} aria-hidden="true" /></Button>}</div>
              <p className="workspace-section__hint">{t('workspace.home.followupHint')}</p>
              {followupShown.length ? <List variant="divided">{followupShown.map(note =>
                <ListRow key={note.id} size="sm" onClick={() => onNote(note)} actionLabel={t('workspace.home.followupAction', { title: noteTitleLower(note) })}
                  leading={<StatusBadge status={note.status} iconOnly />} title={note.title || t('common.untitled')}
                  meta={<><CaseLocation note={note} byId={caseById} projectById={projectById} />{note.priority && <PriorityBadge priority={note.priority} status={note.status} />}{note.due_at && <DueBadge due={note.due_at} />}</>}
                  trailing={<ArrowRight size={15} aria-hidden="true" />} />)}</List> : <EmptyState size="sm" className="workspace-home__quiet" description={allToFollow.length > 0 ? t('workspace.home.followupShownAbove') : t('workspace.home.followupEmpty')} />}
            </section>
          </Card>}

          {welcome && allNotes.length === 0 ? null : projects.length === 0
            ? <EmptyState icon={<Folder />} title={t('workspace.home.firstProjectTitle')} description={t('workspace.home.firstProjectDesc')} action={<Button type="button" variant="secondary" onClick={onCreateProject}><Plus size={17} aria-hidden="true" />{t('workspace.home.createProject')}</Button>} />
            : <section className="workspace-section workspace-home__recent" aria-labelledby="workspace-recent-heading">
            <div className="workspace-section__heading"><h2 id="workspace-recent-heading">{t('workspace.home.recentTitle')}</h2><SegmentedControl className="workspace-home-note-filter" size="sm" label={t('workspace.home.recentTitle')} value={followingOnly ? 'following' : 'all'} onValueChange={value => setFollowingOnly(value === 'following')} items={[{ value: 'all', label: t('workspace.home.filterAll') }, { value: 'following', label: t('workspace.home.filterFollowing'), icon: <CircleDashed size={14} aria-hidden="true" /> }]} /></div>
            {recentNotes.length ? <div className="workspace-note-grid">{recentNotes.map(note => <NoteCard key={note.id} note={note} project={projectById.get(note.project_id)} onOpen={onNote} subCaseCount={subCaseCounts?.get(note.id)} />)}</div> : <EmptyState icon={<StickyNote />} title={followingOnly ? t('workspace.home.emptyFollowingTitle') : notes.length ? t('workspace.home.emptyRecentTitle') : t('workspace.home.emptyNoneTitle')} description={followingOnly ? t('workspace.home.emptyFollowingDesc') : t('workspace.home.emptyDesc')} action={<Button type="button" variant="secondary" onClick={onCreateNote}><Plus size={17} aria-hidden="true" />{t('workspace.home.createNote')}</Button>} />}
          </section>}
        </div>
      </div>

      {!(welcome && allNotes.length === 0) && children}
    </main>
  );
}
