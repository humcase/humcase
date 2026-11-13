# Connect an agent to Humcase

Humcase's MCP server is the desktop executable started with one argument: `Humcase.exe mcp`
(stdio, JSON-RPC). Every client launches its own copy; they all read and write the same database
in `%USERPROFILE%\.humcase`, so the app and every agent see the same cases. Declare the server
under the key `humcase` in each client so tool names and skills stay the same everywhere.

In the snippets below, replace `C:\path\to\Humcase.exe` with the full path of your executable.
Settings → Agents in the app shows the same snippets with that path already written in, and says
which agent connected last.

## Claude Code: the plugin

The plugin `plugins/humcase` declares the server and adds four skills:

- `resume-work`: reload the brief, the reworks and the cases in progress, then say where things stand;
- `write-note`: write a case faithful to what was said, in the frame of the line of work;
- `organize-cases`: check for duplicates, file each case inside its subject as a sub-case, tidy
  and archive only after proposing the plan;
- `deliver-results`: resolve only after verification with a report, post progress reports, and
  handle the reworks the person asks for.

```bash
claude plugin marketplace add humcase/humcase
claude plugin install humcase@humcase
```

The executable path is asked when the plugin is enabled (`/config` shows it afterwards). The
plugin carries no version number, so each commit to the repository is a new version:
`claude plugin marketplace update humcase` then `claude plugin update humcase@humcase` fetch the
latest skills, or turn on auto-update for the `humcase` marketplace in `/plugin` → Marketplaces.

Tools started by the plugin are named `mcp__plugin_humcase_humcase__<tool>`; a server declared by
hand under `humcase` gives `mcp__humcase__<tool>`. The skills accept both.

## Claude desktop app

The desktop app does not start plugin MCP servers. Declare the server once at user scope:

```bash
claude mcp add --scope user humcase -- "C:\path\to\Humcase.exe" mcp
```

## Codex

Add the server to `~/.codex/config.toml` (single quotes keep the backslashes literal):

```toml
[mcp_servers.humcase]
command = 'C:\path\to\Humcase.exe'
args = ["mcp"]
```

For the skills, add the repository as a plugin marketplace and install `humcase` from Codex's
plugin list:

```bash
codex plugin marketplace add humcase/humcase
```

The Codex package (`plugins/humcase/.codex-plugin/plugin.json`) ships the same skills but no
server: a Codex plugin cannot ask for the executable's path, so the server stays in
`config.toml`.

## Cursor

In `~/.cursor/mcp.json` (or `.cursor/mcp.json` in a project), as Cursor's documentation writes it:

```json
{
  "mcpServers": {
    "humcase": {
      "command": "C:\\path\\to\\Humcase.exe",
      "args": ["mcp"]
    }
  }
}
```

## opencode

In `opencode.json` (global `~/.config/opencode/opencode.json` or the project's), servers sit
directly under `mcp`:

```json
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "humcase": {
      "type": "local",
      "command": ["C:\\path\\to\\Humcase.exe", "mcp"],
      "enabled": true
    }
  }
}
```

## Cursor and other MCP clients

Most clients take the usual `mcpServers` shape, for example `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "humcase": {
      "command": "C:\\path\\to\\Humcase.exe",
      "args": ["mcp"]
    }
  }
}
```

Clients without the skills still receive the server's short writing guidelines at connection.

## Reminders to track the work (hooks)

Skills and server instructions are guidance an agent can forget. Hooks make Humcase speak at
fixed moments of the session, through `Humcase.exe hook <event>`:

- `session-start`: the tracking rules, and on a new session the brief (`get_handoff`);
- `post-tool`: a file edit is noted; after `git commit`, `git merge` or `gh pr merge`, a
  reminder to resolve the case with a report once the result is verified;
- `stop`: when the turn changed files (edit tools, or the git state of the working folder) and
  no agent or CLI wrote a case since the session started, the agent is asked once to update its
  case, or to say the work is not tracked.

Humcase never writes in the agent's place, and a hook never breaks the session: on any failure it
prints nothing. The state of each session (times and fingerprints, no content) sits in
`%USERPROFILE%\.humcase\agent-sessions` and is removed after 14 days.

Claude Code, CLI and desktop app: add this to `~/.claude/settings.json` (merge the `hooks` key
with yours). The hooks are declared at user scope, like the server above, because the desktop app
does not fill the plugin's executable path.

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [{ "type": "command", "command": "C:\\path\\to\\Humcase.exe", "args": ["hook", "session-start"], "timeout": 20 }] }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write|MultiEdit|NotebookEdit|Bash|PowerShell",
        "hooks": [{ "type": "command", "command": "C:\\path\\to\\Humcase.exe", "args": ["hook", "post-tool"], "timeout": 10 }]
      }
    ],
    "Stop": [
      { "hooks": [{ "type": "command", "command": "C:\\path\\to\\Humcase.exe", "args": ["hook", "stop"], "timeout": 30 }] }
    ]
  }
}
```

Codex: the same three events in `~/.codex/hooks.json`, with `"command":
"\"C:\\path\\to\\Humcase.exe\" hook session-start"` (then `post-tool` with the matcher
`apply_patch|Bash`, and `stop`). Codex reads the same answers (`additionalContext`, `decision:
block`). Not tested yet: Codex is not installed on the maintainers' machine.

## Check the connection

- From a terminal: `Humcase.exe tools` lists the tools; `Humcase.exe handoff` prints the brief.
- From the agent: ask it to call `get_handoff`. Every write is recorded in the case's journal
  under the client's name (Claude Code, Codex...).
- Without MCP, every tool runs from the CLI with the same safeguards:
  `Humcase.exe call <tool> --args '<json>'`. Set `HUMCASE_ACTOR` to name the writer.
- In PowerShell, pipe the output (`Humcase.exe handoff | Out-String`, `| Out-File`): PowerShell
  does not wait for a Windows program built as a window application when its output is
  redirected with `>` or assigned to a variable, and the output is lost. Bash and agent clients
  are not affected.

The tools, their arguments and the permissions (such as agent editing) are described in
[MCP_SPEC](MCP_SPEC.md).
