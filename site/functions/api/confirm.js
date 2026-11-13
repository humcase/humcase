// GET /api/confirm?t=<token>: the link of the confirmation email. Adds the address to the release news
// segment, then shows the result page (/news/?state=confirmed|expired|error&lang=..).
import { SEGMENT_ID, readToken, resend } from '../../functions-lib/news.js';

const page = (origin, state, lang) => Response.redirect(`${origin}/news/?state=${state}&lang=${encodeURIComponent(lang || 'en')}`, 303);

export async function onRequestGet({ request, env }) {
  const url = new URL(request.url);
  const read = await readToken(env.CONFIRM_SECRET, url.searchParams.get('t'));
  if (read.error) return page(url.origin, read.error === 'expired' ? 'expired' : 'invalid', read.lang);
  const key = env.RESEND_API_KEY;
  const created = await resend(key, 'POST', '/contacts', { email: read.email, unsubscribed: false, segments: [{ id: SEGMENT_ID }] });
  if (!created.ok) {
    // Already a contact (signed up before, or unsubscribed): they confirmed again, so subscribe them again.
    const email = encodeURIComponent(read.email);
    const added = await resend(key, 'POST', `/contacts/${email}/segments/${SEGMENT_ID}`);
    const updated = await resend(key, 'PATCH', `/contacts/${email}`, { unsubscribed: false });
    if (!added.ok && !updated.ok) {
      console.error('resend contact failed', created.status, added.status, updated.status);
      return page(url.origin, 'error', read.lang);
    }
  }
  return page(url.origin, 'confirmed', read.lang);
}
