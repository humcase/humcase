// Release preparation (CI): checks that a published version matches the CHANGELOG and the
// version files, then writes the release notes taken from the CHANGELOG entry.
//
//   node scripts/release-check.mjs <tag> <notes-file>
//
// The tag must be `v` + the latest dated `## [x.y.z]` version of the CHANGELOG; Cargo.toml,
// tauri.conf.json and frontend/package.json must carry that same version (aligned by
// `npm run update-exe`). Fails explicitly (code 1) otherwise.
import { readFileSync, writeFileSync } from 'node:fs';

const [tag, notesPath] = process.argv.slice(2);
if (!tag || !notesPath) {
  console.error('Usage: node scripts/release-check.mjs <tag> <notes-file>');
  process.exit(1);
}

const fail = message => {
  console.error(`FAILED: ${message}`);
  process.exit(1);
};

const changelog = readFileSync('docs/CHANGELOG.md', 'utf8');
const heading = changelog.match(/^##\s*\[(\d+\.\d+\.\d+)\][^\n]*$/m);
if (!heading) fail('no "## [x.y.z]" version in docs/CHANGELOG.md.');
const version = heading[1];

if (tag !== `v${version}`) {
  fail(`tag "${tag}" does not match the latest CHANGELOG version (v${version}).`);
}

const files = [
  ['src-tauri/Cargo.toml', /^\[package\][\s\S]*?^version\s*=\s*"([^"]*)"/m],
  ['src-tauri/tauri.conf.json', /"version"\s*:\s*"([^"]*)"/],
  ['frontend/package.json', /"version"\s*:\s*"([^"]*)"/],
];
for (const [path, regex] of files) {
  const found = readFileSync(path, 'utf8').match(regex)?.[1];
  if (found !== version) {
    fail(`${path} has version "${found ?? '?'}" instead of ${version} (run npm run update-exe).`);
  }
}

// Notes: the CHANGELOG entry, from its heading to the next level-2 heading.
const start = heading.index + heading[0].length;
const next = changelog.slice(start).search(/^##\s/m);
const notes = (next === -1 ? changelog.slice(start) : changelog.slice(start, start + next)).trim();
writeFileSync(notesPath, `${notes}\n`);
console.log(`OK: ${tag} matches the CHANGELOG and the version files; notes written to ${notesPath}.`);
