// Tests of the release news guard: Cloudflare Turnstile must vouch for a sign-up before any email
// leaves (without it, a script could make the site email any address). Run: `npm test` in site/.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { verifyHuman, turnstileHostOk, TURNSTILE_ACTION } from './news.js';
import { onRequestPost } from '../functions/api/subscribe.js';

const SITEVERIFY = 'https://challenges.cloudflare.com/turnstile/v0/siteverify';
const answer = body => async () => ({ ok: true, json: async () => body });
const good = { success: true, action: TURNSTILE_ACTION, hostname: 'humcase.com' };

test('a valid token for the form, issued on the site, passes', async () => {
  assert.equal(await verifyHuman('secret', 'token', '203.0.113.1', 'humcase.com', answer(good)), true);
});

test('the check fails closed', async () => {
  const no = async (...args) => assert.equal(await verifyHuman(...args), false);
  await no('', 'token', null, 'humcase.com', answer(good)); // no secret configured
  await no('secret', '', null, 'humcase.com', answer(good)); // no token
  await no('secret', undefined, null, 'humcase.com', answer(good));
  await no('secret', 'x'.repeat(2049), null, 'humcase.com', answer(good)); // oversized token
  await no('secret', 'token', null, 'humcase.com', answer({ ...good, success: false }));
  await no('secret', 'token', null, 'humcase.com', async () => ({ ok: false, json: async () => good }));
  await no('secret', 'token', null, 'humcase.com', async () => { throw new Error('network'); });
  await no('secret', 'token', null, 'humcase.com', async () => ({ ok: true, json: async () => { throw new SyntaxError('not json'); } }));
});

test('a token for another action or another site is refused', async () => {
  assert.equal(await verifyHuman('secret', 'token', null, 'humcase.com', answer({ ...good, action: 'login' })), false);
  assert.equal(await verifyHuman('secret', 'token', null, 'humcase.com', answer({ ...good, hostname: 'example.com' })), false);
  assert.equal(await verifyHuman('secret', 'token', null, 'humcase.com', answer({ ...good, hostname: 'humcase.com.evil.example' })), false);
});

test('localhost tokens count only for a local request', async () => {
  assert.equal(await verifyHuman('secret', 'token', null, 'humcase.com', answer({ ...good, hostname: 'localhost' })), false);
  assert.equal(await verifyHuman('secret', 'token', null, 'localhost', answer({ ...good, hostname: 'localhost' })), true);
  assert.equal(turnstileHostOk('www.humcase.com', 'www.humcase.com'), true);
  assert.equal(turnstileHostOk('news-turnstile.humcase-site.pages.dev', 'news-turnstile.humcase-site.pages.dev'), true);
  assert.equal(turnstileHostOk('127.0.0.1', 'humcase.com'), false);
});

test('Siteverify receives the secret, the token and the visitor IP', async () => {
  let sent;
  await verifyHuman('secret', 'token', '203.0.113.1', 'humcase.com', async (url, init) => {
    sent = { url, form: Object.fromEntries(init.body) };
    return { ok: true, json: async () => good };
  });
  assert.equal(sent.url, SITEVERIFY);
  assert.deepEqual(sent.form, { secret: 'secret', response: 'token', remoteip: '203.0.113.1' });
});

// The endpoint itself: no email is sent unless Turnstile vouches for the visitor.
const ENV = { TURNSTILE_SECRET: 'ts', CONFIRM_SECRET: 'cs', RESEND_API_KEY: 'rk' };
const post = body => new Request('https://humcase.com/api/subscribe', {
  method: 'POST',
  headers: { Origin: 'https://humcase.com', 'Content-Type': 'application/json', 'CF-Connecting-IP': '203.0.113.1' },
  body: JSON.stringify(body),
});
async function subscribe(body, siteverify) {
  const calls = [];
  const original = globalThis.fetch;
  globalThis.fetch = async (url, init) => {
    calls.push(String(url));
    if (String(url) === SITEVERIFY) return { ok: true, json: async () => siteverify };
    return { ok: true, status: 200, json: async () => ({ id: 'email' }) };
  };
  try {
    const res = await onRequestPost({ request: post(body), env: ENV });
    return { status: res.status, emailed: calls.some(u => u.startsWith('https://api.resend.com')), calls };
  } finally {
    globalThis.fetch = original;
  }
}

test('subscribe sends nothing without a valid Turnstile token', async () => {
  for (const [body, siteverify] of [
    [{ email: 'a@example.com' }, good],
    [{ email: 'a@example.com', turnstile: 'forged' }, { success: false, 'error-codes': ['invalid-input-response'] }],
    [{ email: 'a@example.com', turnstile: 'other' }, { ...good, action: 'login' }],
  ]) {
    const r = await subscribe(body, siteverify);
    assert.equal(r.status, 403);
    assert.equal(r.emailed, false);
  }
});

test('subscribe sends the confirmation email once Turnstile vouches', async () => {
  const r = await subscribe({ email: 'a@example.com', lang: 'fr', turnstile: 'token' }, good);
  assert.equal(r.status, 200);
  assert.deepEqual(r.calls, [SITEVERIFY, 'https://api.resend.com/emails']);
});

test('robots caught by the hidden field get no email and no Siteverify call', async () => {
  const r = await subscribe({ email: 'a@example.com', website: 'spam', turnstile: 'token' }, good);
  assert.equal(r.status, 200);
  assert.deepEqual(r.calls, []);
});
