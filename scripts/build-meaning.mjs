// Builds the folder `meaning/` that ships next to Humcase.exe (installers and `npm run update-exe`):
// the meaning worker (`sense/`), Microsoft's official ONNX Runtime, the Granite model, the Visual
// C++ runtime it needs, and their licenses. Every download is pinned and checked against its
// SHA256; a file already in `sense/vendor/` with the right fingerprint is not downloaded again.
// Fails explicitly (exit code 1) at the first problem, then checks that the worker answers.
// The folder sits next to the delivered Humcase.exe at the repository root, and installers carry
// it through `tauri build --config src-tauri/tauri.meaning.conf.json` (kept out of
// `tauri.conf.json` so that plain cargo builds and tests do not need it).
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';

const VENDOR = 'sense/vendor';
const OUT = 'meaning';

// ONNX Runtime by Microsoft (Windows 10 1809+ and Windows 11). Its version must match the one
// the `ort` crate of `sense/Cargo.toml` is written for (ort 2.0.0-rc.13: ONNX Runtime 1.28).
const RUNTIME = {
  version: '1.28.3',
  url: 'https://github.com/microsoft/onnxruntime/releases/download/v1.28.3/onnxruntime-win-x64-1.28.3.zip',
  sha256: '1d6fab48e85f948436af7c8c971d2c145cf224e2c444755dec894f8b0de11a83',
};
// Granite Embedding 97M multilingual R2 by IBM (Apache-2.0), at a fixed revision.
const MODEL_REPO = 'ibm-granite/granite-embedding-97m-multilingual-r2';
const MODEL_REVISION = '835ad14087e140460703cf0fae09f97d469d65c2';
const MODEL_FILES = [
  { path: 'onnx/model_quint8_avx2.onnx', to: 'model.onnx', sha256: 'a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e' },
  { path: 'tokenizer.json', to: 'tokenizer.json', sha256: '4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f' },
];
// Visual C++ runtime DLLs ONNX Runtime loads, shipped next to the worker (app-local deployment).
const VC_RUNTIME = ['msvcp140.dll', 'msvcp140_1.dll', 'vcruntime140.dll', 'vcruntime140_1.dll'];

const fail = message => {
  console.error(`\nFAILED: ${message}`);
  process.exit(1);
};
const sha256 = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const run = (command, args, options = {}) => {
  const result = spawnSync(command, args, { stdio: 'inherit', ...options });
  if (result.status !== 0) fail(`${command} ${args.join(' ')} (code ${result.status})`);
};

/** Downloads `url` to `path` unless a file with the expected fingerprint is already there. */
async function fetchChecked(url, path, expected) {
  if (existsSync(path) && sha256(path) === expected) return;
  console.log(`   downloading ${url}`);
  const response = await fetch(url);
  if (!response.ok) fail(`download ${url}: HTTP ${response.status}`);
  writeFileSync(path, Buffer.from(await response.arrayBuffer()));
  const actual = sha256(path);
  if (actual !== expected) {
    rmSync(path);
    fail(`${url} has fingerprint ${actual}, expected ${expected}: refused.`);
  }
}

/** Folder of the Visual C++ runtime DLLs of the installed Visual Studio build tools. */
function vcRuntimeDir() {
  const vswhere = join(process.env['ProgramFiles(x86)'] ?? 'C:/Program Files (x86)', 'Microsoft Visual Studio/Installer/vswhere.exe');
  if (!existsSync(vswhere)) fail('vswhere.exe not found: Visual Studio build tools are required.');
  const found = spawnSync(vswhere, ['-latest', '-products', '*', '-find', 'VC/Redist/MSVC/*/x64/Microsoft.VC14*.CRT'], { encoding: 'utf8' });
  const dirs = (found.stdout ?? '').split(/\r?\n/).filter(Boolean).sort();
  const dir = dirs.at(-1);
  if (!dir) fail('Visual C++ runtime (VC/Redist/MSVC/*/x64/Microsoft.VC14*.CRT) not found.');
  return dir;
}

console.log('== 1/4 Meaning worker (sense/) ==');
run('cargo', ['build', '--release', '--locked', '--manifest-path', 'sense/Cargo.toml']);

