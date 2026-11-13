//! Words too common to tell two cases apart, in the six interface languages: articles,
//! pronouns, prepositions, conjunctions and auxiliaries of three letters or more (shorter words
//! are already skipped by their callers). Two cases sharing only "pour" or "with" are not alike.
//!
//! Used by the duplicate check (`queries::find_similar_notes`) and by To tidy (`gather`).

const COMMON_WORDS: &[&str] = &[
    // English
    "the", "and", "for", "with", "from", "that", "this", "these", "those", "have", "has", "had",
    "will", "would", "should", "could", "can", "are", "was", "were", "been", "being", "not", "but",
    "you", "your", "our", "their", "they", "them", "she", "his", "her", "its", "who", "what",
    "when", "which", "where", "there", "then", "than", "into", "onto", "about", "all", "any",
    "some", "also", "just", "only", "very", "out", "off", "over", "after", "before", "until",
    "via", // French
    "les", "des", "une", "est", "sont", "pour", "par", "avec", "dans", "sans", "sur", "sous",
    "chez", "vers", "entre", "plus", "moins", "pas", "que", "qui", "quoi", "dont", "elle", "elles",
    "ils", "nous", "vous", "leur", "leurs", "son", "ses", "mon", "mes", "ton", "tes", "notre",
    "nos", "votre", "vos", "cette", "ces", "cet", "tout", "tous", "toute", "toutes", "comme",
    "mais", "donc", "car", "aussi", "après", "avant", "quand", "être", "avoir", "était", "fait",
    "faire", "été", "aux", "ont", "très", // Spanish
    "los", "las", "del", "una", "unos", "unas", "para", "por", "con", "sin", "sobre", "entre",
    "que", "como", "pero", "más", "este", "esta", "estos", "estas", "ese", "esa", "sus", "nos",
    "ellos", "ellas", "usted", "ser", "estar", "hay", "muy", "cuando", "donde", "también",
    // German
    "der", "die", "das", "den", "dem", "des", "ein", "eine", "einen", "einem", "einer", "und",
    "oder", "aber", "mit", "von", "für", "auf", "aus", "bei", "nach", "über", "unter", "vor",
    "bis", "ist", "sind", "war", "wird", "werden", "haben", "hat", "nicht", "auch", "noch", "wie",
    "wenn", "dass", "sich", "sie", "wir", "ihr", "ich", "zum", "zur", "beim", "vom",
    // Portuguese
    "dos", "das", "uma", "umas", "uns", "com", "sem", "pelo", "pela", "pelos", "pelas", "seu",
    "sua", "seus", "suas", "isso", "isto", "esse", "essa", "mas", "mais", "não", "são", "foi",
    "ter", "está", "quando", "onde", "também", "você", "eles", "elas", "nós", "até", "após",
    // Italian
    "gli", "della", "delle", "degli", "dei", "del", "dal", "dalla", "nel", "nella", "nei", "sul",
    "sulla", "per", "con", "tra", "fra", "che", "chi", "non", "sono", "anche", "come", "quando",
    "dove", "questo", "questa", "questi", "queste", "quello", "quella", "loro", "suo", "sua",
    "suoi", "essere", "avere", "molto", "dopo", "prima",
];

/// Whether `word`, already in lower case, is too common to tell two cases apart.
pub fn is_common(word: &str) -> bool {
    COMMON_WORDS.contains(&word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_listed_word_is_lower_case() {
        for word in COMMON_WORDS {
            assert_eq!(*word, word.to_lowercase(), "{word}");
        }
    }

    #[test]
    fn function_words_are_common_and_subjects_are_not() {
        for word in ["pour", "with", "para", "und", "com", "della"] {
            assert!(is_common(word), "{word}");
        }
        for word in ["plombier", "devis", "cuisine", "fuite", "invoice"] {
            assert!(!is_common(word), "{word}");
        }
    }
}
