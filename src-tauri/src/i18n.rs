//! Texts produced by the engine and shown to the user (errors, native menus, brief).
//!
//! They live in the same language files as the interface
//! (`frontend/src/i18n/locales/<language>/engine.json`), embedded in the binary at compile time:
//! a single place to translate, and the interface consistency test also covers these keys.
//!
//! Language: the interface passes its own (`set_ui_language`); the MCP server and the CLI
//! speak English. A thread-local language ([`with_language`]) takes precedence over the
//! global language: the MCP uses it, which keeps its answers in English even in tests, which
//! run in parallel.

use serde_json::Value;
use std::cell::Cell;
use std::fmt::Display;
use std::sync::{OnceLock, RwLock};

/// Engine texts of each interface language (`frontend/src/i18n/language.ts`).
const CATALOGS: [(&str, &str); 6] = [
    (
        "en",
        include_str!("../../frontend/src/i18n/locales/en/engine.json"),
    ),
    (
        "fr",
        include_str!("../../frontend/src/i18n/locales/fr/engine.json"),
    ),
    (
        "es",
        include_str!("../../frontend/src/i18n/locales/es/engine.json"),
    ),
    (
        "de",
        include_str!("../../frontend/src/i18n/locales/de/engine.json"),
    ),
    (
        "pt-BR",
        include_str!("../../frontend/src/i18n/locales/pt-BR/engine.json"),
    ),
    (
        "it",
        include_str!("../../frontend/src/i18n/locales/it/engine.json"),
    ),
];

/// Fallback language: a text missing from a language is shown in English.
const FALLBACK: &str = "en";

static GLOBAL_LANGUAGE: RwLock<&'static str> = RwLock::new("fr");

thread_local! {
    static THREAD_LANGUAGE: Cell<Option<&'static str>> = const { Cell::new(None) };
}

fn catalog(language: &str) -> &'static Value {
    static PARSED: OnceLock<Vec<(&'static str, Value)>> = OnceLock::new();
    let parsed = PARSED.get_or_init(|| {
        CATALOGS
            .iter()
            .map(|(code, text)| {
                let value = serde_json::from_str(text)
                    .unwrap_or_else(|e| panic!("engine.json ({code}) is not valid JSON: {e}"));
                (*code, value)
            })
            .collect()
    });
    parsed
        .iter()
        .find(|(code, _)| *code == language)
        .or_else(|| parsed.iter().find(|(code, _)| *code == FALLBACK))
        .map(|(_, value)| value)
        .expect("the fallback catalog exists")
}

/// Language code known to the engine, by its base language (`fr-CA` → `fr`, `pt-PT` → `pt-BR`);
/// any other language falls back to English.
pub fn normalize(code: &str) -> &'static str {
    let base = code
        .split(['-', '_'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    CATALOGS
        .iter()
        .map(|(known, _)| *known)
        .find(|known| known.split('-').next() == Some(base.as_str()))
        .unwrap_or(FALLBACK)
}

/// Every language the engine speaks, English first.
pub fn languages() -> impl Iterator<Item = &'static str> {
    CATALOGS.iter().map(|(code, _)| *code)
}

/// Name of a language in English, for the AI's instructions.
pub fn english_name(code: &str) -> &'static str {
    match normalize(code) {
        "fr" => "French",
        "es" => "Spanish",
        "de" => "German",
        "pt-BR" => "Brazilian Portuguese",
        "it" => "Italian",
        _ => "English",
    }
}

pub fn set_language(code: &str) {
    if let Ok(mut language) = GLOBAL_LANGUAGE.write() {
        *language = normalize(code);
    }
}

pub fn language() -> &'static str {
    THREAD_LANGUAGE
        .with(Cell::get)
        .unwrap_or_else(|| GLOBAL_LANGUAGE.read().map(|l| *l).unwrap_or(FALLBACK))
}

