import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import i18n from '../i18n';
import SettingsModal from './SettingsModal';
import { ActivityPreferences } from './workspace/ActivityPreferences';
import { STRUCTURE_FRAMES, templateLabel } from '../utils/prefs';
import type { Activity } from '../types';
import { AGENT_PERMISSION_TEXT } from './SettingsModal';

const activity = (id: string, projects: number): Activity => ({
  id, name: 'Recherche', space: 'work', created_at: '2026-09-01T10:00:00Z', updated_at: '2026-09-01T10:00:00Z', revision: 0, project_ids: Array.from({ length: projects }, (_, i) => `p${i}`),
});

const renderActivities = (activities: Activity[]) => render(
  <ActivityPreferences activities={activities} enabledSpaces={['work']} onCreate={vi.fn()} onRename={vi.fn()} onDelete={vi.fn()} />,
);

describe('Settings: texts by language', () => {
  it('keeps the original French plurals (0 and 1 included)', () => {
    renderActivities([activity('a', 0), activity('b', 1), activity('c', 3)]);
    expect(screen.getByText('0 projets')).toBeTruthy();
    expect(screen.getByText('1 projet')).toBeTruthy();
    expect(screen.getByText('3 projets')).toBeTruthy();
  });

  it('shows the Settings sections in English then goes back to French', async () => {
    await i18n.changeLanguage('en');
    try {
      render(<SettingsModal onClose={vi.fn()} prepareBackup={async () => ({ ready: true })} />);
      for (const name of ['General', 'Dictation', 'AI services', 'Suggestions', 'Agents', 'Data']) {
        expect(screen.getByRole('button', { name })).toBeTruthy();
      }
    } finally {
      await i18n.changeLanguage('fr');
    }
    expect(i18n.t('settings.sections.proposals')).toBe('Propositions');
  });

  it('shows the activities in English with English plurals', async () => {
    await i18n.changeLanguage('en');
    try {
      renderActivities([activity('a', 1), activity('b', 2)]);
      expect(screen.getByText('1 project')).toBeTruthy();
      expect(screen.getByText('2 projects')).toBeTruthy();
      expect(screen.getByRole('heading', { name: 'Your areas' })).toBeTruthy();
      expect(screen.getByLabelText('New Work area')).toBeTruthy();
    } finally {
      await i18n.changeLanguage('fr');
    }
  });

  it('the reference constant of the agent scope matches the French text', () => {
    expect(i18n.t('settings.agent.permissionText')).toBe(AGENT_PERMISSION_TEXT);
  });

  it('resolves the frames at read time, not at import', async () => {
    expect(templateLabel('auto')).toBe('Automatique (selon le texte et le projet)');
    await i18n.changeLanguage('en');
    try {
      expect(templateLabel('auto')).toBe('Automatic (based on the text and project)');
      expect(STRUCTURE_FRAMES.find(frame => frame.id === 'research')?.label).toBe('Research');
    } finally {
      await i18n.changeLanguage('fr');
    }
    expect(STRUCTURE_FRAMES.find(frame => frame.id === 'research')?.label).toBe('Recherche');
  });
});
