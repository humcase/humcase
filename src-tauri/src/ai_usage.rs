//! What each AI call consumes: tokens sent, received and spent reasoning, audio minutes, how long
//! it took, and an estimated cost from a dated price table. Founder rule (03/10): no AI call
//! without its consumption context, per provider and model, since people will swap providers and
//! the costs drive the future subscription.
//!
//! JSON Lines file next to the database (`ai-usage.jsonl`), so inside the isolated profile during
//! tests. It holds numbers only: never a text, a key or a case identifier. Bounded like the
//! dictation journal; a write failure never blocks the call.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "ai-usage.jsonl";
const MAX_SIZE: u64 = 4 * 1024 * 1024;

/// Name of each operation recorded in the journal. The interface gives each one a label and an
/// icon (`UsageSection`); its test reads this list and fails when one is missing.
pub mod operation {
    pub const TRANSCRIPTION: &str = "transcription";
    pub const INTEGRATE: &str = "integrate";
    pub const STRUCTURE: &str = "structure";
    pub const CLEAN: &str = "clean";
    pub const STRUCTURE_AGENT: &str = "structure_agent";
    pub const GERMINATE: &str = "germinate";
    pub const CHECK_NATURE: &str = "check_nature";
    pub const CHECK_PROPOSALS: &str = "check_proposals";
    pub const CHECK_DECISIONS: &str = "check_decisions";
    pub const CHECK_GATHER: &str = "check_gather";
    pub const CHECK_PROFILE: &str = "check_profile";
    pub const ASK: &str = "ask";
    pub const REFLECT: &str = "reflect";
}

/// Day the prices below were read from the providers' pages.
pub const PRICES_AS_OF: &str = "2026-10-08";

/// Tokens of one language model call, as the provider reports them (`usage` of the
/// OpenAI-compatible answer). Absent when the provider does not report them.
#[derive(Debug, Default, Clone, PartialEq, Serialize)]
pub struct TokenUsage {
    pub input_tokens: Option<u64>,
    /// Billed output, the reasoning included.
    pub output_tokens: Option<u64>,
    /// Part of the output spent reasoning before answering (billed, never shown).
    pub reasoning_tokens: Option<u64>,
    /// Part of the input served from the provider's cache.
    pub cached_tokens: Option<u64>,
}

impl TokenUsage {
    /// Reads `usage` from an OpenAI-compatible chat completion (Cerebras, OpenAI, Groq, OpenRouter,
    /// Ollama): `prompt_tokens`, `completion_tokens`, `completion_tokens_details.reasoning_tokens`,
    /// `prompt_tokens_details.cached_tokens`.
    pub fn from_response(data: &serde_json::Value) -> Self {
        let usage = &data["usage"];
        Self {
            input_tokens: usage["prompt_tokens"].as_u64(),
            output_tokens: usage["completion_tokens"].as_u64(),
            reasoning_tokens: usage["completion_tokens_details"]["reasoning_tokens"].as_u64(),
            cached_tokens: usage["prompt_tokens_details"]["cached_tokens"].as_u64(),
        }
    }
}

/// Price of a language model, in US dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TokenPrice {
    pub input: f64,
    pub output: f64,
}

/// Published pay-as-you-go prices, read on [`PRICES_AS_OF`]. A model missing here has no
/// estimate: its tokens are still counted. Sources: cerebras.ai/pricing (relayed by
/// pricepertoken.com and morphllm.com, the page renders its table in script),
/// developers.openai.com/api/docs/pricing. Groq stopped publishing per-token prices for its
/// Llama models in August 2026. Mistral (docs.mistral.ai/models/mistral-large-4-0, read on
/// 2026-10-08): Mistral Large 4 at $1.36 per million input tokens and $4.18 output, list price.
/// Anthropic (platform.claude.com/docs/en/about-claude/pricing, read on 2026-10-08): Claude Haiku
/// 5.5 at $0.10 / $0.50 for prompts up to 100,000 tokens, Sonnet 5.5 at $2 / $10, Opus 5.5 at
/// $4 / $20. TypeSafe (docs.typesafe.ai/models, read on 2026-10-04): Jev 1.13
/// at $0.042 per million input tokens, output not billed.
pub fn token_price(provider: &str, model: &str) -> Option<TokenPrice> {
    let (input, output) = match (provider, model) {
        ("typesafe", m) if m.starts_with("jev-1.13") => (0.042, 0.0),
        ("cerebras", "gpt-oss-120b") => (0.35, 0.75),
        // Prompts up to 100,000 tokens; longer ones cost five times more on Haiku 5.5.
        ("anthropic", "claude-haiku-5-5") => (0.10, 0.50),
        ("anthropic", "claude-sonnet-5-5") => (2.0, 10.0),
        ("anthropic", "claude-opus-5-5") => (4.0, 20.0),
        ("cerebras", "qwen-3.8-27b") => (0.99, 1.49),
        ("openai", "gpt-4o-mini") | ("openrouter", "openai/gpt-4o-mini") => (0.15, 0.60),
        // Public preview since 2026-10-06; the list price, not the launch sale (0.68 / 2.09).
        ("mistral", "mistral-large-4") => (1.36, 4.18),
        ("ollama", _) => (0.0, 0.0),
        _ => return None,
    };
    Some(TokenPrice { input, output })
}

