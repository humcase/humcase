"""Writes the live cases of a Humcase database to a bench corpus, reading it in read-only mode.

Usage: python make_corpus.py [path/to/humcase.db] [out/corpus.json]
The database defaults to %USERPROFILE%/.humcase/humcase.db; it is copied in memory, never written.
The corpus holds the person's own notes: keep it out of the repository.
"""
import json
import os
import pathlib
import sqlite3
import sys

source = sys.argv[1] if len(sys.argv) > 1 else os.path.join(
    os.path.expanduser('~'), '.humcase', 'humcase.db')
out = sys.argv[2] if len(sys.argv) > 2 else 'corpus.json'

src = sqlite3.connect(pathlib.Path(source).resolve().as_uri() + '?mode=ro', uri=True)
copy = sqlite3.connect(':memory:')
src.backup(copy)
src.close()
rows = copy.execute(
    "SELECT id, title, COALESCE(content, ''), note_type FROM notes WHERE deleted_at IS NULL"
).fetchall()
copy.close()

docs = [
    {'id': r[0], 'title': r[1] or '', 'text': ((r[1] or '') + '\n\n' + r[2]).strip(), 'type': r[3]}
    for r in rows
]
with open(out, 'w', encoding='utf-8') as f:
    json.dump(docs, f, ensure_ascii=False)
words = [len(d['text'].split()) for d in docs]
print(len(docs), 'cases; words avg', sum(words) // max(len(words), 1), 'max', max(words, default=0))