/// Runs `f` with a language specific to the current thread, then restores the previous one.
pub fn with_language<T>(code: &str, f: impl FnOnce() -> T) -> T {
    let previous = THREAD_LANGUAGE.with(|l| l.replace(Some(normalize(code))));
    struct Restore(Option<&'static str>);
    impl Drop for Restore {
        fn drop(&mut self) {
            THREAD_LANGUAGE.with(|l| l.set(self.0));
        }
    }
    let _restore = Restore(previous);
    f()
}

fn lookup<'a>(root: &'a Value, key: &str) -> Option<&'a str> {
    key.split('.')
        .try_fold(root, |node, part| node.get(part))?
        .as_str()
}

/// i18next plural form: in French and Brazilian Portuguese 0 and 1 are singular, elsewhere only 1.
/// The `many` form (French, Spanish, Italian, Portuguese) only applies to millions written in
/// compact form, which the engine never writes: `other` stands for it.
fn plural_key(language: &str, key: &str, count: &str) -> String {
    let n: f64 = count.parse().unwrap_or(f64::NAN);
    let one = if matches!(language, "fr" | "pt-BR") {
        (0.0..2.0).contains(&n)
    } else {
        n == 1.0
    };
    format!("{key}_{}", if one { "one" } else { "other" })
}

/// Text of `key` in the current language, `{{name}}` variables replaced. Missing key: English,
/// then the key itself (visible, never an empty text).
pub fn tr(key: &str, variables: &[(&str, &dyn Display)]) -> String {
    let language = language();
    let values: Vec<(&str, String)> = variables
        .iter()
        .map(|(name, v)| (*name, v.to_string()))
        .collect();
    let count = values
        .iter()
        .find(|(name, _)| *name == "count")
        .map(|(_, v)| v.as_str());
    let find = |l: &str| -> Option<&'static str> {
        let root = catalog(l);
        count
            .and_then(|c| lookup(root, &plural_key(l, key, c)))
            .or_else(|| lookup(root, key))
    };
    let template = find(language).or_else(|| find(FALLBACK)).unwrap_or(key);
    let mut text = template.to_string();
    for (name, value) in &values {
        text = text.replace(&format!("{{{{{name}}}}}"), value);
    }
    text
}

/// `tr!("notes.notFound", id = note_id)`: shortcut for [`tr`].
#[macro_export]
macro_rules! tr {
    ($key:expr) => { $crate::i18n::tr($key, &[]) };
    ($key:expr, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::tr($key, &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+])
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_by_thread_language_with_variables() {
        assert_eq!(
            with_language("fr", || tr!("notes.notFound", id = "n1")),
            "Case introuvable : n1"
        );
        assert_eq!(
            with_language("en", || tr!("notes.notFound", id = "n1")),
            "Note not found: n1"
        );
    }

    #[test]
    fn thread_language_restored_and_unknown_languages_fall_back_to_english() {
        with_language("en", || {
            with_language("fr-CA", || assert_eq!(language(), "fr"));
            assert_eq!(language(), "en");
        });
        assert_eq!(normalize("de-DE"), "de");
        assert_eq!(normalize("pt-PT"), "pt-BR");
        assert_eq!(normalize("es_MX"), "es");
        assert_eq!(normalize("ja-JP"), "en");
    }

    #[test]
    fn missing_key_stays_visible() {
        assert_eq!(
            with_language("fr", || tr!("key.nonexistent")),
            "key.nonexistent"
        );
    }

    #[test]
    fn french_and_english_plurals() {
        assert_eq!(plural_key("fr", "k", "0"), "k_one");
        assert_eq!(plural_key("en", "k", "0"), "k_other");
        assert_eq!(plural_key("en", "k", "1"), "k_one");
        assert_eq!(plural_key("pt-BR", "k", "0"), "k_one");
        assert_eq!(plural_key("de", "k", "0"), "k_other");
        assert_eq!(plural_key("es", "k", "2"), "k_other");
    }
}
