// POST /api/subscribe {email, lang, website}: sends the confirmation email. Nothing is stored until the
// person clicks the link (confirm.js). The answer is the same whether or not the address is known.
import { FROM, isEmail, langOf, makeToken, confirmationMail, resend } from '../../functions-lib/news.js';

/** Pages that may post here: the site itself, its preview deployments and local development. */
const allowedOrigin = origin => /^https:\/\/((www\.)?humcase\.com|[a-z0-9-]+\.humcase-site\.pages\.dev)$/.test(origin) || /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?$/.test(origin);

const json = (status, body) => new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' } });

export async function onRequestPost({ request, env }) {
  if (!allowedOrigin(request.headers.get('Origin') || '')) return json(403, { error: 'origin' });
  let body;
  try { body = await request.json(); } catch { return json(400, { error: 'invalid' }); }
  // A field hidden from people: robots fill it in, and get a polite answer with no email sent.
  if (body.website) return json(200, { ok: true });
  const email = String(body.email || '').trim().toLowerCase();
  if (!isEmail(email)) return json(400, { error: 'email' });
  const lang = langOf(body.lang);
  const token = await makeToken(env.CONFIRM_SECRET, email, lang);
  // The link returns to the deployment that sent it (humcase.com, or a preview while testing).
  const link = `${new URL(request.url).origin}/api/confirm?t=${encodeURIComponent(token)}`;
  const mail = confirmationMail(lang, link);
  const sent = await resend(env.RESEND_API_KEY, 'POST', '/emails', { from: FROM, to: [email], subject: mail.subject, html: mail.html, text: mail.text, tags: [{ name: 'kind', value: 'news-confirm' }] });
  if (!sent.ok) {
    console.error('resend /emails failed', sent.status, sent.data && sent.data.name);
    return json(502, { error: 'send' });
  }
  return json(200, { ok: true });
}

