// Release news: sign-up with a confirmation email (double opt-in), shared by the Pages Functions
// `functions/api/subscribe.js` and `functions/api/confirm.js`. Secrets come from the Pages project
// (RESEND_API_KEY, CONFIRM_SECRET, TURNSTILE_SECRET), never from the repository.

export const SITE = 'https://humcase.com';
export const FROM = 'Humcase <news@humcase.com>';
/** Resend segment "Humcase release news": confirmed subscribers only. */
export const SEGMENT_ID = 'a26a4dd1-0c07-4255-9192-c71fae117f69';
/** How long a confirmation link stays valid. */
export const LINK_DAYS = 7;

export const LANGS = ['en', 'fr', 'es', 'de', 'pt-BR', 'it'];
export const langOf = l => (LANGS.includes(l) ? l : 'en');

const enc = new TextEncoder();
const b64url = bytes => btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
const unb64url = s => Uint8Array.from(atob(s.replace(/-/g, '+').replace(/_/g, '/')), c => c.charCodeAt(0));
const hmacKey = secret => crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign', 'verify']);

/** A signed token carrying the address, the language and an expiry: no database needed. */
export async function makeToken(secret, email, lang, now = Date.now()) {
  const payload = b64url(enc.encode(JSON.stringify({ e: email, l: lang, x: now + LINK_DAYS * 86400000 })));
  const sig = b64url(await crypto.subtle.sign('HMAC', await hmacKey(secret), enc.encode(payload)));
  return `${payload}.${sig}`;
}

/** The token's content if its signature is right and it has not expired, else why not. */
export async function readToken(secret, token, now = Date.now()) {
  const [payload, sig] = String(token || '').split('.');
  if (!payload || !sig) return { error: 'invalid' };
  let ok = false;
  try { ok = await crypto.subtle.verify('HMAC', await hmacKey(secret), unb64url(sig), enc.encode(payload)); } catch { ok = false; }
  if (!ok) return { error: 'invalid' };
  let data;
  try { data = JSON.parse(new TextDecoder().decode(unb64url(payload))); } catch { return { error: 'invalid' }; }
  if (typeof data.e !== 'string' || typeof data.x !== 'number') return { error: 'invalid' };
  if (data.x < now) return { error: 'expired', lang: langOf(data.l) };
  return { email: data.e, lang: langOf(data.l) };
}

/** A plausible address: one @, a dot in the domain, no spaces, at most 254 characters. */
export const isEmail = s => typeof s === 'string' && s.length <= 254 && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(s);

/** The pages of the site in each language. */
export const homeOf = lang => `${SITE}${lang === 'en' ? '/' : `/${lang.toLowerCase()}/`}`;

const MAIL = {
  en: { subject: 'Confirm your Humcase release news', line: 'Confirm to get an email when a new version of Humcase is out. Nothing else, and you can unsubscribe in one click.', button: 'Confirm', ignore: 'If you didn’t ask for this, ignore this email.' },
  fr: { subject: 'Confirmez les nouveautés Humcase', line: 'Confirmez pour recevoir un e-mail à chaque nouvelle version de Humcase. Rien d’autre, et désinscription en un clic.', button: 'Confirmer', ignore: 'Si vous n’avez rien demandé, ignorez cet e-mail.' },
  es: { subject: 'Confirma las novedades de Humcase', line: 'Confirma para recibir un correo cada vez que salga una nueva versión de Humcase. Nada más, y puedes darte de baja con un clic.', button: 'Confirmar', ignore: 'Si no lo has pedido, ignora este correo.' },
  de: { subject: 'Bestätige die Humcase-Neuigkeiten', line: 'Bestätige, um eine E-Mail zu bekommen, wenn eine neue Version von Humcase erscheint. Sonst nichts, und du kannst dich mit einem Klick abmelden.', button: 'Bestätigen', ignore: 'Wenn du das nicht angefordert hast, ignoriere diese E-Mail.' },
  'pt-BR': { subject: 'Confirme as novidades do Humcase', line: 'Confirme para receber um e-mail a cada nova versão do Humcase. Nada mais, e você pode cancelar com um clique.', button: 'Confirmar', ignore: 'Se você não pediu isso, ignore este e-mail.' },
  it: { subject: 'Conferma le novità di Humcase', line: 'Conferma per ricevere un’email a ogni nuova versione di Humcase. Nient’altro, e puoi annullare l’iscrizione con un clic.', button: 'Conferma', ignore: 'Se non l’hai richiesto, ignora questa email.' },
};

