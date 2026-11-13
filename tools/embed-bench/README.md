# embed-bench

A bench of local embedding models, run before Humcase adds search by meaning. It is not part of the application build; it is a standalone crate (`[workspace]` of its own) that links the same runtime the application would use: [ONNX Runtime](https://onnxruntime.ai/) through the [`ort`](https://ort.pyke.io/) crate (`=2.0.0-rc.13`, linked statically, prebuilt binaries) and the Hugging Face [`tokenizers`](https://crates.io/crates/tokenizers) crate (`fancy-regex`, no native regex library).

It measures what the decision needs:

- time until the first vector (load the tokenizer, build the session, embed one word);
- memory of the process: before, after loading, peak while indexing, and after the model is released;
- time per case while indexing and per query, on one thread at below-normal priority;
- how often the expected case comes back first and in the first five results, with the mean reciprocal rank, per language (EN, FR, ES, DE, IT, PT-BR);
- the same for Humcase's current search (the same SQLite FTS5 query: every word required, title weighted 5), for a word search where any word counts (BM25 without the very common words), and for the fusion of the model with each of them (reciprocal rank fusion);
- the size the runtime adds to an executable: `probe_ort` minus `probe_none`.

## Models

| | Granite Embedding 97M multilingual R2 | multilingual-e5-small |
|---|---|---|
| Source | [ibm-granite/granite-embedding-97m-multilingual-r2](https://huggingface.co/ibm-granite/granite-embedding-97m-multilingual-r2) | [intfloat/multilingual-e5-small](https://huggingface.co/intfloat/multilingual-e5-small) |
| Licence | Apache-2.0 | MIT |
| File | `onnx/model_quint8_avx2.onnx` (98 MB) + `tokenizer.json` (25 MB) | `onnx/model_qint8_avx512_vnni.onnx` (118 MB) + `onnx/tokenizer.json` (17 MB) |
| Vector | 384 dimensions, CLS token, normalized | 384 dimensions, mean of tokens, normalized |
| Prefixes | none | `query: ` / `passage: ` |
| Context | 32,768 tokens | 512 tokens |

Put each model's ONNX file as `model.onnx` next to its `tokenizer.json`, for example in `data/models/granite/` (the `data/` folder is git-ignored).

## Data

- `make_corpus.py` writes the live cases of a Humcase database to `corpus.json`, reading a copy made in memory from a read-only connection. The corpus holds the person's own notes: it stays in `data/`, never in the repository.
- `queries.json` lists 24 target cases (first 8 characters of their id), each with a `meaning` query (the same thing said without the case's words) and a `words` query (typed as in a search box) in each language.
- To measure within one language, translate a corpus whole and pass it with `--corpus corpus_xx.json --langs xx --corpus-lang xx`: only the queries in that language run. Translations contain the cases' text and stay in `data/` too.

## Run

```bash
cargo build --release
```

```bash
./target/release/embed-bench --model granite --dir data/models/granite --corpus data/corpus.json --queries queries.json --max-tokens 512 --chunk-words 300 --title --out data/granite.json
```

Options: `--langs en,fr` (query languages, all six by default), `--corpus-lang xx` (label of a translated corpus), `--threads N` (default 1), `--max-tokens N` (2048 for Granite, 512 for e5 by default), `--chunk-words N` (split long cases into pieces of N words; a case scores as its best piece; 0 keeps it whole), `--title` (adds a vector of the title alone), `--opt 0-3` (ONNX Runtime graph optimization), `--no-memory-pattern`.

`word_bench` compares word-search rules without a model, on the same queries and corpora:

```bash
./target/release/word_bench --queries queries.json --corpus data/corpus.json:fr,data/corpus_en.json:en
```

## Results

See [RESULTS.md](RESULTS.md).
