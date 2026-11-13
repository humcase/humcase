import { describe, expect, it } from 'vitest';
import { agentSetup } from './agentSetup';

const exe = String.raw`C:\Program Files\Humcase\Humcase.exe`;

describe('agent setup snippets', () => {
  it('writes the path into each client\'s own format', () => {
    expect(agentSetup('claude', exe)).toEqual({ file: null, code: String.raw`claude mcp add --scope user humcase -- "C:\Program Files\Humcase\Humcase.exe" mcp` });
    expect(agentSetup('codex', exe).code).toBe(String.raw`[mcp_servers.humcase]` + '\n' + String.raw`command = 'C:\Program Files\Humcase\Humcase.exe'` + '\nargs = ["mcp"]');
    expect(JSON.parse(agentSetup('cursor', exe).code)).toEqual({ mcpServers: { humcase: { command: exe, args: ['mcp'] } } });
    expect(JSON.parse(agentSetup('opencode', exe).code).mcp.humcase).toEqual({ type: 'local', command: [exe, 'mcp'], enabled: true });
  });

  it('keeps short JSON arrays on one line', () => {
    expect(agentSetup('cursor', exe).code).toContain('"args": ["mcp"]');
    expect(agentSetup('opencode', exe).code).toContain(String.raw`"command": ["C:\\Program Files\\Humcase\\Humcase.exe", "mcp"]`);
  });

  it('keeps a TOML path with a single quote valid', () => {
    expect(agentSetup('codex', String.raw`C:\Users\O'Neil\Humcase.exe`).code).toContain(String.raw`command = "C:\\Users\\O'Neil\\Humcase.exe"`);
  });
});
