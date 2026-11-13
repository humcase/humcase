import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));

const setTitle = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ hide: vi.fn().mockResolvedValue(undefined), onFocusChanged: vi.fn().mockResolvedValue(() => {}), setTitle }),
}));

vi.mock('./services/tauriApi', () => ({
  api: { getProjects: vi.fn(), createNote: vi.fn(), getOrCreateInbox: vi.fn() },
}));

import QuickCaptureApp from './QuickCaptureApp';
import { api } from './services/tauriApi';
import i18n from './i18n';

describe('Quick capture in English', () => {
  beforeEach(async () => {
    localStorage.clear();
    vi.mocked(api.getProjects).mockResolvedValue([
      { id: 'p1', name: 'Campaign', color: '#3b82f6', created_at: '2026-09-22T10:00:00.000Z', updated_at: '2026-09-22T10:00:00.000Z', revision: 0, space: null },
    ]);
    await i18n.changeLanguage('en');
  });
  afterEach(async () => {
    await i18n.changeLanguage('fr');
  });

  it('shows the window, the destinations and the types in English', async () => {
    render(<QuickCaptureApp />);
    await waitFor(() => expect(api.getProjects).toHaveBeenCalled());
    expect(screen.getByText('Quick capture')).toBeTruthy();
    expect(screen.getByText('Esc to close')).toBeTruthy();
    expect(screen.getByRole('textbox', { name: 'Capture text' })).toBeTruthy();
    expect(screen.getByPlaceholderText('I need to…')).toBeTruthy();
    expect(screen.getByRole('combobox', { name: 'Case destination' })).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Inbox · Work' })).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Inbox · Personal' })).toBeTruthy();
    expect(screen.getByRole('option', { name: 'Issue' })).toBeTruthy();
    expect(screen.queryByText('Capture rapide')).toBeNull();
    expect(setTitle).toHaveBeenCalledWith('Humcase · Quick capture');
  });
});
