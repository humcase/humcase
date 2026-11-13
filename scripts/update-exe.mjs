// Delivery: syncs the version, builds the release, then replaces Humcase.exe and really checks
// the replacement.
//
// Three distinct steps, each failing explicitly:
//   0. version sync: the reference version is the latest `## [x.y.z]` entry of the CHANGELOG;
//      it is propagated to `Cargo.toml`, `tauri.conf.json`, `frontend/package.json` and the
//      two version fields of `frontend/package-lock.json` (and checked afterwards), so an
//      executable always matches a CHANGELOG entry. The plugin manifests under `plugins/` carry
//      no version on purpose: installs then follow the git commit, so a skill change reaches
//      users without waiting for a release (`scripts/check-plugins.mjs` keeps it that way);
//   1. build: `meaning/` (meaning worker, ONNX Runtime, model: scripts/build-meaning.mjs), next to
//      Humcase.exe, then `tauri build` with it as an installer resource; if either fails, no
//      executable is touched;
//   2. replacement + fingerprint check: if an executable is locked (interface or MCP server
//      open), it stops with a clear message instead of a misleading success.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, readdirSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const SOURCE = 'src-tauri/target/release/humcase.exe';
// Kept as a list so several copies can be delivered; a single one since 0.9.74.
const DESTS = ['Humcase.exe'];
const NAMES = DESTS.join(' and ');
const CHANGELOG = 'docs/CHANGELOG.md';

const sha256 = path => createHash('sha256').update(readFileSync(path)).digest('hex');

/** Latest dated version of the CHANGELOG (`## [x.y.z] - date`). */
function latestChangelogVersion() {
  const changelog = readFileSync(CHANGELOG, 'utf8');
  const m = changelog.match(/^##\s*\[(\d+\.\d+\.\d+)\]/m);
  if (!m) {
    console.error(`\nFAILED: no "## [x.y.z]" version found in ${CHANGELOG}.`);
    process.exit(1);
  }
  return m[1];
}

/** Files carrying a version to align with the CHANGELOG. */
const VERSION_TARGETS = [
  { path: 'src-tauri/Cargo.toml', regex: /(^\[package\][\s\S]*?^version\s*=\s*)"([^"]*)"/m },
  { path: 'src-tauri/tauri.conf.json', regex: /("version"\s*:\s*)"([^"]*)"/ },
  { path: 'frontend/package.json', regex: /("version"\s*:\s*)"([^"]*)"/ },
  { path: 'frontend/package-lock.json', regex: /(^\{\s*"name"\s*:\s*"frontend"\s*,\s*"version"\s*:\s*)"([^"]*)"/m },
  { path: 'frontend/package-lock.json', regex: /("packages"\s*:\s*\{\s*""\s*:\s*\{\s*"name"\s*:\s*"frontend"\s*,\s*"version"\s*:\s*)"([^"]*)"/ },
];

function syncVersion(version) {
  for (const { path, regex } of VERSION_TARGETS) {
    const content = readFileSync(path, 'utf8');
    if (!regex.test(content)) {
      console.error(`\nFAILED: cannot find the version in ${path}.`);
      process.exit(1);
    }
    const updated = content.replace(regex, `$1"${version}"`);
    if (updated !== content) {
      writeFileSync(path, updated);
      console.log(`   ${path} → ${version}`);
    }
  }
}

function checkVersion(version) {
  for (const { path, regex } of VERSION_TARGETS) {
    const content = readFileSync(path, 'utf8');
    const found = content.match(regex);
    if (!found || found[2] !== version) {
      console.error(
        `\nFAILED: ${path} has version "${found?.[2] ?? '?'}" instead of the CHANGELOG's ` +
          `(${version}). Refusing to deliver a binary with an inconsistent version.`
      );
      process.exit(1);
    }
  }
}

