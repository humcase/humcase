import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { MemorySection } from './MemorySection';

const api = vi.hoisted(() => ({
  tidySettings: vi.fn(async () => ({ archive_weeks: 4, supersede: true, group: true })),
  setTidySettings: vi.fn(async (settings: { archive_weeks: number; supersede: boolean; group: boolean }) => settings),
}));
vi.mock('../../services/tauriApi', () => ({ api }));

describe('Memory settings', () => {
  it('changes the archive delay and turns the search for replaced decisions off', async () => {
    render(<MemorySection />);
    const four = await screen.findByRole('radio', { name: '4 semaines' });
    expect(four.getAttribute('aria-checked')).toBe('true');
    fireEvent.click(screen.getByRole('radio', { name: 'Jamais' }));
    await waitFor(() => expect(api.setTidySettings).toHaveBeenCalledWith({ archive_weeks: 0, supersede: true, group: true }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Comparer les décisions : remplacées ou contredites' }));
    await waitFor(() => expect(api.setTidySettings).toHaveBeenLastCalledWith({ archive_weeks: 0, supersede: false, group: true }));
    fireEvent.click(screen.getByRole('checkbox', { name: 'Proposer de regrouper les cases proches' }));
    await waitFor(() => expect(api.setTidySettings).toHaveBeenLastCalledWith({ archive_weeks: 0, supersede: false, group: false }));
  });
});
