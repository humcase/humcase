/** The agent clients Settings → Agents explains how to connect (docs/AGENT_SETUP.md). */
export const AGENT_TOOLS = ['claude', 'codex', 'cursor', 'opencode'] as const;
export type AgentTool = (typeof AGENT_TOOLS)[number];

export interface AgentSetup {
  /** Where the snippet goes: a terminal, or the client's configuration file. */
  file: string | null;
  /** The command or the configuration, with this Humcase's path written in. */
  code: string;
}

/**
 * What to paste to connect `tool` to the Humcase at `exePath`, as each client documents it: Claude
 * Code's command (user scope, so the Claude desktop app sees it too), Codex's `config.toml`, Cursor's
 * `mcp.json`, opencode's `opencode.json`. Paths keep their backslashes: TOML single quotes take them
 * literally, JSON gets them escaped.
 */
export function agentSetup(tool: AgentTool, exePath: string): AgentSetup {
  // Indented JSON, with short arrays kept on one line as the clients' docs write them.
  const json = (value: unknown) => JSON.stringify(value, null, 2)
    .replace(/\[\n\s+([^[\]{}]*?)\n\s*\]/g, (_, items: string) => `[${items.split(/,\n\s+/).join(', ')}]`);
  switch (tool) {
    case 'claude':
      return { file: null, code: `claude mcp add --scope user humcase -- "${exePath}" mcp` };
    case 'codex': {
      // A single-quoted TOML string cannot hold a single quote: the rare path with one is escaped.
      const command = exePath.includes("'") ? JSON.stringify(exePath) : `'${exePath}'`;
      return { file: '~/.codex/config.toml', code: `[mcp_servers.humcase]\ncommand = ${command}\nargs = ["mcp"]` };
    }
    case 'cursor':
      return { file: '~/.cursor/mcp.json', code: json({ mcpServers: { humcase: { command: exePath, args: ['mcp'] } } }) };
    case 'opencode':
      return { file: '~/.config/opencode/opencode.json', code: json({ $schema: 'https://opencode.ai/config.json', mcp: { humcase: { type: 'local', command: [exePath, 'mcp'], enabled: true } } }) };
  }
}
