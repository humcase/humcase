# Security policy

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Report it privately through GitHub: **Security → Report a vulnerability** on this repository. You can also write to **security@humcase.com**.

Include what you found, how to reproduce it, the Humcase version and your operating system. We acknowledge reports within 7 days and keep you informed until a fix is released. We credit reporters who wish to be named.

## Supported versions

Only the latest release receives security fixes.

## Scope

Humcase is a local-first desktop app. Of particular interest:

- the local SQLite database and backup archives (`.humcase.zip`);
- the MCP server (`Humcase.exe mcp`) and the CLI, which let agents read and write notes;
- storage of AI provider keys (`ai-config.json`);
- content sent to AI or dictation providers chosen by the user.
