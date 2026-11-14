import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { UpdatesSection } from './UpdatesSection';

const api = vi.hoisted(() => ({
  getUpdateSettings: vi.fn(),
  setUpdateCheck: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

describe('Updates settings', () => {
  it('turns the check at startup off', async () => {
    api.getUpdateSettings.mockResolvedValue({ supported: true, enabled: true });
    render(<UpdatesSection />);
    const box = await screen.findByRole('checkbox', { name: 'Chercher les mises à jour au démarrage' }) as HTMLInputElement;
    expect(box.checked).toBe(true);
    fireEvent.click(box);
    await waitFor(() => expect(api.setUpdateCheck).toHaveBeenCalledWith(false));
    await waitFor(() => expect(box.checked).toBe(false));
  });

  it('says a copy built on this computer does not update itself', async () => {
    api.getUpdateSettings.mockResolvedValue({ supported: false, enabled: true });
    render(<UpdatesSection />);
    expect(await screen.findByText(/compilée sur cet ordinateur/)).toBeTruthy();
    expect(screen.queryByRole('checkbox')).toBeNull();
  });
});
