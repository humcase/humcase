# Results — 03/10/2026

Machine: AMD Ryzen 7 7800X3D (8 cores, AVX-512), 31 GB of RAM, Windows 11 Pro build 26300. Unless stated, one thread at below-normal priority, graph optimization level 3, pieces of 300 words capped at 512 tokens, plus a vector of the title alone, files read from a warm disk cache.

## Test set

- **24 target cases** of the founder's base (bugs, ideas, research, decisions; 5 to 256 words), each with two queries in each of the six launch languages (EN, FR, ES, DE, IT, PT-BR): 288 queries.
  - **meaning**: says the same thing as the case without its words ("keep the program running in the background when the window is closed" for "Add the System Tray").
  - **words**: typed as in a search box, with the case's key words in that language ("system tray notification area").
- **Across languages**: the 446 live cases of the founder's base, mostly French, queried in the six languages.
- **Within one language**: 150 cases (the 24 targets, the 56 cases that most often ranked above a target in a first run, 70 drawn at random), cut to their first 150 words, translated whole into each of the five other languages by an agent (Claude Sonnet), and queried in the same language.
- **Searches compared**:
  - **model**: the embedding model alone;
  - **current**: Humcase's current search (the same SQLite FTS5 query: every word required, exact words, title weighted 5);
  - **any word**: BM25 where any word counts, without the words found in more than a tenth of the cases;
  - **model + any word**: their fusion (reciprocal rank).

## Quality

Expected case in the first five results.

