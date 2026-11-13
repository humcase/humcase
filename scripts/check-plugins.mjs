// Checks the agent packaging under `plugins/` and the marketplace at the repository root.
// Run by CI (`ci.yml`, job `plugins`) and by hand with `npm run check-plugins`.
//
// What it guards:
//   - every manifest and MCP file parses as JSON;
//   - no plugin manifest nor marketplace entry pins a `version`: a pinned version keeps every
//     installed copy on its cache until the string changes, so a skill change would never reach
//     users (Claude Code plugin docs, "Versions and updates"). Without it, installs follow the
//     git commit;
//   - every `${user_config.KEY}` used by the plugin's MCP file is declared in `userConfig`;
//   - every skill: `name` equal to its folder (lowercase, digits, hyphens, at most 64
//     characters), a `description` of at most 1,024 characters without XML tags, a body under
//     500 lines, and relative links that point to existing files.
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const MARKETPLACE = '.claude-plugin/marketplace.json';
const errors = [];
const fail = message => errors.push(message);

function readJson(path) {
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (err) {
    fail(`${path}: not valid JSON (${err.message})`);
    return null;
  }
}

function checkNoVersion(path, object) {
  if (object && Object.hasOwn(object, 'version')) {
    fail(
      `${path}: remove "version". A pinned version freezes installed copies until it changes; ` +
        'without it, installs follow the git commit.'
    );
  }
}

/** Splits `---` YAML front matter from the body; only the flat `key: value` lines are read. */
function frontMatter(path, text) {
  const match = text.match(/^---\r?\n([\s\S]*?)\r?\n---\r?\n?([\s\S]*)$/);
  if (!match) {
    fail(`${path}: missing --- front matter ---`);
    return null;
  }
  const fields = {};
  for (const line of match[1].split(/\r?\n/)) {
    const field = line.match(/^([A-Za-z_-]+):\s*(.*)$/);
    if (!field) continue;
    const value = field[2];
    // A plain YAML scalar ends at ": " or " #": quote such a value or the loader truncates it.
    if (!/^['"]/.test(value) && /: | #/.test(value)) {
      fail(`${path}: "${field[1]}" contains ": " or " #" without quotes (invalid plain YAML)`);
    }
    fields[field[1]] = value.replace(/^(['"])(.*)\1$/, '$2');
  }
  return { fields, body: match[2] };
}

function checkSkill(skillDir) {
  const name = skillDir.split('/').pop();
  const path = join(skillDir, 'SKILL.md').replaceAll('\\', '/');
  if (!existsSync(path)) {
    fail(`${skillDir}: no SKILL.md`);
    return;
  }
  const parsed = frontMatter(path, readFileSync(path, 'utf8'));
  if (!parsed) return;
  const { fields, body } = parsed;
  if (fields.name !== name) fail(`${path}: name "${fields.name}" differs from its folder "${name}"`);
  if (!/^[a-z0-9-]{1,64}$/.test(name)) fail(`${path}: name must be 1-64 lowercase letters, digits or hyphens`);
  const description = fields.description ?? '';
  if (!description) fail(`${path}: empty description`);
  if (description.length > 1024) fail(`${path}: description has ${description.length} characters (max 1024)`);
  if (/<[a-zA-Z/][^>]*>/.test(description)) fail(`${path}: description must not contain XML tags`);
  const lines = body.split(/\r?\n/).length;
  if (lines >= 500) fail(`${path}: body has ${lines} lines (keep it under 500)`);
  for (const [, target] of body.matchAll(/\]\(([^)\s]+)\)/g)) {
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith('#')) continue;
    const file = join(dirname(path), target.split('#')[0]);
    if (!existsSync(file)) fail(`${path}: link to a missing file (${target})`);
  }
}

function checkPlugin(pluginDir) {
  const manifestPath = `${pluginDir}/.claude-plugin/plugin.json`;
  const manifest = readJson(manifestPath);
  checkNoVersion(manifestPath, manifest);

  for (const client of ['.codex-plugin', '.cursor-plugin']) {
    const path = `${pluginDir}/${client}/plugin.json`;
    if (!existsSync(path)) continue;
    const other = readJson(path);
    checkNoVersion(path, other);
    if (other && manifest && other.name !== manifest.name) {
      fail(`${path}: name "${other.name}" differs from the Claude Code manifest ("${manifest.name}")`);
    }
    for (const key of ['skills', 'mcpServers']) {
      const target = other?.[key];
      if (typeof target === 'string' && !existsSync(join(pluginDir, target))) {
        fail(`${path}: "${key}" points to a missing path (${target})`);
      }
      if (key === 'mcpServers' && typeof target === 'string' && target.endsWith('.json')) {
        readJson(join(pluginDir, target).replaceAll('\\', '/'));
      }
    }
  }

  const mcpPath = `${pluginDir}/.mcp.json`;
  if (existsSync(mcpPath)) {
    readJson(mcpPath);
    const declared = new Set(Object.keys(manifest?.userConfig ?? {}));
    for (const [, key] of readFileSync(mcpPath, 'utf8').matchAll(/\$\{user_config\.([A-Za-z0-9_]+)\}/g)) {
      if (!declared.has(key)) fail(`${mcpPath}: \${user_config.${key}} is not declared in userConfig`);
    }
  }

  const skillsDir = `${pluginDir}/skills`;
  if (existsSync(skillsDir)) {
    for (const entry of readdirSync(skillsDir, { withFileTypes: true })) {
      if (entry.isDirectory()) checkSkill(`${skillsDir}/${entry.name}`);
    }
  }
}

const marketplace = readJson(MARKETPLACE);
checkNoVersion(MARKETPLACE, marketplace);
for (const entry of marketplace?.plugins ?? []) {
  checkNoVersion(`${MARKETPLACE} (plugin "${entry.name}")`, entry);
  if (typeof entry.source !== 'string' || !entry.source.startsWith('./')) {
    fail(`${MARKETPLACE}: plugin "${entry.name}" should use a relative "./" source`);
    continue;
  }
  const pluginDir = entry.source.slice(2).replace(/\/$/, '');
  if (!existsSync(`${pluginDir}/.claude-plugin/plugin.json`)) {
    fail(`${MARKETPLACE}: plugin "${entry.name}" has no ${pluginDir}/.claude-plugin/plugin.json`);
    continue;
  }
  checkPlugin(pluginDir);
}

if (errors.length) {
  console.error(`Plugin packaging check FAILED (${errors.length}):`);
  for (const message of errors) console.error(`  - ${message}`);
  process.exit(1);
}
console.log('Plugin packaging check passed.');
