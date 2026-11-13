import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ArrowRight, Check, Plus } from '@phosphor-icons/react';
import { Button } from '../ui/Button';
import { Card } from '../ui/Card';
import { api } from '../../services/tauriApi';
import { getExampleProjectId, setExampleProjectId } from '../../utils/prefs';
import type { AgentSeen, Note, Project, ProjectSpace } from '../../types';

/**
 * Home's first steps, until the person has written a case and connected an agent, or hides them:
 * what a case is, the two steps with what is done ticked from real data (cases written, agents
 * that opened a session), quick capture, and how work with an agent goes. A filled example can be
 * added and removed (archived) again; its cases do not count as the person's first case.
 */
export function WelcomeGuide({ notes, projects, agents, space, onCreateNote, onConnectAgent, onDataChanged }: {
  /** Every visible case, sub-cases included. */
  notes: Note[];
  projects: Project[];
  agents: AgentSeen[] | null;
  /** Space the example project is created in. */
  space: ProjectSpace;
  onCreateNote: () => void;
  onConnectAgent: () => void;
  /** Projects and cases changed (example added or removed): they are read again. */
  onDataChanged: () => void;
}) {
  const { t } = useTranslation();
  const [exampleId, setExampleId] = useState(getExampleProjectId);
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const example = exampleId ? projects.find(project => project.id === exampleId && !project.archived_at) : undefined;
  const wrote = notes.some(note => note.project_id !== example?.id);
  const agent = agents?.[0];
  const doneCount = (wrote ? 1 : 0) + (agent ? 1 : 0);

  const addExample = async () => {
    setCreating(true); setError(null);
    try {
      const e = {
        project: t('workspace.welcome.example.project'), main: t('workspace.welcome.example.main'), mainText: t('workspace.welcome.example.mainText'),
        payment: t('workspace.welcome.example.payment'), products: t('workspace.welcome.example.products'), abroad: t('workspace.welcome.example.abroad'),
        logo: t('workspace.welcome.example.logo'), logoText: t('workspace.welcome.example.logoText'), phone: t('workspace.welcome.example.phone'),
      };
      const projectId = await api.createProject(e.project, undefined, space);
      const main = await api.createNote({ project_id: projectId, title: e.main, content: e.mainText, note_type: 'idea' });
      await api.createSubCase(main, e.payment, 'research');
      await api.createSubCase(main, e.products, 'todo');
      await api.createSubCase(main, e.abroad, 'question');
      await api.createNote({ project_id: projectId, title: e.logo, content: e.logoText, note_type: 'decision' });
      await api.createNote({ project_id: projectId, title: e.phone, note_type: 'todo' });
      setExampleProjectId(projectId); setExampleId(projectId);
      onDataChanged();
    } catch (cause) { setError(t('workspace.welcome.exampleError', { error: String(cause) })); }
    finally { setCreating(false); }
  };
  const removeExample = async () => {
    if (!example) return;
    try {
      await api.setProjectArchived(example.id, true);
      setExampleProjectId(null); setExampleId(null);
      onDataChanged();
    } catch (cause) { setError(String(cause)); }
  };

  const steps = [
    { key: 'write', done: wrote, title: t('workspace.welcome.writeTitle'), text: t('workspace.welcome.writeText'),
      action: wrote ? null : <Button type="button" onClick={onCreateNote}><Plus size={17} aria-hidden="true" />{t('workspace.welcome.writeAction')}</Button>,
      doneLabel: t('workspace.welcome.writeDone') },
    { key: 'agent', done: Boolean(agent), title: t('workspace.welcome.agentTitle'), text: t('workspace.welcome.agentText'),
      action: agent ? null : <Button type="button" variant={wrote ? 'default' : 'secondary'} onClick={onConnectAgent}>{t('workspace.welcome.agentAction')}<ArrowRight size={16} aria-hidden="true" /></Button>,
      doneLabel: agent ? t('workspace.welcome.agentDone', { name: agent.name }) : '' },
  ];

  return (
    <div className="workspace-welcome">
      <section className="workspace-welcome__steps" aria-label={t('workspace.welcome.stepsLabel')}>
        {steps.map((step, index) => (
          <Card as="article" key={step.key} padding="none" className="workspace-welcome__step" data-done={step.done || undefined}>
            <div className="workspace-welcome__kicker">
              {/* The step's number is in its label; the badge only marks a step done. */}
              {step.done && <span className="workspace-welcome__badge" aria-hidden="true"><Check size={15} weight="bold" /></span>}
              <span>{t('workspace.welcome.step', { n: index + 1 })}</span>
            </div>
            <h2>{step.title}</h2>
            <p>{step.text}</p>
            {step.done ? <span className="workspace-welcome__done"><Check size={15} aria-hidden="true" />{step.doneLabel}</span> : step.action}
          </Card>
        ))}
        <Card as="article" padding="none" className="workspace-welcome__step">
          <div className="workspace-welcome__kicker"><span>{t('workspace.welcome.anyTime')}</span></div>
          <h2>{t('workspace.welcome.captureTitle')}</h2>
          <p>{t('workspace.welcome.captureText')}</p>
          <span className="ui-kbd-group workspace-welcome__keys"><kbd className="ui-kbd">Alt</kbd><kbd className="ui-kbd">{t('workspace.home.shiftKey')}</kbd><kbd className="ui-kbd">N</kbd></span>
        </Card>
      </section>
      <p className="workspace-welcome__progress">{t('workspace.welcome.progress', { count: doneCount })}</p>

      <section className="workspace-welcome__how" aria-labelledby="workspace-welcome-how">
        <h2 id="workspace-welcome-how">{t('workspace.welcome.howHeading')}</h2>
        <div className="workspace-welcome__how-grid">
          <div><strong>{t('workspace.welcome.howWrite')}</strong><span>{t('workspace.welcome.howWriteText')}</span></div>
          <div><strong>{t('workspace.welcome.howAgent')}</strong><span>{t('workspace.welcome.howAgentText')}</span></div>
          <div><strong>{t('workspace.welcome.howDecide')}</strong><span>{t('workspace.welcome.howDecideText')}</span></div>
        </div>
      </section>

      <div className="workspace-welcome__example">
        {example
          ? <><span role="status">{t('workspace.welcome.exampleAdded', { name: example.name, count: 6 })}</span><Button type="button" variant="ghost" size="sm" onClick={() => { void removeExample(); }}>{t('workspace.welcome.exampleRemove')}</Button></>
          : <Button type="button" variant="secondary" disabled={creating} onClick={() => { void addExample(); }}>{creating ? t('workspace.welcome.exampleCreating') : t('workspace.welcome.exampleShow')}</Button>}
        {error && <p role="alert" className="workspace-welcome__error">{error}</p>}
      </div>
    </div>
  );
}