**Within one language** (150 cases in the query's language), out of 24 per language:

| | EN | FR | ES | DE | IT | PT-BR | Total / 144 |
|---|---|---|---|---|---|---|---|
| meaning, Granite R2 | 17 | 16 | 16 | 17 | 16 | 15 | **97** |
| meaning, e5-small | 14 | 13 | 15 | 15 | 14 | 15 | 86 |
| meaning, any word | 11 | 11 | 14 | 13 | 10 | 14 | 73 |
| meaning, Granite + any word | 14 | 13 | 19 | 15 | 15 | 19 | 95 |
| meaning, current search | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| words, Granite R2 | 24 | 24 | 24 | 24 | 24 | 23 | **143** |
| words, e5-small | 24 | 24 | 24 | 24 | 24 | 24 | 144 |
| words, any word | 24 | 24 | 24 | 24 | 24 | 24 | **144** |
| words, current search | 18 | 20 | 12 | 18 | 15 | 15 | 98 |

**Across languages** (446 French cases, queries in each language), out of 24 per language:

| | EN | FR | ES | DE | IT | PT-BR | Total / 144 |
|---|---|---|---|---|---|---|---|
| meaning, Granite R2 | 11 | 13 | 11 | 13 | 14 | 15 | **77** |
| meaning, e5-small | 9 | 9 | 9 | 7 | 8 | 10 | 52 |
| meaning, any word | 0 | 8 | 0 | 0 | 0 | 0 | 8 |
| meaning, current search | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| words, Granite R2 | 24 | 24 | 24 | 24 | 24 | 23 | **143** |
| words, e5-small | 23 | 24 | 23 | 21 | 20 | 23 | 134 |
| words, any word | 16 | 24 | 12 | 14 | 13 | 12 | 91 |
| words, current search | 3 | 20 | 1 | 2 | 1 | 1 | 28 |

What it says:

- **Granite R2 is the model**: ahead of e5-small within one language (97 against 86 out of 144 on meaning) and far ahead across languages (77 against 52; 143 against 134 with words), at half its memory.
- **The current search misses a third of the cases even with their own words** (98 out of 144): every word must be in the case, exactly as typed (no plural, no other form). A word search where any word counts, ranked by BM25 without the very common words, finds all 144. This needs no model and no AI.
- **The model is needed for queries that do not use the case's words** (97 against 73 for any word, 0 for the current search), and for queries in another language than the case (77 against 8).
- **Fusing the model with the word search brings nothing on the whole** (95 against 97 within one language), with large differences by language (better in Spanish and Portuguese, worse in English and French). The model alone is kept; the improved word search serves when the model is off or unavailable.
- **Misses** are mostly very short cases (a 5-word title such as "[Phase 2] Système de Tags") and cases whose neighbours speak of the same thing (an automatic title for typed notes ranks above the one for dictation).

## Word search without a model (`word_bench`)

Rules run on SQLite FTS5 like Humcase. "In the list" counts the target among the cases the rule returns; the size is the median and 90th percentile of cases returned. Within one language (150 cases per language, 144 queries of each kind):

| Rule | Words: in the first five | Words: in the list | Words: size | Meaning: in the list | Meaning: size |
|---|---|---|---|---|---|
| Every word, as typed (Humcase until now) | 98 | 98 | 1, 1 | 0 | 0, 0 |
| Every word, near forms | 107 | 107 | 1, 1 | 0 | 0, 0 |
| Any rare word, near forms | 144 | 144 | 11–18, 24–28 | 97 | 16–21, 29–37 |
| Any rare word, kept at ≥ 30 % of the best | 144 | 144 | 3–4, 5–8 | 94 | 11–17, 19–28 |
| Any rare word, kept at ≥ 50 % of the best | 144 | 144 | 1, 2–4 | 79 | 4–8, 10–13 |
| **Every word, near forms, plus any rare word at ≥ 40 % of the best** | **144** | **144** | **2, 3–5** | **86** | **8–13, 14–22** |

On the 446 French cases, French queries: every word as typed finds 20 of 24 with its words, the chosen rule 24 (4 cases listed, 13 at the 90th percentile).

- **Near forms**: a word of five letters or more loses a final s or x, then a final e, and is searched as a prefix ("liens" finds "lien", "perdus" finds "perdu").
- **Rare words**: found in at least one case and at most a tenth of them; when none is, all words count.
- The cases holding every word are always kept, so one word lists every case holding it, as before, and nothing the former rule found is lost.
- Chosen for Humcase (`src-tauri/src/db/queries/search.rs`).

## Cost

| | Granite R2, 512 tokens | Granite R2, 2048 tokens | e5-small, 512 tokens |
|---|---|---|---|
| First vector (load + one embedding) | 0.80–0.88 s | 0.86 s | 0.54–0.58 s |
| Process memory after loading | 182 MB | 184 MB | 408 MB |
| Peak while indexing | 276 MB | 536 MB | 491 MB |
| After the model is released | 47 MB (11 MB before loading) | 47 MB | 44 MB |
| Indexing, per case (446 cases) | 123 ms (45 ms on 4 threads) | 120 ms | 72 ms |
| Indexing, whole base (446 cases) | 55 s (20 s on 4 threads) | 54 s | 32 s |
| One query | 5–6 ms | 6 ms | 3 ms |
| Comparing a query with every case | 0.2 ms | 0.1 ms | 0.2 ms |

- The cases of the translated set are cut at 150 words, so they index faster (82–104 ms per case with Granite).
- Size added to an executable by ONNX Runtime and the tokenizer: 24.4 MB (`probe_ort` 24.6 MB, `probe_none` 0.14 MB). `Humcase.exe` is 26.4 MB.
- Graph optimization levels 1, 2 and 3 load in 0.80 to 0.90 s with the same speed; `--no-memory-pattern` changes nothing.
- About 36 MB stay in the process after the session and the vectors are dropped: a worker process that exits after a minute idle returns to 0.

## Platform

From the [`ort` documentation](https://ort.pyke.io/misc/prebuilt-binaries): the prebuilt x86-64 binaries need an x86-64-v3 processor (Intel Haswell 2013 or later, any AMD Ryzen; not most low-cost Intel chips such as Pentiums), and the platform notes say Windows builds "require a recent version of Windows 11". Linked statically, `probe_ort.exe` imports `DirectML.dll`, `d3d12.dll`, `dxgi.dll`, `MSVCP140.dll`, `MSVCP140_1.dll` and `VCRUNTIME140_1.dll` at load time, which `Humcase.exe` does not: if one is missing, Windows refuses to start the whole executable.

## Conclusion

- **Model**: Granite Embedding 97M multilingual R2 (Apache-2.0), pieces of 300 words capped at 512 tokens, plus a vector of the title.
- **Where it runs**: in a separate worker executable shipped next to `Humcase.exe`.
  - It starts on the first search or to index, and stops after a minute idle.
  - A machine that cannot run it loses search by meaning only, and memory at rest stays at 0.
- **Without the model**: the word search keeps every case holding all the words (near forms included) and adds the cases holding some of the rarer words that score at least 40 % of the best (see above).

## Limits

- Translations were made by an agent: a few interface strings quoted in the cases stayed in French in some languages. Cases are cut at 150 words in the translated set.
- 24 targets: one case more or less moves a language by 4 points.
- Not measured: a cold disk (first read of the 98 MB model after a restart), and a machine slower than this one.
