//! TypeSafe's System One, model Jev (https://docs.typesafe.ai): typed questions about a state,
//! answered with probabilities. Humcase asks it one thing, the nature of a case, to propose
//! natures to check (`crate::natures`); it never writes anything.
//!
//! - The question is written and versioned in `jev_nature.v2.json` (shared with the measuring
//!   tool `tools/nature-review`). Jev reads literally: each option has its own criteria.
//! - Jev leans toward the first option of a choice: each case is asked twice in one request,
//!   the options in opposite orders, and only an answer both orders give counts.
//! - Before sending: the title and the first 300 words only, strings that look like secrets or
//!   email addresses replaced. The key is never logged nor shown.

use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::ai_usage::TokenUsage;
use crate::models::note::NoteType;

pub const PROVIDER: &str = "typesafe";
pub const KEY_ENV: &str = "TYPESAFE_API_KEY";
/// The Jev model every question asks for: pinned, so a change on the service never shifts the
/// measured thresholds silently (the questions' files name it too).
pub const MODEL: &str = "jev-1.13.0";
const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
/// The nature question, versioned: a new version reads every case again.
pub const NATURE_QUESTION: &str = include_str!("jev_nature.v2.json");
/// Words of a case sent: its opening says what it is.
const WORDS: usize = 300;

#[derive(Deserialize)]
struct QuestionSpec {
    version: String,
    model: String,
    instructions: String,
    /// In order: the request keeps it, forward and backward.
    options: Vec<NatureOption>,
}

#[derive(Deserialize)]
struct NatureOption {
    nature: String,
    criteria: String,
}

fn spec() -> &'static QuestionSpec {
    static SPEC: std::sync::OnceLock<QuestionSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| serde_json::from_str(NATURE_QUESTION).expect("valid nature question"))
}

/// Version of the nature question (`nature.v2`).
pub fn question_version() -> &'static str {
    &spec().version
}

/// Jev's reading of a case.
#[derive(Debug, Clone, PartialEq)]
pub struct NatureReading {
    pub nature: NoteType,
    /// The lower of the two orders' confidences (0 to 1).
    pub confidence: f32,
    /// Both orders of the options gave this nature.
    pub agree: bool,
    pub model: String,
}

/// Hides what could be a secret or an address before a text leaves the computer.
pub fn redact(text: &str) -> String {
    static RULES: std::sync::OnceLock<Vec<(regex::Regex, &'static str)>> =
        std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (
                r"\b(sk|pk|rk|gsk|csk|xai|ghp|gho|github_pat|glpat|AKIA)[-_][A-Za-z0-9_-]{8,}",
                "[secret]",
            ),
            (r"(?i)\bBearer\s+[A-Za-z0-9._~+/-]{8,}=*", "Bearer [secret]"),
            (r"\b[A-Za-z0-9+/_-]{32,}={0,2}", "[secret]"),
            (r"[\w.+-]+@[\w-]+\.[\w.-]+", "[email]"),
        ]
        .into_iter()
        .map(|(pattern, by)| (regex::Regex::new(pattern).expect("valid regex"), by))
        .collect()
    });
    rules.iter().fold(text.to_string(), |text, (rule, by)| {
        rule.replace_all(&text, *by).into_owned()
    })
}

/// What Jev reads of a case: its title and the first words of its text, redacted.
pub fn state_of(title: &str, content: &str) -> Value {
    let words: Vec<&str> = content.split_whitespace().take(WORDS).collect();
    json!({ "title": redact(title.trim()), "text": redact(&words.join(" ")) })
}

/// The options of a choice, serialized in this order or the opposite one (a JSON object read
/// by `serde_json` would sort them).
struct Criteria {
    backward: bool,
}

impl Serialize for Criteria {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let options = &spec().options;
        let mut map = serializer.serialize_map(Some(options.len()))?;
        let mut write =
            |option: &NatureOption| map.serialize_entry(&option.nature, &option.criteria);
        if self.backward {
            options.iter().rev().try_for_each(&mut write)?;
        } else {
            options.iter().try_for_each(&mut write)?;
        }
        map.end()
    }
}

#[derive(Serialize)]
struct Choice {
    #[serde(rename = "type")]
    kind: &'static str,
    instructions: &'static str,
    criteria: Criteria,
}

/// The request: the nature question in both orders of its options, as sent.
pub fn request_body(title: &str, content: &str) -> String {
    let spec = spec();
    let choice = |backward| Choice {
        kind: "choice",
        instructions: &spec.instructions,
        criteria: Criteria { backward },
    };
    // The options' order must survive: the questions are written as text, not as a `Value`.
    let forward = serde_json::to_string(&choice(false)).expect("serializable");
    let backward = serde_json::to_string(&choice(true)).expect("serializable");
    let state = state_of(title, content).to_string();
    format!(
        r#"{{"state":{state},"model":{model},"questions":{{"forward":{forward},"backward":{backward}}}}}"#,
        model = serde_json::to_string(&spec.model).expect("serializable"),
    )
}

/// Reads Jev's answer to `request_body`.
pub fn parse_answer(reply: &Value) -> Result<NatureReading, String> {
    let choice = |id: &str| -> Result<(NoteType, f32), String> {
        let answer = &reply["answers"][id];
        let nature = answer["choice"]
            .as_str()
            .and_then(NoteType::parse)
            .ok_or_else(|| format!("Jev answered no known nature for {id}"))?;
        Ok((nature, answer["confidence"].as_f64().unwrap_or(0.0) as f32))
    };
    let (forward, forward_confidence) = choice("forward")?;
    let (backward, backward_confidence) = choice("backward")?;
    Ok(NatureReading {
        nature: forward,
        confidence: forward_confidence.min(backward_confidence),
        agree: forward == backward,
        model: reply["model"].as_str().unwrap_or(MODEL).to_string(),
    })
}

