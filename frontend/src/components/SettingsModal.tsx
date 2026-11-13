import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { TFunction } from 'i18next';
import { ArrowLeft, Microphone as Mic, SlidersHorizontal, Plug, Compass, HardDrive, Robot as Bot } from '@phosphor-icons/react';
import './settings.css';
import type { Activity, ProjectSpace } from '../types';
import type { BackupPreparation } from '../utils/backupBarrier';
import { loadSpacePreferences, type SpacePreferences, type SpaceUsage } from '../utils/spaces';
import { Button } from './ui/Button';
import { Dialog, DialogTitle } from './ui/Dialog';
import { useMicrophones } from './settings/useMicrophones';
import { useAiProviders } from './settings/useAiProviders';
import { useAgentPermission } from './settings/useAgentPermission';
import { useArchiveRestore } from './settings/useArchiveRestore';
import { SpacesSection } from './settings/SpacesSection';
import { AppearanceSection } from './settings/AppearanceSection';
import { DictationSection } from './settings/DictationSection';
import { AiServicesSection } from './settings/AiServicesSection';
import { AgentSection } from './settings/AgentSection';
import { DataSection } from './settings/DataSection';
import { MeaningSection } from './settings/MeaningSection';
import { MemorySection } from './settings/MemorySection';
import { ProfileSection } from './settings/ProfileSection';
import { NaturesSection } from './settings/NaturesSection';
import { UsageSection } from './settings/UsageSection';

interface SettingsModalProps {
  onClose: () => void;
  prepareBackup: () => Promise<BackupPreparation>;
  spacePreferences?: SpacePreferences;
  onSpaceUsage?: (usage: SpaceUsage) => void;
  onCombinedView?: (enabled: boolean) => void;
  activities?: Activity[];
  onCreateActivity?: (name: string, space: ProjectSpace) => Promise<void>;
  onRenameActivity?: (id: string, name: string) => Promise<void>;
  onDeleteActivity?: (id: string) => Promise<void>;
  focusActivities?: boolean;
  /** Opens directly on AI services ("Connect an AI" link in the editor). */
  focusAi?: boolean;
  /** Opens directly on Agents (Home's first steps). */
  focusAgent?: boolean;
  theme?: 'dark' | 'light';
  onThemeChange?: (theme: 'dark' | 'light') => void;
  /** Opens the work profile of a space (Settings close). */
  onOpenProfile?: (space: ProjectSpace) => void;
}

type SettingsSection = 'general' | 'dictation' | 'ai' | 'proposals' | 'agent' | 'data';

/** Navigation sections; labels are resolved at render time (language change). One icon per
 * section, none shared with another place of the app. */
const sections = (t: TFunction): { id: SettingsSection; label: string; icon: typeof Mic }[] => [
  { id: 'general', label: t('settings.sections.general'), icon: SlidersHorizontal },
  { id: 'dictation', label: t('settings.sections.dictation'), icon: Mic },
  { id: 'ai', label: t('settings.sections.ai'), icon: Plug },
  { id: 'proposals', label: t('settings.sections.proposals'), icon: Compass },
  { id: 'agent', label: t('settings.sections.agent'), icon: Bot },
  { id: 'data', label: t('settings.sections.data'), icon: HardDrive },
];

/** A page that gathers several settings: its title and what it is for, then each part under a
 * smaller heading. */
function SettingsGroup({ title, lead, children }: { title: string; lead: ReactNode; children: ReactNode }) {
  return (
    <div className="settings-group">
      <div className="settings-panel"><div className="settings-section-heading"><h3>{title}</h3>{lead}</div></div>
      {children}
    </div>
  );
}

/**
 * Honest explanation of the MCP agent's write scope (what is and is not editable).
 *
 * The flag governs ONLY `update_note` (title, text, type, tags, priority). The pre-existing dedicated
 * MCP tools (`update_note_status`, `set_due_date`) do not depend on this setting: the
 * text must therefore say so explicitly so as not to suggest broader gating.
 *
 * This literal French constant is only a reference for tests (a computed constant
 * would fail the hot-reload rule); the screen displays
 * `t('settings.agent.permissionText')`, which follows the current language. A test guarantees they match.
 */
export const AGENT_PERMISSION_TEXT =
  "Actif : l’agent peut modifier titre, texte, nature, tags, priorité et échéance. Chacune de ces modifications est datée, signée, et l’ancienne version gardée. Jamais la dictée brute, jamais de suppression. Même désactivé, l’agent peut changer le statut, écrire un compte rendu et poser une échéance, par ses outils dédiés.";

