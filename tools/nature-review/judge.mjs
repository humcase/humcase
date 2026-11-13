// Asks Jev (TypeSafe System One) the nature of each case, to measure Humcase's nature
// proposals against an independent reader. Development tool only: the app never calls it.
//
//   node tools/nature-review/judge.mjs <cases.json> <out.json> [question.json]
//
// The question is the app's own, `src-tauri/src/jev_nature.v2.json` (`crate::jev`), unless another
// version is given to compare.
// <cases.json>: [{ "id", "title"?, "text" }]. The key is read from TYPESAFE_API_KEY and never
// printed. Each case is asked twice in one request, the options in opposite orders (Jev leans
// toward the first option): `agree` says both orders gave the same nature. Strings that look like
// secrets or email addresses are replaced before sending, and the text is cut to 300 words.
// Tokens and the estimated cost are written in <out.json> and printed.
import { readFileSync, writeFileSync } from 'node:fs';

const ENDPOINT = 'https://api.typesafe.ai/v1/systemone';
// Official pricing (https://docs.typesafe.ai/models, read 2026-10-04): $0.042 per million input
// tokens, output not billed.
const USD_PER_MTOK_IN = 0.042;
const WORDS = 300;
const CONCURRENCY = 8;

const [casesPath, outPath, questionPath = new URL('../../src-tauri/src/jev_nature.v2.json', import.meta.url)] = process.argv.slice(2);
if (!casesPath || !outPath) {
  console.error('usage: node tools/nature-review/judge.mjs <cases.json> <out.json> [question.json]');
  process.exit(1);
}
const key = process.env.TYPESAFE_API_KEY;
if (!key) {
  console.error('TYPESAFE_API_KEY is not set.');
  process.exit(1);
}
const spec = JSON.parse(readFileSync(questionPath, 'utf8'));
const choice = options => ({
  type: 'choice',
  instructions: spec.instructions,
  criteria: Object.fromEntries(options.map(option => [option.nature, option.criteria])),
});
const cases = JSON.parse(readFileSync(casesPath, 'utf8'));

/** Hides what could be a secret or an address before the text leaves the computer. */
export function redact(text) {
  return text
    .replace(/\b(sk|pk|rk|gsk|csk|xai|ghp|gho|github_pat|glpat|AKIA)[-_][A-Za-z0-9_-]{8,}/g, '[secret]')
    .replace(/\bBearer\s+[A-Za-z0-9._~+/-]{8,}=*/gi, 'Bearer [secret]')
    .replace(/\b[A-Za-z0-9+/_-]{32,}={0,2}/g, '[secret]')
    .replace(/[\w.+-]+@[\w-]+\.[\w.-]+/g, '[email]');
}

const firstWords = text => text.split(/\s+/).filter(Boolean).slice(0, WORDS).join(' ');

async function ask(item) {
  const state = { ...(item.title ? { title: redact(item.title) } : {}), text: redact(firstWords(item.text ?? '')) };
  const body = JSON.stringify({
    state,
    model: spec.model,
    questions: {
      forward: choice(spec.options),
      backward: choice([...spec.options].reverse()),
    },
  });
  for (let attempt = 0; ; attempt++) {
    const response = await fetch(ENDPOINT, {
      method: 'POST',
      headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
      body,
    });
    if ((response.status === 429 || response.status === 529) && attempt < 6) {
      await new Promise(resolve => setTimeout(resolve, 500 * 2 ** attempt + Math.random() * 250));
      continue;
    }
    if (!response.ok) throw new Error(`${item.id}: HTTP ${response.status} ${(await response.text()).slice(0, 300)}`);
    const reply = await response.json();
    const { forward, backward } = reply.answers;
    return {
      id: item.id,
      nature: forward.choice,
      agree: forward.choice === backward.choice,
      backward: backward.choice,
      confidence: Math.min(forward.confidence, backward.confidence),
      probabilities: forward.probabilities,
      model: reply.model,
      usage: reply.usage,
    };
  }
}

const results = new Array(cases.length);
let next = 0;
await Promise.all(Array.from({ length: CONCURRENCY }, async () => {
  while (next < cases.length) {
    const index = next++;
    results[index] = await ask(cases[index]);
  }
}));
const input = results.reduce((sum, r) => sum + (r.usage?.input_tokens ?? 0), 0);
const output = results.reduce((sum, r) => sum + (r.usage?.output_tokens ?? 0), 0);
const usage = {
  provider: 'typesafe',
  model: results[0]?.model,
  questions: spec.version,
  requests: results.length,
  input_tokens: input,
  output_tokens: output,
  estimated_usd: Number(((input / 1e6) * USD_PER_MTOK_IN).toFixed(5)),
};
writeFileSync(outPath, JSON.stringify({ usage, results }, null, 1));
console.log(JSON.stringify(usage));
