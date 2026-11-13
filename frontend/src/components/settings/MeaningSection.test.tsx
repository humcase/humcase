import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MeaningSection } from './MeaningSection';

const api = vi.hoisted(() => ({
  meaningStatus: vi.fn(),
  setMeaningEnabled: vi.fn(async () => undefined),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

describe('Search by meaning settings', () => {
  it('says how many cases are read and turns search by meaning off', async () => {
    api.meaningStatus.mockResolvedValueOnce({ state: 'ready', done: 451, total: 451, error: null })
      .mockResolvedValue({ state: 'off', done: 0, total: 0, error: null });
    render(<MeaningSection />);
    expect(await screen.findByText(/Prête · 451 cases lues/)).toBeTruthy();
    const box = screen.getByRole('checkbox', { name: 'Activer la recherche par le sens' }) as HTMLInputElement;
    expect(box.checked).toBe(true);
    fireEvent.click(box);
    await waitFor(() => expect(api.setMeaningEnabled).toHaveBeenCalledWith(false));
    expect(await screen.findByText(/Désactivée/)).toBeTruthy();
  });

  it('keeps word search when meaning is unavailable on this computer', async () => {
    api.meaningStatus.mockResolvedValue({ state: 'unavailable', done: 0, total: 0, error: 'runtime not found' });
    render(<MeaningSection />);
    expect(await screen.findByText(/Indisponible sur cet ordinateur/)).toBeTruthy();
  });

  it('says the reading starts when turned back on, before any case is counted', async () => {
    api.meaningStatus.mockResolvedValueOnce({ state: 'off', done: 0, total: 0, error: null })
      .mockResolvedValue({ state: 'reading', done: 0, total: 0, error: null });
    render(<MeaningSection />);
    const box = await screen.findByRole('checkbox', { name: 'Activer la recherche par le sens' }) as HTMLInputElement;
    await waitFor(() => expect(box.disabled).toBe(false));
    expect(box.checked).toBe(false);
    fireEvent.click(box);
    await waitFor(() => expect(api.setMeaningEnabled).toHaveBeenCalledWith(true));
    expect(await screen.findByText('Démarrage de la lecture des cases…')).toBeTruthy();
  });
});
