# Nature review: measurements

Development tool, never run by the app. It asks Jev (TypeSafe System One) the nature of each
case with the app's own question (`src-tauri/src/jev_nature.v2.json`), to measure readers of
natures against an independent one.

```
node tools/nature-review/judge.mjs <cases.json> <out.json> [question.json]
```

`<cases.json>` is `[{ "id", "title"?, "text" }]`. The key comes from `TYPESAFE_API_KEY` and is
never printed. Each case is asked twice in one request, the options in opposite orders (Jev leans
toward the first option); strings that look like secrets or email addresses are replaced and the
text is cut to 300 words, as in the app. Tokens and the estimated cost are written in `<out.json>`.

## Results of 2026-10-04 (jev-1.13.0, question `nature.v1`)

**Jev against known natures.** The 360 cases of `src-tauri/src/recognize_cases.json` (60 per
language, natures known): 341 right (95%); per language en 57, fr 58, es 56, de 57, it 56, pt 57
out of 60. Both orders agree on 355. Mistakes are mostly research read as a thought (11). Cost:
337,486 input tokens, $0.014.

**Readers on a real base.** A copy of a real base (450 live text cases, mostly French development
work), in an isolated profile, every case read by Jev (540,590 input tokens, $0.023). A proposal
counts as right when Jev, sure of itself (both orders agree, confidence at least 0.7), reads the
proposed nature, and as against when it reads the current one.

| Reader on the computer | Proposals | Right | Against |
| --- | --- | --- | --- |
| Nearest cases by meaning (10, loose thresholds) | 123 | 20 | 43 |
| Wording (`recognize`), opening of the case | 134 | 19 | 59 |
| Linear head on the meaning vectors, learned from the base's own natures (p ≥ 0.8) | 36 | 10 | 9 |
| Linear head learned from the 360 multilingual cases | 14 | 5 | 2 |

None is good enough to ask the person's time: the base's own natures are inconsistent (Jev agrees
with 256 of 327), so a reader that learns them learns their noise, and short multilingual cases do
not teach the long cases of a real base. A head learned from Jev's consistent readings of the same
base reached 92–96% at p ≥ 0.8, a lead for a local reader later, from a public corpus.

**Threshold of the review.** Jev's reading is proposed when both orders agree with a confidence
of at least 0.9 (`natures::MIN_CONFIDENCE`): 6 of the 360 known cases would be proposed wrongly
(10 at 0.7), and the real base gets 40 proposals out of 450 (71 at 0.7).

## Question `nature.v2` (2026-10-04, jev-1.13.0)

Read on a real base, `nature.v1` proposed tasks written by agents as decisions when they start
with the choice they carry out ("Décision du fondateur…", a `### Décision` section), the steps of a
plan as thoughts, the lots of a plan as ideas and a night report as a decision. `confusions.json`
reproduces these shapes with invented cases (16, five languages, controls included).

| Question | `confusions.json` right | proposed wrongly (both orders agree, ≥ 0.9) | 360 known cases right | proposed wrongly |
| --- | --- | --- | --- | --- |
| `nature.v1` | 6/16 | 3 | 342 | 6 |
| `nature.v2` | 12/16 | 0 | 339 | 7 |

`nature.v2` judges what the case is mainly for: a case that mainly lays out work to build stays a
task when it first recalls its decision; a short choice followed by its next step stays a
decision; an account of what was done is a thought. A first draft that pulled every case naming
work toward a task got 16/16 on the confusions but lost 9 known cases (decisions followed by an
action read as tasks).

On the real base (489 live text cases), proposals went from 45 to 26: the tasks read as
decisions, the steps read as thoughts and the lots read as ideas are gone; two right proposals
are lost (task → research) with them, and 10 new ones are mostly old ideas carried out as tasks.
Cost of each full reading: about $0.03.
