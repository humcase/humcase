// Drives the installed Humcase window over CDP like a first-time user and records what it sees.
// Usage: node ui.mjs <outDir> <stepName> [script.json]
import { chromium } from 'playwright-core';
import fs from 'node:fs';
import path from 'node:path';

const [outDir, stepName, scriptFile] = process.argv.slice(2);
fs.mkdirSync(outDir, { recursive: true });
const browser = await chromium.connectOverCDP('http://127.0.0.1:9222');
const page = browser.contexts()[0].pages().find((p) => !p.url().startsWith('devtools')) ?? browser.contexts()[0].pages()[0];
const errors = [];
page.on('console', (m) => { if (m.type() === 'error' || m.type() === 'warning') errors.push(`${m.type()}: ${m.text()}`); });
page.on('pageerror', (e) => errors.push(`pageerror: ${e.message}`));

let n = 0;
async function snap(label) {
  n += 1;
  const base = path.join(outDir, `${stepName}-${String(n).padStart(2, '0')}-${label}`);
  const cdp = await page.context().newCDPSession(page);
  const { data } = await cdp.send('Page.captureScreenshot', { format: 'png' });
  fs.writeFileSync(`${base}.png`, Buffer.from(data, 'base64'));
  const text = await page.evaluate(() => document.body.innerText);
  const controls = await page.evaluate(() => [...document.querySelectorAll('button, a, input, textarea, [role=button], [role=tab], [role=menuitem], [contenteditable=true]')]
    .filter((e) => e.offsetParent !== null)
    .map((e) => `${e.tagName.toLowerCase()}${e.getAttribute('role') ? `[${e.getAttribute('role')}]` : ''} "${(e.getAttribute('aria-label') || e.innerText || e.getAttribute('placeholder') || e.getAttribute('title') || '').trim().replace(/\s+/g, ' ').slice(0, 80)}"`));
  fs.writeFileSync(`${base}.txt`, `URL ${page.url()}\nSIZE ${JSON.stringify(await page.evaluate(() => [innerWidth, innerHeight, devicePixelRatio]))}\n\n=== TEXT ===\n${text}\n\n=== CONTROLS ===\n${controls.join('\n')}\n`);
  console.log(`snap ${base}`);
}

await page.waitForLoadState('domcontentloaded');
await page.waitForTimeout(2000);
const steps = scriptFile ? JSON.parse(fs.readFileSync(scriptFile, 'utf8')) : [{ snap: 'screen' }];
for (const s of steps) {
  try {
    if (s.click) await page.getByText(s.click, { exact: s.exact ?? false }).first().click({ timeout: 8000 });
    if (s.role) await page.getByRole(s.role, { name: s.name, exact: s.exact ?? false }).first().click({ timeout: 8000 });
    if (s.css) await page.locator(s.css).first().click({ timeout: 8000 });
    if (s.type) await page.keyboard.type(s.type, { delay: 20 });
    if (s.press) await page.keyboard.press(s.press);
    if (s.reload) await page.reload();
    await page.waitForTimeout(s.wait ?? 1200);
    if (s.snap) await snap(s.snap);
  } catch (e) {
    errors.push(`step ${JSON.stringify(s)} failed: ${e.message.split('\n')[0]}`);
    await snap('failed');
  }
}
fs.writeFileSync(path.join(outDir, `${stepName}-console.txt`), errors.join('\n') || '(no console errors or warnings)');
await browser.close().catch(() => {});
