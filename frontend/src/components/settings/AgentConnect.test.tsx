import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { api } from '../../services/tauriApi';
import { AgentConnect } from './AgentConnect';

vi.mock('../../services/tauriApi', () => ({ api: { listAgentsSeen: vi.fn(), humcaseExecutablePath: vi.fn() } }));

const exe = String.raw`C:\Apps\Humcase\Humcase.exe`;

beforeEach(() => {
  vi.resetAllMocks();
  vi.mocked(api.humcaseExecutablePath).mockResolvedValue(exe);
});

describe('Settings → Agents, connecting one', () => {
  it('says no agent is connected yet and gives the command with this Humcase\'s path', async () => {
    vi.mocked(api.listAgentsSeen).mockResolvedValue([]);
    render(<AgentConnect />);
    expect(await screen.findByText('Aucun agent branché pour l’instant')).toBeTruthy();
    expect(await screen.findByText(`claude mcp add --scope user humcase -- "${exe}" mcp`)).toBeTruthy();
    fireEvent.click(screen.getByRole('radio', { name: 'Codex' }));
    expect(screen.getByText('2. Ajoutez ceci à ~/.codex/config.toml')).toBeTruthy();
    expect(screen.getByText('3. Relancez Codex, puis demandez-lui :')).toBeTruthy();
  });

  it('names the last agent seen and the others', async () => {
    vi.mocked(api.listAgentsSeen).mockResolvedValue([{ name: 'Codex', at_unix_ms: Date.now() }, { name: 'Claude Code', at_unix_ms: 1 }]);
    render(<AgentConnect />);
    expect(await screen.findByText('Codex est branché')).toBeTruthy();
    expect(screen.getByText('Aussi : Claude Code')).toBeTruthy();
  });

  it('copies the command, and says so when the copy is refused', async () => {
    vi.mocked(api.listAgentsSeen).mockResolvedValue([]);
    const writeText = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValueOnce(new Error('denied'));
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true });
    render(<AgentConnect />);
    await screen.findByText(`claude mcp add --scope user humcase -- "${exe}" mcp`);
    fireEvent.click(screen.getByRole('button', { name: 'Copier' }));
    expect(await screen.findByRole('button', { name: 'Copié' })).toBeTruthy();
    expect(writeText).toHaveBeenCalledWith(`claude mcp add --scope user humcase -- "${exe}" mcp`);
    fireEvent.click(screen.getByRole('button', { name: 'Copié' }));
    expect((await screen.findByRole('alert')).textContent).toContain('Copie impossible');
  });
});