console.log('\n== 2/4 Runtime and model (pinned, checked) ==');
mkdirSync(join(VENDOR, 'model'), { recursive: true });
const runtimeZip = join(VENDOR, `onnxruntime-win-x64-${RUNTIME.version}.zip`);
await fetchChecked(RUNTIME.url, runtimeZip, RUNTIME.sha256);
const runtimeDir = join(VENDOR, `onnxruntime-win-x64-${RUNTIME.version}`);
if (!existsSync(join(runtimeDir, 'lib', 'onnxruntime.dll'))) run('tar', ['-xf', runtimeZip, '-C', VENDOR]);
for (const file of MODEL_FILES) {
  await fetchChecked(`https://huggingface.co/${MODEL_REPO}/resolve/${MODEL_REVISION}/${file.path}`, join(VENDOR, 'model', file.to), file.sha256);
}

console.log(`\n== 3/4 Assembling ${OUT}/ ==`);
// Only changed files are written, so a worker still running from `meaning/` (it stops after a
// minute without work) does not block a rebuild that changes nothing it holds.
const vc = vcRuntimeDir();
const wanted = new Map([
  ['humcase-sense.exe', 'sense/target/release/humcase-sense.exe'],
  ['onnxruntime.dll', join(runtimeDir, 'lib', 'onnxruntime.dll')],
  ...MODEL_FILES.map(file => [`model/${file.to}`, join(VENDOR, 'model', file.to)]),
  ...VC_RUNTIME.map(dll => [dll, join(vc, dll)]),
  ...readdirSync(runtimeDir)
    .filter(name => /^(LICENSE|ThirdPartyNotices)/i.test(name))
    .map(name => [`licenses/onnxruntime-${name}`, join(runtimeDir, name)]),
]);
const NOTICE = 'licenses/NOTICE.txt';
const listFiles = (dir, prefix = '') =>
  existsSync(dir)
    ? readdirSync(dir, { withFileTypes: true }).flatMap(entry =>
        entry.isDirectory() ? listFiles(join(dir, entry.name), `${prefix}${entry.name}/`) : [`${prefix}${entry.name}`])
    : [];
for (const stale of listFiles(OUT).filter(name => !wanted.has(name) && name !== NOTICE)) rmSync(join(OUT, stale));
for (const [name, source] of wanted) {
  const dest = join(OUT, name);
  mkdirSync(dirname(dest), { recursive: true });
  if (existsSync(dest) && statSync(dest).size === statSync(source).size && sha256(dest) === sha256(source)) continue;
  try {
    copyFileSync(source, dest);
  } catch (err) {
    fail(`cannot write ${dest} (Humcase or its meaning worker running? close it and rerun): ${err.message}`);
  }
}
writeFileSync(join(OUT, NOTICE), [
  "This folder holds Humcase's meaning worker and what it runs:",
  `- ONNX Runtime ${RUNTIME.version}, (c) Microsoft Corporation, MIT License (onnxruntime-LICENSE, onnxruntime-ThirdPartyNotices.txt), ${RUNTIME.url}`,
  `- Granite Embedding 97M multilingual R2, (c) IBM, Apache License 2.0, https://huggingface.co/${MODEL_REPO} (revision ${MODEL_REVISION})`,
  '- Microsoft Visual C++ runtime, redistributable files of Microsoft Visual Studio.',
  '',
].join('\n'));

console.log('\n== 4/4 Check: the worker answers ==');
const check = spawnSync(join(OUT, 'humcase-sense.exe'), ['--idle', '5'], { input: '{"id":1,"embed":["Humcase"]}\n', encoding: 'utf8' });
const lines = (check.stdout ?? '').trim().split('\n').map(line => { try { return JSON.parse(line); } catch { return null; } });
if (!lines[0]?.ready || lines[1]?.vectors?.[0]?.length !== lines[0].dim) {
  fail(`the worker does not answer: ${check.stdout || check.stderr || `code ${check.status}`}`);
}
console.log(`OK: ${OUT}/ ready (${lines[0].model}, ${lines[0].dim} dimensions).`);