/**
 * Settings, one section at a time. The state of each section lives here, in its hook, for as long
 * as Settings are open: changing section never drops a status, a typed key or a backup in
 * progress, and nothing is read before Settings open.
 */
export default function SettingsModal({ onClose, prepareBackup, spacePreferences = loadSpacePreferences(), onSpaceUsage = () => {}, onCombinedView = () => {}, activities = [], onCreateActivity = async () => {}, onRenameActivity = async () => {}, onDeleteActivity = async () => {}, focusActivities = false, focusAi = false, focusAgent = false, theme = 'dark', onThemeChange = () => {}, onOpenProfile }: SettingsModalProps) {
  const { t } = useTranslation();
  const profileSpaces: ProjectSpace[] = spacePreferences.usage === 'both' ? ['work', 'private'] : [spacePreferences.usage];
  const [activeSection, setActiveSection] = useState<SettingsSection>(focusActivities ? 'general' : focusAi ? 'ai' : focusAgent ? 'agent' : 'general');
  const activitySettingsRef = useRef<HTMLDivElement | null>(null);
  useEffect(() => {
    if (!focusActivities || activeSection !== 'general') return;
    const timer = setTimeout(() => activitySettingsRef.current?.scrollIntoView?.({ block: 'start' }), 0);
    return () => clearTimeout(timer);
  }, [focusActivities, activeSection]);
  const microphones = useMicrophones();
  const data = useArchiveRestore(prepareBackup);
  const ai = useAiProviders();
  const agentPermission = useAgentPermission(activeSection === 'agent');

  const contentRef = useRef<HTMLElement>(null);
  // Escape and the back button go through here: never close during a restore.
  const go = (id: SettingsSection) => {
    if (id === 'agent') agentPermission.clearError();
    setActiveSection(id);
    if (contentRef.current) contentRef.current.scrollTop = 0;
  };
  const handleClose = () => {
    if (!data.restoringRef.current) onClose();
  };

  return (
    <Dialog onClose={handleClose} size="page" className="settings-page">
      <div className="settings-dialog">
        <header className="settings-header">
          <div>
            <DialogTitle className="settings-title">{t('settings.title')}</DialogTitle>
          </div>
        </header>

        <div className="settings-layout">
          <nav className="settings-nav" aria-label={t('settings.navAria')}>
            {sections(t).map(({ id, label, icon: Icon }) => (
              <Button
                key={id}
                type="button"
                variant="nav"
                className="settings-nav__button"
                aria-current={activeSection === id ? 'page' : undefined}
                onClick={() => go(id)}
                disabled={data.isRestoring}
              >
                <Icon size={18} aria-hidden="true" />
                <span>{label}</span>
              </Button>
            ))}
            <Button
              onClick={handleClose}
              disabled={data.isRestoring}
              variant="nav"
              className="settings-nav__button settings-nav__back"
            >
              <ArrowLeft size={18} aria-hidden="true" /><span>{t('settings.back')}</span>
            </Button>
          </nav>
          <main className="settings-content" ref={contentRef}>
            {activeSection === 'general' && <SettingsGroup title={t('settings.sections.general')} lead={<p>{t('settings.general.description')}</p>}>
              <AppearanceSection theme={theme} onThemeChange={onThemeChange} />
              <SpacesSection spacePreferences={spacePreferences} onSpaceUsage={onSpaceUsage} onCombinedView={onCombinedView}
                activities={activities} onCreateActivity={onCreateActivity} onRenameActivity={onRenameActivity} onDeleteActivity={onDeleteActivity} activitiesRef={activitySettingsRef} />
            </SettingsGroup>}
            {activeSection === 'dictation' && <DictationSection microphones={microphones} ai={ai} onOpenAiServices={() => go('ai')} />}
            {activeSection === 'ai' && <SettingsGroup title={t('settings.sections.ai')} lead={<p>{t('settings.aiServices.description')}</p>}>
              <AiServicesSection ai={ai} />
              <UsageSection />
            </SettingsGroup>}
            {activeSection === 'proposals' && <SettingsGroup title={t('settings.sections.proposals')} lead={<><p>{t('settings.proposals.description')}</p><p>{t('settings.proposals.review')}</p></>}>
              <MeaningSection />
              <MemorySection />
              <NaturesSection onOpenAiServices={() => go('ai')} />
              <ProfileSection spaces={profileSpaces} onOpenProfile={onOpenProfile} />
            </SettingsGroup>}
            {activeSection === 'agent' && <AgentSection permission={agentPermission} />}
            {activeSection === 'data' && <DataSection data={data} />}
          </main>
        </div>
      </div>
    </Dialog>
  );
}