/// Price of streaming transcription, in US dollars per minute of audio. Deepgram (deepgram.com/
/// pricing), Nova-3 streaming regular rates: $0.0077 for one language, $0.0092 for `multi`; the
/// $0.0048 and $0.0058 shown on 03/10 are a limited-time promotion, so budgets use the regular
/// rates.
pub fn audio_price_per_minute(provider: &str, language: &str) -> Option<f64> {
    match (provider, language) {
        ("deepgram", "multi") => Some(0.0092),
        ("deepgram", _) => Some(0.0077),
        _ => None,
    }
}

/// Estimated cost of a language model call; `None` when the price or the tokens are unknown.
pub fn token_cost(provider: &str, model: &str, usage: &TokenUsage) -> Option<f64> {
    let price = token_price(provider, model)?;
    Some(
        usage.input_tokens? as f64 * price.input / 1e6
            + usage.output_tokens? as f64 * price.output / 1e6,
    )
}

/// One AI call: what it was for, with what, what it consumed and whether it succeeded.
#[derive(Debug, Serialize)]
pub struct UsageEntry<'a> {
    /// `clean`, `integrate`, `structure`, `structure_agent`, `transcription` or `check_nature`.
    pub operation: &'a str,
    pub provider: &'a str,
    pub model: &'a str,
    /// Language asked of a transcription (`multi`: several languages); none for a language model.
    pub language: Option<&'a str>,
    #[serde(flatten)]
    pub tokens: TokenUsage,
    pub audio_seconds: Option<f64>,
    pub duration_ms: u64,
    pub ok: bool,
    pub estimated_cost_usd: Option<f64>,
    pub prices_as_of: &'static str,
}

/// Appends an entry next to this database; never returns an error to the caller.
pub fn record(db_path: &Path, entry: &UsageEntry) {
    let Some(folder) = db_path.parent() else {
        return;
    };
    let mut value = match serde_json::to_value(entry) {
        Ok(value) => value,
        Err(_) => return,
    };
    if let Some(object) = value.as_object_mut() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        object.insert("at_unix_ms".into(), serde_json::Value::from(millis));
    }
    if let Err(e) = crate::dictation_journal::append(&folder.join(FILE_NAME), &value, MAX_SIZE) {
        eprintln!("AI usage not recorded ({e})");
    }
}

/// One recorded call as the consumption screen reads it: quantities only. The estimated cost
/// stays in the file for Humcase's own use and is never sent to the interface (founder decision
/// of 03/10: no price in the app, whatever the model).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageRecord {
    pub at_unix_ms: u64,
    pub operation: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub reasoning_tokens: Option<u64>,
    #[serde(default)]
    pub audio_seconds: Option<f64>,
    pub ok: bool,
}

/// The calls recorded next to this database since `since_ms`, oldest first. A missing file is
/// an empty history; an unreadable line (cut by a power failure, older shape) is skipped.
pub fn read(db_path: &Path, since_ms: u64) -> Vec<UsageRecord> {
    let Some(folder) = db_path.parent() else {
        return Vec::new();
    };
    let Ok(content) = std::fs::read_to_string(folder.join(FILE_NAME)) else {
        return Vec::new();
    };
    content
        .lines()
        .filter_map(|line| serde_json::from_str::<UsageRecord>(line).ok())
        .filter(|record| record.at_unix_ms >= since_ms)
        .collect()
}

