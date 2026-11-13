// Talks to the installed `Humcase.exe mcp` the way an agent client does, and records the exchange.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
const [exe, outFile] = process.argv.slice(2);
const child = spawn(exe, ['mcp'], { stdio: ['pipe', 'pipe', 'pipe'] });
let buf = ''; const waiting = new Map(); const log = [];
child.stdout.on('data', (d) => { buf += d; let i; while ((i = buf.indexOf('\n')) >= 0) { const line = buf.slice(0, i).trim(); buf = buf.slice(i + 1); if (!line) continue; const msg = JSON.parse(line); log.push({ in: msg }); waiting.get(msg.id)?.(msg); } });
let stderr = ''; child.stderr.on('data', (d) => { stderr += d; });
let id = 0;
const rpc = (method, params) => new Promise((res, rej) => { const m = { jsonrpc: '2.0', id: ++id, method, params }; log.push({ out: m }); waiting.set(m.id, res); child.stdin.write(JSON.stringify(m) + '\n'); setTimeout(() => rej(new Error(`timeout ${method}`)), 20000); });
const call = async (name, args) => { const r = await rpc('tools/call', { name, arguments: args }); const t = r.result?.content?.[0]?.text; return { error: r.error ?? (r.result?.isError ? t : undefined), data: (() => { try { return JSON.parse(t); } catch { return t; } })() }; };
const report = {};
try {
  const init = await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'new-user-test', version: '1.0' } });
  report.server = init.result?.serverInfo; report.instructions = init.result?.instructions?.slice(0, 400);
  child.stdin.write(JSON.stringify({ jsonrpc: '2.0', method: 'notifications/initialized' }) + '\n');
  const tools = await rpc('tools/list', {});
  report.toolCount = tools.result?.tools?.length; report.tools = tools.result?.tools?.map((t) => t.name);
  report.handoffBefore = await call('get_handoff', {});
  const inbox = await call('get_or_create_inbox', { space: 'work' });
  report.inbox = inbox;
  const pid = inbox.data?.id ?? inbox.data?.project?.id;
  report.added = await call('add_note', { project_id: pid, title: 'Compare three venues for the kickoff', content: 'Capacity, price, access by train.', note_type: 'research' });
  const nid = report.added.data?.id;
  report.status = await call('update_note_status', { note_id: nid, status: 'in_progress' });
  report.resolved = await call('update_note_status', { note_id: nid, status: 'resolved', resolution_comment: 'Venue B fits: 40 seats, 900 EUR, 5 min from the station. A is too small, C has no train access.' });
  report.handoffAfter = await call('get_handoff', { project_id: pid });
  report.forbiddenEdit = await call('update_note', { note_id: nid, expected_revision: 1, title: 'x' });
} catch (e) { report.failure = e.message; }
report.stderr = stderr.slice(0, 2000);
child.kill();
fs.writeFileSync(outFile, JSON.stringify({ report, log }, null, 2));
console.log(JSON.stringify(report, null, 2).slice(0, 4000));