/// Tokens of a System One answer (`usage.input_tokens`, `usage.output_tokens`).
pub fn usage_of(reply: &Value) -> TokenUsage {
    TokenUsage {
        input_tokens: reply["usage"]["input_tokens"].as_u64(),
        output_tokens: reply["usage"]["output_tokens"].as_u64(),
        ..TokenUsage::default()
    }
}

/// Why a reading failed: the key (stop until it changes), or something that passes (retry later).
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Key,
    Passing(String),
}

/// Asks Jev the nature of a case.
pub async fn read_nature(
    client: &reqwest::Client,
    key: &str,
    title: &str,
    content: &str,
) -> (Result<NatureReading, Failure>, TokenUsage) {
    let (reply, tokens) = ask(client, key, request_body(title, content)).await;
    (
        reply.and_then(|reply| parse_answer(&reply).map_err(Failure::Passing)),
        tokens,
    )
}

/// The model Jev answered with (`jev-1.13.0`), or the one asked for when it said nothing.
pub fn model_of(reply: &Value) -> String {
    reply["model"].as_str().unwrap_or(MODEL).to_string()
}

/// Sends a request to System One and returns its answer. Busy (429) or overloaded (529):
/// retried with a growing wait.
pub async fn ask(
    client: &reqwest::Client,
    key: &str,
    body: String,
) -> (Result<Value, Failure>, TokenUsage) {
    let mut attempt = 0;
    loop {
        let response = match client
            .post(ENDPOINT)
            .bearer_auth(key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.clone())
            .send()
            .await
        {
            Ok(response) => response,
            Err(e) => {
                return (
                    Err(Failure::Passing(format!(
                        "TypeSafe unreachable: {}",
                        e.without_url()
                    ))),
                    TokenUsage::default(),
                )
            }
        };
        let status = response.status().as_u16();
        if (status == 429 || status == 529) && attempt < 4 {
            attempt += 1;
            tokio::time::sleep(std::time::Duration::from_millis(500 << attempt)).await;
            continue;
        }
        if status == 401 || status == 403 {
            return (Err(Failure::Key), TokenUsage::default());
        }
        if !(200..300).contains(&status) {
            return (
                Err(Failure::Passing(format!("TypeSafe answered HTTP {status}"))),
                TokenUsage::default(),
            );
        }
        return match response.json::<Value>().await {
            Ok(reply) => {
                let tokens = usage_of(&reply);
                (Ok(reply), tokens)
            }
            Err(e) => (
                Err(Failure::Passing(format!(
                    "unreadable TypeSafe answer: {}",
                    e.without_url()
                ))),
                TokenUsage::default(),
            ),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_and_addresses_never_leave_the_computer() {
        let text = "clé sk-proj-AbCdEf123456789 et Bearer abcdefgh.ijkl, \
                    jeton ghp_1234567890abcdef, hash 0123456789abcdef0123456789abcdef01, \
                    écrire à jean.dupont@example.com";
        let sent = redact(text);
        for secret in [
            "sk-proj",
            "abcdefgh.ijkl",
            "ghp_",
            "0123456789abcdef0123",
            "jean.dupont",
        ] {
            assert!(!sent.contains(secret), "{secret} in {sent}");
        }
        assert!(sent.contains("[secret]") && sent.contains("[email]"));
        assert_eq!(redact("Appeler le plombier"), "Appeler le plombier");
    }

    #[test]
    fn a_case_is_asked_in_both_orders_with_its_opening_only() {
        let long = "mot ".repeat(400);
        let sent = request_body("Titre", &long);
        let body: Value = serde_json::from_str(&sent).unwrap();
        assert_eq!(
            body["state"]["text"].as_str().unwrap().split(' ').count(),
            300
        );
        assert_eq!(body["state"]["title"], "Titre");
        // The options in the text sent: one order, then the opposite one.
        let backward_at = sent.find(r#""backward""#).unwrap();
        let place = |nature: &str, from: usize| {
            from + sent[from..].find(&format!(r#""{nature}":"#)).unwrap()
        };
        let natures: Vec<&str> = spec().options.iter().map(|o| o.nature.as_str()).collect();
        assert_eq!(natures.len(), NoteType::ALL.len());
        for pair in natures.windows(2) {
            assert!(place(pair[0], 0) < place(pair[1], 0), "forward order");
            assert!(
                place(pair[0], backward_at) > place(pair[1], backward_at),
                "backward order"
            );
        }
        for nature in natures {
            assert!(NoteType::parse(nature).is_some(), "{nature}");
        }
    }

    #[test]
    fn an_answer_counts_only_when_both_orders_agree() {
        let reply = json!({
            "model": "jev-1.13.0",
            "answers": {
                "forward": { "choice": "decision", "confidence": 0.97 },
                "backward": { "choice": "decision", "confidence": 0.92 }
            },
            "usage": { "input_tokens": 1200, "output_tokens": 130 }
        });
        let reading = parse_answer(&reply).unwrap();
        assert_eq!(
            (reading.nature, reading.agree, reading.confidence),
            (NoteType::Decision, true, 0.92)
        );
        assert_eq!(usage_of(&reply).input_tokens, Some(1200));
        let split = json!({ "answers": {
            "forward": { "choice": "todo", "confidence": 0.9 },
            "backward": { "choice": "bug", "confidence": 0.9 }
        }});
        assert!(!parse_answer(&split).unwrap().agree);
        assert!(parse_answer(&json!({ "answers": {} })).is_err());
    }
}