/// Audio duration a transcription session was billed for, from Deepgram's messages: the
/// `Metadata` message gives the total; until it arrives, the end of the last `Results` segment.
#[derive(Debug, Default)]
pub struct AudioMeter {
    metadata_seconds: Option<f64>,
    last_end: f64,
}

impl AudioMeter {
    pub fn read(&mut self, message: &str) {
        let Ok(data) = serde_json::from_str::<serde_json::Value>(message) else {
            return;
        };
        match data["type"].as_str() {
            Some("Metadata") => self.metadata_seconds = data["duration"].as_f64(),
            Some("Results") => {
                if let (Some(start), Some(duration)) =
                    (data["start"].as_f64(), data["duration"].as_f64())
                {
                    self.last_end = self.last_end.max(start + duration);
                }
            }
            _ => {}
        }
    }

    pub fn seconds(&self) -> f64 {
        self.metadata_seconds.unwrap_or(self.last_end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_read_from_an_openai_compatible_answer() {
        let answer = serde_json::json!({
            "choices": [{ "message": { "content": "ok" } }],
            "usage": {
                "prompt_tokens": 1200, "completion_tokens": 800, "total_tokens": 2000,
                "completion_tokens_details": { "reasoning_tokens": 500 },
                "prompt_tokens_details": { "cached_tokens": 0 }
            }
        });
        let usage = TokenUsage::from_response(&answer);
        assert_eq!(
            usage,
            TokenUsage {
                input_tokens: Some(1200),
                output_tokens: Some(800),
                reasoning_tokens: Some(500),
                cached_tokens: Some(0)
            }
        );
        // 1,200 × $0.35/M + 800 × $0.75/M.
        let cost = token_cost("cerebras", "gpt-oss-120b", &usage).unwrap();
        assert!((cost - 0.00102).abs() < 1e-9, "{cost}");
        assert_eq!(token_cost("groq", "llama-3.3-70b-versatile", &usage), None);
        assert_eq!(
            TokenUsage::from_response(&serde_json::json!({ "choices": [] })),
            TokenUsage::default()
        );
    }

    #[test]
    fn the_audio_meter_prefers_the_billed_total() {
        let mut meter = AudioMeter::default();
        meter.read(r#"{"type":"Results","start":0.0,"duration":2.5}"#);
        meter.read(r#"{"type":"Results","start":2.5,"duration":3.0}"#);
        meter.read("not json");
        assert_eq!(meter.seconds(), 5.5);
        meter.read(r#"{"type":"Metadata","duration":6.25}"#);
        assert_eq!(meter.seconds(), 6.25);
    }

    #[test]
    fn an_entry_holds_numbers_and_never_a_text() {
        let folder = std::env::temp_dir().join(format!("humcase-usage-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&folder).unwrap();
        record(
            &folder.join("humcase.db"),
            &UsageEntry {
                operation: "clean",
                provider: "cerebras",
                model: "gpt-oss-120b",
                language: None,
                tokens: TokenUsage {
                    input_tokens: Some(10),
                    output_tokens: Some(5),
                    ..Default::default()
                },
                audio_seconds: None,
                duration_ms: 300,
                ok: true,
                estimated_cost_usd: Some(0.0000073),
                prices_as_of: PRICES_AS_OF,
            },
        );
        let line = std::fs::read_to_string(folder.join(FILE_NAME)).unwrap();
        let entry: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(entry["input_tokens"], 10);
        assert_eq!(entry["operation"], "clean");
        assert!(entry["at_unix_ms"].as_u64().is_some());

        // The screen reads it back without the cost, skipping a broken line and older calls.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(folder.join(FILE_NAME))
            .unwrap();
        use std::io::Write;
        writeln!(file, "{{\"operation\":\"clean\",\"at_unix").unwrap();
        writeln!(file, "{{\"at_unix_ms\":5,\"operation\":\"structure\",\"provider\":\"cerebras\",\"model\":\"gpt-oss-120b\",\"ok\":true}}").unwrap();
        let records = read(&folder.join("humcase.db"), 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].operation, "clean");
        assert_eq!(records[0].input_tokens, Some(10));
        let sent = serde_json::to_value(&records[0]).unwrap();
        assert!(
            sent.get("estimated_cost_usd").is_none(),
            "no price reaches the interface"
        );
        assert_eq!(read(&folder.join("humcase.db"), 0).len(), 2);
        assert!(read(&folder.join("absent").join("humcase.db"), 0).is_empty());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
