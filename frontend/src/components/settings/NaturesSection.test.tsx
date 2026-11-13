import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { NaturesSection } from './NaturesSection';

const api = vi.hoisted(() => ({
  natureReadingStatus: vi.fn(),
  setNatureReading: vi.fn(async () => undefined),
  listAiProviders: vi.fn(),
  setAiKey: vi.fn(async () => undefined),
  recheckNatureReading: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

const typesafe = (keyDefined: boolean, source: 'app' | 'env' | 'none') => ({
  cleanup: [], transcription: [],
  reading: [{ id: 'typesafe', label: 'TypeSafe (Jev)', key_env: 'TYPESAFE_API_KEY', default_model: '', url_env: '', model_env: '', needs_key: true, key_defined: keyDefined, key_source: source, url_editable: false, url: '', ready: keyDefined }],
});

describe('Natures to check settings', () => {
  it('is off until the person turns it on', async () => {
    api.natureReadingStatus.mockResolvedValueOnce({ state: 'off', read: 0, total: 0, error: null })
      .mockResolvedValue({ state: 'no_key', read: 0, total: 0, error: null });
    api.listAiProviders.mockResolvedValue(typesafe(false, 'none'));
    render(<NaturesSection />);
    expect(await screen.findByText('Désactivée : aucune case n’est envoyée.')).toBeTruthy();
    expect(screen.getByText(/Une nature que vous avez choisie ne l’est jamais/)).toBeTruthy();
    const box = screen.getByRole('checkbox', { name: 'Faire relire les natures' }) as HTMLInputElement;
    await waitFor(() => expect(box.disabled).toBe(false));
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    await waitFor(() => expect(api.setNatureReading).toHaveBeenCalledWith(true));
    expect(await screen.findByText('Il manque une clé TypeSafe pour relire les cases.')).toBeTruthy();
  });

  it('sends the person to AI services when the key is missing', async () => {
    api.natureReadingStatus.mockResolvedValue({ state: 'no_key', read: 0, total: 0, error: null });
    const openAi = vi.fn();
    render(<NaturesSection onOpenAiServices={openAi} />);
    fireEvent.click(await screen.findByRole('button', { name: 'Ajouter la clé dans Services IA' }));
    expect(openAi).toHaveBeenCalled();
    expect(screen.queryByLabelText(/Clé TypeSafe/)).toBeNull();
  });
});
