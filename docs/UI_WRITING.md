# Writing the interface

How every visible text of Humcase is written: the app, its errors, and the website. Terms come from
[`GLOSSARY.md`](GLOSSARY.md); translation method from [`TRANSLATION.md`](TRANSLATION.md). The French
column of the examples is the reference wording the founder approved, kept next to the English as in the
glossary (an exception to the English-only rule, like the glossary's columns).

The model is a professional tool people trust with their work: Linear, Stripe, GitHub, Obsidian. Calm,
exact, short. Never a blog, never an ad, never a chat.

## Rules

1. **Exact terms.** The word a professional uses for the thing: "API key", "local", "environment
   variable", "archive". Not an everyday approximation ("online", "your AI's password").
2. **One idea, once.** A heading is not repeated by the line under it. A page explains a thing once,
   where it acts; the parts below do not explain it again.
3. **Short.** Labels of 1 to 4 words. A hint is one sentence, two at most. Cut every word that changes
   nothing: "Humcase follows…" becomes "Follows…" when the setting already says what it is about.
4. **Say what it does, not how good it is.** No "smart", "powerful", "magic", "seamless". No sparkle icons
   for AI (name the action instead).
5. **Nothing about other companies we cannot guarantee.** No prices ("free"), no rankings ("most
   capable"), no promise about their terms. Name them, say what they need (API key, local).
6. **Never promise what could change.** Say only what ships today ("no account" is not written, sync may
   need one). No roadmap in the interface.
7. **Headings are names, not questions.** "AI services", not "Which AI should you connect?".
8. **Privacy facts are precise and placed above the switch that triggers them**: what leaves, where to,
   what never leaves. Keep these sentences under a test.
9. **Honest scope.** A permission or safeguard text states exactly what is allowed and what is not, no
   wider, no narrower. It may be short; it may not be vague.
10. **No internal names.** No feature code names, file names or paths in the interface unless the person
    needs them to act (the data folder in Data is fine; `ai-usage.jsonl` only next to "Open folder").
11. **Errors say what to do.** "Couldn't save the key. Try again." Not the raw error alone; the raw detail
    goes after, for support.
12. **Buttons are verbs.** "Save", "Open AI services", "Check". Not "OK" or "Yes".
13. **Typography.** No em dash; use a colon, a comma or a new sentence. French: no-break space before
    `: ; ? !` and inside « », sentence case, « vous ». English: sentence case, curly quotes.

## Before and after

| Before (EN) | After (EN) | Après (FR) | Rule |
|---|---|---|---|
| Which AI to connect? Free to start: Groq or Cerebras. Most capable: OpenAI or OpenRouter. | Supported: Groq, Cerebras, OpenAI, OpenRouter, Mistral, Anthropic (API key) · Ollama, LM Studio (local). | Compatibles : Groq, Cerebras, OpenAI, OpenRouter, Mistral, Anthropic (clé API) · Ollama, LM Studio (local). | 1, 5, 7 |
| Online: Groq, Cerebras… | …(API key) | …(clé API) | 1 |
| Appearance. Adjust the look of the app and your reading comfort. Changes apply immediately. | Appearance | Apparence | 2, 3 |
| What the agent can do. What an agent can do in your cases. | What the agent can do | Ce que l’agent peut faire | 2 |
| Humcase follows your system language when it is available, otherwise English. | Follows your system language, otherwise English. | Suit la langue du système, sinon l’anglais. | 3 |
| Your work, in cases. With your agents, or without. | Every decision has roots. | Chaque décision a ses racines. | 4 |

## Before a text ships

- Read it next to how Linear, Stripe or GitHub phrase the same thing.
- Check every rule above; the founder validates new wording in English and French before it is coded.
- Read it in the real app, in context, at full size: a text that repeats its neighbour only shows there.