const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');

/** The confirmation email, in HTML and plain text. */
export function confirmationMail(lang, link) {
  const t = MAIL[langOf(lang)];
  const html = `<!doctype html><html lang="${langOf(lang)}"><body style="margin:0;padding:32px 16px;background:#f4f1ea;font-family:Inter,'Segoe UI',system-ui,sans-serif;color:#24221f">
<div style="max-width:520px;margin:0 auto;background:#ffffff;border-radius:14px;padding:32px 28px">
<p style="margin:0 0 24px;font:500 14px/1 'DM Mono',Consolas,monospace;letter-spacing:.16em"><span style="color:#d4581f">HUM</span>CASE</p>
<p style="margin:0 0 24px;font-size:16px;line-height:1.6">${esc(t.line)}</p>
<p style="margin:0 0 28px"><a href="${esc(link)}" style="display:inline-block;padding:12px 22px;border-radius:10px;background:#e2622a;color:#ffffff;text-decoration:none;font-weight:600">${esc(t.button)}</a></p>
<p style="margin:0;font-size:13px;line-height:1.5;color:#6f6a62">${esc(t.ignore)}</p>
</div>
<p style="max-width:520px;margin:16px auto 0;font-size:12px;color:#8a857c;text-align:center">humcase.com</p>
</body></html>`;
  const text = `${t.line}\n\n${t.button}: ${link}\n\n${t.ignore}\n\nhumcase.com`;
  return { subject: t.subject, html, text };
}

/** The Turnstile action of the sign-up form (public/js/world.js renders the widget with it). */
export const TURNSTILE_ACTION = 'news';
/** Pages whose Turnstile tokens the site accepts: production, its preview deployments, and local
 *  development only when the request itself is local (a production check never accepts localhost). */
const PUBLIC_HOST = /^((www\.)?humcase\.com|[a-z0-9-]+\.humcase-site\.pages\.dev)$/;
const LOCAL_HOST = /^(localhost|127\.0\.0\.1)$/;
export const turnstileHostOk = (hostname, requestHost) =>
  typeof hostname === 'string' && (PUBLIC_HOST.test(hostname) || (LOCAL_HOST.test(requestHost) && LOCAL_HOST.test(hostname)));

/** Whether Cloudflare Turnstile vouches for this sign-up: a fresh token (single use) for the form's
 *  action, issued on one of the site's pages. Fails closed: no secret, no token or no answer is a no. */
export async function verifyHuman(secret, token, ip, requestHost, fetcher = fetch) {
  if (!secret || typeof token !== 'string' || token.length === 0 || token.length > 2048) return false;
  const form = new URLSearchParams({ secret, response: token });
  if (ip) form.set('remoteip', ip);
  let result;
  try {
    const res = await fetcher('https://challenges.cloudflare.com/turnstile/v0/siteverify', {
      method: 'POST',
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
      body: form,
      signal: AbortSignal.timeout(10000),
    });
    if (!res.ok) return false;
    result = await res.json();
  } catch {
    return false;
  }
  return result?.success === true && result.action === TURNSTILE_ACTION && turnstileHostOk(result.hostname, requestHost);
}

/** One call to the Resend API. */
export async function resend(apiKey, method, path, body) {
  const res = await fetch(`https://api.resend.com${path}`, {
    method,
    headers: { Authorization: `Bearer ${apiKey}`, 'Content-Type': 'application/json' },
    body: body ? JSON.stringify(body) : undefined,
  });
  let data = null;
  try { data = await res.json(); } catch { data = null; }
  return { ok: res.ok, status: res.status, data };
}
