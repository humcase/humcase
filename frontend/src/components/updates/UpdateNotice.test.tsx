import { describe, expect, it, vi } from 'vitest';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { useAppUpdate, UPDATE_CHECK_DELAY_MS } from '../../hooks/useAppUpdate';
import { UpdateNotice } from './UpdateNotice';

const calls: string[] = [];
const api = vi.hoisted(() => ({
  checkForUpdate: vi.fn(),
  downloadUpdate: vi.fn(),
  installUpdate: vi.fn(),
}));
vi.mock('../../services/tauriApi', () => ({ api }));
vi.mock('../../utils/beforeExit', () => ({ saveBeforeExit: vi.fn(async () => { calls.push('save'); }) }));

function Harness() {
  const { update, install, dismiss } = useAppUpdate();
  return <div className="workspace-app"><UpdateNotice update={update} onInstall={install} onDismiss={dismiss} /></div>;
}

async function showAvailable() {
  vi.useFakeTimers();
  api.checkForUpdate.mockResolvedValue({ version: '0.10.0', current_version: '0.9.99' });
  render(<Harness />);
  await act(async () => { await vi.advanceTimersByTimeAsync(UPDATE_CHECK_DELAY_MS); });
  vi.useRealTimers();
}

describe('update message', () => {
  it('says nothing when there is no update or the check fails', async () => {
    vi.useFakeTimers();
    api.checkForUpdate.mockRejectedValue(new Error('offline'));
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    render(<Harness />);
    await act(async () => { await vi.advanceTimersByTimeAsync(UPDATE_CHECK_DELAY_MS); });
    vi.useRealTimers();
    expect(screen.queryByRole('status')).toBeNull();
    expect(screen.queryByRole('alert')).toBeNull();
    warn.mockRestore();
  });

  it('downloads, saves what the editors hold, then installs', async () => {
    calls.length = 0;
    api.downloadUpdate.mockImplementation(async (onProgress: (p: number) => void) => { onProgress(40); calls.push('download'); });
    api.installUpdate.mockImplementation(async () => { calls.push('install'); });
    await showAvailable();
    expect(screen.getByText('Humcase 0.10.0 est disponible.')).toBeTruthy();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Installer et redémarrer' })); });
    expect(calls).toEqual(['download', 'save', 'install']);
    expect(screen.getByText('Installation… Humcase va redémarrer.')).toBeTruthy();
  });

  it('offers to retry when the download fails, and never installs', async () => {
    api.downloadUpdate.mockRejectedValue('signature mismatch');
    api.installUpdate.mockClear();
    await showAvailable();
    await act(async () => { fireEvent.click(screen.getByRole('button', { name: 'Installer et redémarrer' })); });
    expect(screen.getByRole('alert').textContent).toContain('La mise à jour a échoué : signature mismatch');
    expect(api.installUpdate).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Réessayer' })).toBeTruthy();
  });

  it('puts the update off until the next launch', async () => {
    await showAvailable();
    fireEvent.click(screen.getByRole('button', { name: 'Plus tard' }));
    expect(screen.queryByText('Humcase 0.10.0 est disponible.')).toBeNull();
  });
});
