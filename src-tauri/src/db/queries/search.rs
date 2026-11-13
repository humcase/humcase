//! Word search over the cases (interface, MCP `list_notes`, CLI).
//!
//! A case is returned when it holds every word of the search, or when it holds some of them and
//! scores close to the best case. Each word also finds its near forms: a word of five letters or
//! more loses a final s, x or e and is searched as a prefix ("liens" finds "lien"). Words found
//! in more than a tenth of the cases ("the", "de", "la") do not bring partial matches up.
//!
//! Measured on 24 cases searched in six languages (`tools/embed-bench`, `word_bench`): the
//! former rule (every word, exactly as typed) found 98 of 144 searches typed with the case's own
//! words; this one finds 144, with two cases listed in the usual search.

use std::collections::HashSet;

use rusqlite::Connection;

/// Relevance of a search, title first (columns: title, content, raw_content, report).
pub const SEARCH_RANK: &str = "bm25(notes_fts, 5.0, 1.0, 1.0, 1.0)";

/// Least share of the best score that a case holding only some of the words needs to be returned.
const PARTIAL_MATCH_SHARE: f64 = 0.4;

/// Live cases matching an FTS5 query, with their score (higher is better), best first.
const LIVE_MATCHES: &str = "SELECT notes_fts.rowid, -bm25(notes_fts, 5.0, 1.0, 1.0, 1.0)
       FROM notes_fts JOIN notes n ON n.rowid = notes_fts.rowid
      WHERE notes_fts MATCH ?1 AND n.deleted_at IS NULL
      ORDER BY bm25(notes_fts, 5.0, 1.0, 1.0, 1.0)";

/// A word search, ready for SQL.
pub struct WordSearch {
    /// FTS5 query matching every case the search may return: any of the rare words, near forms
    /// included. Callers use it for the snippet and the ranking.
    pub fts: String,
    /// JSON array of the rowids the search returns, for `rowid IN (SELECT value FROM json_each(?))`.
    pub kept: String,
}

/// Words of a search as FTS5 cuts them (letters and digits, anything else separates), in lower
/// case, without repeats.
fn search_words(text: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
    {
        if !words.contains(&word) {
            words.push(word);
        }
    }
    words
}

/// FTS5 term of a word with its near forms: from five letters, a final s or x, then a final e,
/// are dropped and the rest is searched as a prefix. Shorter words are searched as typed.
fn near_forms(word: &str) -> String {
    let mut chars: Vec<char> = word.chars().collect();
    if chars.len() < 5 {
        return format!("\"{word}\"");
    }
    if matches!(chars.last(), Some('s' | 'x')) {
        chars.pop();
    }
    if chars.last() == Some(&'e') {
        chars.pop();
    }
    let base: String = chars.into_iter().collect();
    format!("\"{base}\"*")
}

fn live_matches(conn: &Connection, fts: &str) -> rusqlite::Result<Vec<(i64, f64)>> {
    let mut stmt = conn.prepare_cached(LIVE_MATCHES)?;
    let rows = stmt.query_map([fts], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Plans the search of `text` (tags already set apart). `None` when it holds no word: the caller
/// returns no case.
pub fn plan_word_search(conn: &Connection, text: &str) -> rusqlite::Result<Option<WordSearch>> {
    let mut terms: Vec<String> = Vec::new();
    for word in search_words(text) {
        let term = near_forms(&word);
        if !terms.contains(&term) {
            terms.push(term);
        }
    }
    if terms.is_empty() {
        return Ok(None);
    }
    let complete: HashSet<i64> = live_matches(conn, &terms.join(" AND "))?
        .into_iter()
        .map(|(id, _)| id)
        .collect();

    // Partial matches count only the rarer words found somewhere (a word found nowhere is not a
    // rare word), unless every word is common.
    let live: i64 = conn.query_row(
        "SELECT COUNT(*) FROM notes WHERE deleted_at IS NULL",
        [],
        |r| r.get(0),
    )?;
    let mut rare = Vec::new();
    for term in &terms {
        let found = live_matches(conn, term)?.len() as i64;
        if found > 0 && found * 10 <= live {
            rare.push(term.clone());
        }
    }
    let fts = if rare.is_empty() { terms } else { rare }.join(" OR ");

    let hits = live_matches(conn, &fts)?;
    let best = hits.first().map_or(0.0, |&(_, score)| score);
    // A case holding every word holds the rare ones too, so it is among the hits.
    let kept: Vec<i64> = hits
        .into_iter()
        .filter(|&(id, score)| complete.contains(&id) || score >= best * PARTIAL_MATCH_SHARE)
        .map(|(id, _)| id)
        .collect();
    Ok(Some(WordSearch {
        fts,
        kept: serde_json::to_string(&kept).unwrap_or_else(|_| "[]".into()),
    }))
}