console.log('== 0/3 Version sync (reference: CHANGELOG) ==');
const version = latestChangelogVersion();
console.log(`   reference version: ${version}`);
syncVersion(version);
checkVersion(version);

console.log('\n== 1/3 Release build (meaning/, then tauri build) ==');
const meaning = spawnSync(process.execPath, ['scripts/build-meaning.mjs'], { stdio: 'inherit' });
if (meaning.status !== 0) {
  console.error(`\nFAILED: meaning/ could not be built (code ${meaning.status}). No executable was modified.`);
  process.exit(1);
}
// `shell: true` + an argument array triggers DEP0190: go through cmd.exe explicitly.
const build =
  process.platform === 'win32'
    ? spawnSync('cmd.exe', ['/d', '/s', '/c', 'tauri build --config src-tauri/tauri.meaning.conf.json'], { stdio: 'inherit' })
    : spawnSync('tauri', ['build', '--config', 'src-tauri/tauri.meaning.conf.json'], { stdio: 'inherit' });
if (build.status !== 0) {
  console.error(`\nFAILED: the build failed (code ${build.status}). No executable was modified.`);
  process.exit(1);
}

if (!existsSync(SOURCE)) {
  console.error(`\nFAILED: release binary not found (${SOURCE}). No executable was modified.`);
  process.exit(1);
}

console.log(`\n== 2/3 Replacing ${NAMES} ==`);
for (const dest of DESTS) {
  try {
    copyFileSync(SOURCE, dest);
  } catch (err) {
    console.error(
      `\nFAILED: cannot replace ${dest}: the file is probably in use ` +
        '(interface or MCP server open).\n' +
        `Close every ${NAMES}, then rerun \`npm run update-exe\`.\n` +
        `Detail: ${err.message}`
    );
    process.exit(1);
  }
}

console.log('\n== 3/3 Fingerprint and version check ==');
const sourceHash = sha256(SOURCE);
for (const dest of DESTS) {
  if (sha256(dest) !== sourceHash) {
    console.error(
      `\nFAILED: ${dest} does not match the release binary (different SHA256 fingerprints): ` +
        'the replacement did not happen.'
    );
    process.exit(1);
  }
}
checkVersion(version);
console.log(`\nOK: ${NAMES} v${version} replaced and verified (SHA256 ${sourceHash}).`);

// Test cache upkeep (target/debug): it grows with every test build (121 GB on 2026-09-29). Above
// the threshold it is emptied; it only holds regenerable files, and the release, which produces
// the exe, is never touched. A failure (test app open) is only a warning: the delivery is
// already done and verified.
const DEBUG_CACHE = 'src-tauri/target/debug';
const CACHE_THRESHOLD_GB = 30;
function folderSize(folder) {
  let total = 0;
  for (const entry of readdirSync(folder, { withFileTypes: true })) {
    const entryPath = join(folder, entry.name);
    try {
      if (entry.isDirectory()) total += folderSize(entryPath);
      else if (entry.isFile()) total += statSync(entryPath).size;
    } catch {
      // File vanished or unreadable during the walk: ignored.
    }
  }
  return total;
}
if (existsSync(DEBUG_CACHE)) {
  const gb = folderSize(DEBUG_CACHE) / 1024 ** 3;
  if (gb > CACHE_THRESHOLD_GB) {
    console.log(`\nTest cache ${DEBUG_CACHE}: ${gb.toFixed(1)} GB (> ${CACHE_THRESHOLD_GB} GB), cleaning...`);
    const cleaning = spawnSync('cargo', ['clean', '--profile', 'dev'], { cwd: 'src-tauri', stdio: 'inherit' });
    if (cleaning.status !== 0) {
      console.warn('   Warning: incomplete cleanup (is a test app running?). The delivery remains valid.');
    }
  } else {
    console.log(`\nTest cache: ${gb.toFixed(1)} GB (cleanup threshold: ${CACHE_THRESHOLD_GB} GB).`);
  }
}
