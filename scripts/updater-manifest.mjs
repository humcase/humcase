// Update manifest (CI): writes the `latest.json` that installed copies of Humcase read at
// startup (endpoint in tauri.conf.json: releases/latest/download/latest.json).
//
//   node scripts/updater-manifest.mjs <tag> <notes-file> <bundle-dir> <out-file>
//
// <bundle-dir> holds the NSIS installer and its `.sig`, made by `tauri build` with
// `createUpdaterArtifacts` and the signing key. The installer is downloaded from this release
// by its own name; the signature is the content of the `.sig` file, never a path or URL.
// Fails explicitly (code 1) when the installer, its signature or the version is missing.
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [tag, notesPath, bundleDir, outPath] = process.argv.slice(2);
if (!tag || !notesPath || !bundleDir || !outPath) {
  console.error('Usage: node scripts/updater-manifest.mjs <tag> <notes-file> <bundle-dir> <out-file>');
  process.exit(1);
}
const fail = message => {
  console.error(`FAILED: ${message}`);
  process.exit(1);
};

const version = tag.replace(/^v/, '');
if (!/^\d+\.\d+\.\d+$/.test(version)) fail(`tag "${tag}" is not v<major>.<minor>.<patch>.`);

const installers = readdirSync(bundleDir).filter(name => name.endsWith('-setup.exe'));
if (installers.length !== 1) fail(`expected one NSIS installer in ${bundleDir}, found ${installers.length}.`);
const installer = installers[0];
if (!installer.includes(`_${version}_`)) fail(`installer ${installer} does not carry version ${version}.`);

let signature;
try {
  signature = readFileSync(join(bundleDir, `${installer}.sig`), 'utf8').trim();
} catch {
  fail(`signature ${installer}.sig missing: is TAURI_SIGNING_PRIVATE_KEY set?`);
}
// The updater refuses a signature that does not name the version it was signed for
// (`requireSignedVersion`): check it here rather than on people's computers.
const comment = Buffer.from(signature, 'base64').toString('utf8');
if (!comment.includes(`\tversion:${version}`)) fail(`signature of ${installer} does not carry version ${version}.`);

const manifest = {
  version,
  notes: readFileSync(notesPath, 'utf8').trim(),
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/humcase/humcase/releases/download/${tag}/${encodeURIComponent(installer)}`,
    },
  },
};
writeFileSync(outPath, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`OK: ${outPath} announces ${version} (${installer}).`);
