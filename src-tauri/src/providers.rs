//! AI provider registry.
//!
//! Single extension point: adding a provider = adding an entry here (or using `custom` with its
//! environment variables). No key is ever sent back to the webview: only the metadata the
//! interface needs is (`list_ai_providers`).

use serde::Serialize;
use std::path::Path;

use crate::ai_settings::KeyProvenance;

/// **Cleaning** provider (OpenAI-compatible completion API).
pub struct LlmProvider {
    pub id: &'static str,
    pub label: &'static str,
    /// Default URL; empty if the URL must come from `url_env`.
    pub base_url: &'static str,
    /// Environment variable holding the key (empty = no key required, e.g. local server).
    pub key_env: &'static str,
    pub default_model: &'static str,
    /// Environment variable that overrides the model (empty = no override).
    pub model_env: &'static str,
    /// Environment variable that provides the URL (empty = `base_url`).
    pub url_env: &'static str,
    /// `true`: a missing key is an error (otherwise a local server without a key is accepted).
    pub needs_key: bool,
    /// `true`: the server address can be entered in the application (local model, custom server).
    pub url_editable: bool,
    /// `reasoning_effort` sent with the default model when the caller gives none (`None` = the
    /// model's own default). Mistral Large 4 reasons by default and then outlasts the client's
    /// timeout on a reply; its documented values are `high` and `none`.
    pub default_effort: Option<&'static str>,
}

pub const LLM_PROVIDERS: &[LlmProvider] = &[
    LlmProvider {
        id: "cerebras",
        label: "Cerebras",
        base_url: "https://api.cerebras.ai/v1/chat/completions",
        key_env: "CEREBRAS_API_KEY",
        default_model: "gpt-oss-120b",
        model_env: "CEREBRAS_MODEL",
        url_env: "",
        needs_key: true,
        url_editable: false,
        default_effort: None,
    },
    LlmProvider {
        id: "openai",
        label: "OpenAI",
        base_url: "https://api.openai.com/v1/chat/completions",
        key_env: "OPENAI_API_KEY",
        default_model: "gpt-4o-mini",
        model_env: "OPENAI_MODEL",
        url_env: "",
        needs_key: true,
        url_editable: false,
        default_effort: None,
    },
    LlmProvider {
        id: "groq",
        label: "Groq",
        base_url: "https://api.groq.com/openai/v1/chat/completions",
        key_env: "GROQ_API_KEY",
        default_model: "llama-3.3-70b-versatile",
        model_env: "GROQ_MODEL",
        url_env: "",
        needs_key: true,
        url_editable: false,
        default_effort: None,
    },
    LlmProvider {
        id: "openrouter",
        label: "OpenRouter",
        base_url: "https://openrouter.ai/api/v1/chat/completions",
        key_env: "OPENROUTER_API_KEY",
        default_model: "openai/gpt-4o-mini",
        model_env: "OPENROUTER_MODEL",
        url_env: "",
        needs_key: true,
        url_editable: false,
        default_effort: None,
    },
    LlmProvider {
        id: "mistral",
        label: "Mistral",
        base_url: "https://api.mistral.ai/v1/chat/completions",
        key_env: "MISTRAL_API_KEY",
        default_model: "mistral-large-4",
        model_env: "MISTRAL_MODEL",
        url_env: "",
        needs_key: true,
        url_editable: false,
        default_effort: Some("none"),
    },
    LlmProvider {
        id: "ollama",
        label: "Ollama (local)",
        base_url: "http://127.0.0.1:11434/v1/chat/completions",
        key_env: "",
        default_model: "llama3.2",
        model_env: "OLLAMA_MODEL",
        url_env: "",
        needs_key: false,
        url_editable: true,
        default_effort: None,
    },
    LlmProvider {
        id: "custom",
        label: "Custom (OpenAI-compatible)",
        base_url: "",
        key_env: "CUSTOM_LLM_API_KEY",
        default_model: "",
        model_env: "CUSTOM_LLM_MODEL",
        url_env: "CUSTOM_LLM_BASE_URL",
        needs_key: false,
        url_editable: true,
        default_effort: None,
    },
];

/// **Transcription** (voice) provider. Only one adapter is implemented for now (Deepgram);
/// the registry exists for the interface and to host other adapters.
pub struct SttProvider {
    pub id: &'static str,
    pub label: &'static str,
    pub key_env: &'static str,
}

pub const STT_PROVIDERS: &[SttProvider] = &[SttProvider {
    id: "deepgram",
    label: "Deepgram",
    key_env: "DEEPGRAM_API_KEY",
}];

/// Display label of a cleaning provider: only "custom" has a translatable name, the others
/// are brand names.
fn llm_label(f: &LlmProvider) -> String {
    if f.id == "custom" {
        crate::tr!("ai.provider.custom")
    } else {
        f.label.to_string()
    }
}

/// **Reading** provider: Jev (TypeSafe) reads the natures of the cases (`crate::natures`).
pub const READING_PROVIDERS: &[SttProvider] = &[SttProvider {
    id: crate::jev::PROVIDER,
    label: "TypeSafe (Jev)",
    key_env: crate::jev::KEY_ENV,
}];

pub fn find_llm_provider(id: &str) -> Result<&'static LlmProvider, String> {
    LLM_PROVIDERS
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| crate::tr!("ai.provider.unknownCleaning", id = id))
}

pub fn find_stt_provider(id: &str) -> Result<&'static SttProvider, String> {
    STT_PROVIDERS
        .iter()
        .find(|f| f.id == id)
        .ok_or_else(|| crate::tr!("ai.provider.unknownTranscription", id = id))
}

/// Stable code that prefixes the "missing API key" error, in the form `[ai_key_missing] <text>`.
///
/// Contract with the interface (`frontend/src/utils/aiErrors.ts`): it recognizes this case by this code,
/// never by the translated text that follows. Do not rename it without updating both sides.
pub const MISSING_KEY_CODE: &str = "ai_key_missing";

/// "Missing key" error: stable code, then the translated message (name of the expected variable).
fn missing_key_error(key_env: &str) -> String {
    format!(
        "[{MISSING_KEY_CODE}] {}",
        crate::tr!("ai.provider.missingKey", name = key_env)
    )
}

/// Resolved cleaning provider (URL, optional key, model) ready for the HTTP call.
#[derive(Debug)]
pub struct ResolvedLlm {
    pub url: String,
    pub key: Option<String>,
    pub model: String,
}

/// Resolves a cleaning provider. `app_key` (key entered in the application) takes precedence over
/// the environment; `env` is injected so the function is **testable without depending on the
/// machine's real environment**.
pub fn resolve_llm_with<F: Fn(&str) -> Option<String>>(
    id: &str,
    requested_model: Option<&str>,
    app_key: Option<&str>,
    env: F,
) -> Result<ResolvedLlm, String> {
    resolve_llm_with_url(id, requested_model, app_key, None, env)
}

/// Same as `resolve_llm_with`, with the server address entered in the application (`app_url`),
/// which takes precedence over the environment for providers whose address is editable.
pub fn resolve_llm_with_url<F: Fn(&str) -> Option<String>>(
    id: &str,
    requested_model: Option<&str>,
    app_key: Option<&str>,
    app_url: Option<&str>,
    env: F,
) -> Result<ResolvedLlm, String> {
    let f = find_llm_provider(id)?;

    let url = effective_url(f, app_url, &env);
    if url.trim().is_empty() {
        let source = if f.url_env.is_empty() {
            crate::tr!("ai.provider.noUrl")
        } else {
            f.url_env.to_string()
        };
        return Err(crate::tr!(
            "ai.provider.missingUrl",
            provider = llm_label(f),
            source = source
        ));
    }

    // Plain-text URL to a non-local host: flagged (the API key would travel over unencrypted HTTP).
    // It is not refused (a server on the local network remains a legitimate use), but the
    // incident is logged.
    if url.starts_with("http://") && !is_local_host(&url) {
        eprintln!("humcase: warning — plain-text LLM URL (http://) to a non-local host: {url}");
    }

    let key = app_key
        .map(str::to_string)
        .filter(|k| !k.trim().is_empty())
        .or_else(|| {
            if f.key_env.is_empty() {
                None
            } else {
                env(f.key_env).filter(|k| !k.trim().is_empty())
            }
        });
    if key.is_none() && f.needs_key {
        return Err(missing_key_error(f.key_env));
    }

    let model = requested_model
        .map(str::to_string)
        .filter(|m| !m.trim().is_empty())
        .or_else(|| {
            if f.model_env.is_empty() {
                None
            } else {
                env(f.model_env).filter(|m| !m.trim().is_empty())
            }
        })
        .unwrap_or_else(|| f.default_model.to_string());
    if model.trim().is_empty() {
        return Err(crate::tr!(
            "ai.provider.missingModel",
            provider = llm_label(f)
        ));
    }

    Ok(ResolvedLlm { url, key, model })
}

/// Server address actually used: the application's (editable providers only), then the
/// environment variable, then the built-in default.
fn effective_url<F: Fn(&str) -> Option<String>>(
    f: &LlmProvider,
    app_url: Option<&str>,
    env: &F,
) -> String {
    if f.url_editable {
        if let Some(url) = app_url.map(str::trim).filter(|u| !u.is_empty()) {
            return url.to_string();
        }
    }
    if !f.url_env.is_empty() {
        if let Some(url) = env(f.url_env).filter(|u| !u.trim().is_empty()) {
            return url;
        }
    }
    f.base_url.to_string()
}

/// Turns an address typed by the user into the completion endpoint of an OpenAI-compatible
/// server: `http://localhost:11434` and `http://localhost:1234/v1` both become
/// `.../v1/chat/completions`. Only `http://` and `https://` are accepted.
pub fn normalize_llm_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_end_matches('/');
    let invalid = || crate::tr!("ai.provider.invalidUrl", url = input.trim());
    let (scheme, rest) = trimmed.split_once("://").ok_or_else(invalid)?;
    let scheme = scheme.to_ascii_lowercase();
    if (scheme != "http" && scheme != "https") || rest.is_empty() || rest.starts_with('/') {
        return Err(invalid());
    }
    let base = format!("{scheme}://{rest}");
    if base.ends_with("/chat/completions") {
        return Ok(base);
    }
    Ok(if rest.contains('/') {
        format!("{base}/chat/completions")
    } else {
        format!("{base}/v1/chat/completions")
    })
}

/// Model-list endpoint (`GET .../models`) next to a completion endpoint.
pub fn models_url(completion_url: &str) -> String {
    match completion_url.strip_suffix("/chat/completions") {
        Some(base) => format!("{base}/models"),
        None => format!("{}/models", completion_url.trim_end_matches('/')),
    }
}

/// True if the URL points to the local machine (`localhost`, `127.0.0.1`, `[::1]`).
fn is_local_host(url: &str) -> bool {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = if let Some(rest) = after_scheme.strip_prefix('[') {
        // IPv6 address in brackets: `[::1]:8080`.
        rest.split(']').next().unwrap_or("")
    } else {
        after_scheme.split(['/', ':', '?']).next().unwrap_or("")
    }
    .to_ascii_lowercase();
    matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1")
}

/// Resolution with the application key, then the process environment.
pub fn resolve_llm(id: &str, requested_model: Option<&str>) -> Result<ResolvedLlm, String> {
    let app_key = crate::ai_settings::app_key(id);
    let app_url = crate::ai_settings::app_url(id);
    resolve_llm_with_url(
        id,
        requested_model,
        app_key.as_deref(),
        app_url.as_deref(),
        |key| std::env::var(key).ok(),
    )
}

/// `true` if the identifier matches a known provider (cleaning, transcription or reading).
pub fn is_known_provider(id: &str) -> bool {
    find_llm_provider(id).is_ok()
        || find_stt_provider(id).is_ok()
        || READING_PROVIDERS.iter().any(|f| f.id == id)
}

/// Key of a **transcription** provider: the application's key takes precedence over the environment.
pub fn stt_key(id: &str) -> Result<String, String> {
    let f = find_stt_provider(id)?;
    let (key, _) = crate::ai_settings::effective_key(id, f.key_env, |k| std::env::var(k).ok());
    key.ok_or_else(|| missing_key_error(f.key_env))
}

/// The `reasoning_effort` to send: the caller's, or the provider's default for its default model
/// (another model may not accept that value).
pub fn effort_for<'a>(provider_id: &str, model: &str, asked: Option<&'a str>) -> Option<&'a str> {
    asked.or_else(|| {
        LLM_PROVIDERS
            .iter()
            .find(|p| p.id == provider_id && p.default_model == model)
            .and_then(|p| p.default_effort)
    })
}

/// The text of a message `content`: a string, or the list of typed chunks some providers send
/// (Mistral: `thinking` chunks, then `text` chunks); only the `text` chunks are the answer.
pub fn content_text(content: &serde_json::Value) -> String {
    match content {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(chunks) => chunks
            .iter()
            .filter(|chunk| chunk["type"] == "text")
            .filter_map(|chunk| chunk["text"].as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Extracts the cleaned text from an **OpenAI-compatible** completion response.
pub fn extract_content(data: &serde_json::Value) -> Result<String, String> {
    Some(content_text(&data["choices"][0]["message"]["content"]))
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| crate::tr!("ai.provider.emptyResponse"))
}

/// Metadata exposed to the interface (no key: only "defined?" and the provenance).
#[derive(Serialize)]
pub struct ProviderInfo {
    pub id: String,
    pub label: String,
    pub key_env: String,
    pub default_model: String,
    pub url_env: String,
    pub model_env: String,
    pub needs_key: bool,
    pub key_defined: bool,
    pub key_source: KeyProvenance,
    /// The server address can be entered in the application (local model, custom server).
    pub url_editable: bool,
    /// Server address actually used (never contains a key); empty if none is configured.
    pub url: String,
    /// Usable now: an address is known and the key is present when one is required. The model
    /// is checked at call time (it may come from the interface preferences).
    pub ready: bool,
}

#[derive(Serialize)]
pub struct ProvidersInfo {
    pub cleanup: Vec<ProviderInfo>,
    pub transcription: Vec<ProviderInfo>,
    /// Readers of the natures to check (Jev).
    pub reading: Vec<ProviderInfo>,
    /// Reason for the last incident reading `ai-config.json` (corrupted file): shown
    /// in Settings so keys are not lost silently.
    pub config_alert: Option<String>,
}

/// Builds the metadata of a cleaning provider from a key-status resolver.
fn info_llm_with<F: Fn(&str, &str) -> (bool, KeyProvenance)>(
    f: &LlmProvider,
    status: F,
    url: String,
) -> ProviderInfo {
    let (key_defined, key_source) = status(f.id, f.key_env);
    let ready = !url.trim().is_empty() && (key_defined || !f.needs_key);
    ProviderInfo {
        id: f.id.to_string(),
        label: llm_label(f),
        key_env: f.key_env.to_string(),
        default_model: f.default_model.to_string(),
        url_env: f.url_env.to_string(),
        model_env: f.model_env.to_string(),
        needs_key: f.needs_key,
        key_defined,
        key_source,
        url_editable: f.url_editable,
        url,
        ready,
    }
}

fn info_stt_with<F: Fn(&str, &str) -> (bool, KeyProvenance)>(
    f: &SttProvider,
    status: F,
) -> ProviderInfo {
    let (key_defined, key_source) = status(f.id, f.key_env);
    ProviderInfo {
        id: f.id.to_string(),
        label: f.label.to_string(),
        key_env: f.key_env.to_string(),
        default_model: String::new(),
        url_env: String::new(),
        model_env: String::new(),
        needs_key: true,
        key_defined,
        key_source,
        url_editable: false,
        url: String::new(),
        ready: key_defined,
    }
}

/// **Pure** variant: configuration path and environment access injected (tests without
/// touching the real user configuration). Visible behavior identical to `providers_info`.
pub fn providers_info_in<F: Fn(&str) -> Option<String>>(path: &Path, env: F) -> ProvidersInfo {
    // The alert may be raised DURING evaluation (corruption reported by a read, or
    // quarantine by a concurrent write): it is captured before AND after, the most recent
    // wins. An earlier alert is thus visible too (N1).
    let before = crate::ai_settings::last_config_alert();
    let cleanup = LLM_PROVIDERS
        .iter()
        .map(|f| {
            let app_url = crate::ai_settings::app_url_in(path, f.id);
            info_llm_with(
                f,
                |id, key_env| crate::ai_settings::key_status_in(path, id, key_env, &env),
                effective_url(f, app_url.as_deref(), &env),
            )
        })
        .collect();
    let transcription = STT_PROVIDERS
        .iter()
        .map(|f| {
            info_stt_with(f, |id, key_env| {
                crate::ai_settings::key_status_in(path, id, key_env, &env)
            })
        })
        .collect();
    let reading = READING_PROVIDERS
        .iter()
        .map(|f| {
            info_stt_with(f, |id, key_env| {
                crate::ai_settings::key_status_in(path, id, key_env, &env)
            })
        })
        .collect();
    let after = crate::ai_settings::last_config_alert();
    ProvidersInfo {
        cleanup,
        transcription,
        reading,
        config_alert: after.or(before),
    }
}

pub fn providers_info() -> ProvidersInfo {
    match crate::ai_settings::config_path() {
        Ok(path) => providers_info_in(&path, |k| std::env::var(k).ok()),
        // Without a resolvable path (APPDATA missing): no app key can be resolved,
        // but the environment and the already known alert remain exposed.
        Err(_) => {
            let cleanup = LLM_PROVIDERS
                .iter()
                .map(|f| {
                    info_llm_with(
                        f,
                        |id, key_env| {
                            crate::ai_settings::key_status(id, key_env, |k| std::env::var(k).ok())
                        },
                        effective_url(f, None, &|k: &str| std::env::var(k).ok()),
                    )
                })
                .collect();
            let transcription = STT_PROVIDERS
                .iter()
                .map(|f| {
                    info_stt_with(f, |id, key_env| {
                        crate::ai_settings::key_status(id, key_env, |k| std::env::var(k).ok())
                    })
                })
                .collect();
            let reading = READING_PROVIDERS
                .iter()
                .map(|f| {
                    info_stt_with(f, |id, key_env| {
                        crate::ai_settings::key_status(id, key_env, |k| std::env::var(k).ok())
                    })
                })
                .collect();
            ProvidersInfo {
                cleanup,
                transcription,
                reading,
                config_alert: crate::ai_settings::last_config_alert(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_content_made_of_chunks_gives_its_text_chunks_only() {
        let data = serde_json::json!({ "choices": [{ "message": { "content": [
            { "type": "thinking", "thinking": [{ "type": "text", "text": "reasoning" }] },
            { "type": "text", "text": "{\"a\":" },
            { "type": "text", "text": "1}" }
        ] } }] });
        assert_eq!(extract_content(&data).unwrap(), "{\"a\":1}");
        let plain = serde_json::json!({ "choices": [{ "message": { "content": "hello" } }] });
        assert_eq!(extract_content(&plain).unwrap(), "hello");
        let only_thinking = serde_json::json!({ "choices": [{ "message": { "content": [
            { "type": "thinking", "thinking": [] }] } }] });
        assert!(extract_content(&only_thinking).is_err());
    }

    fn empty_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn normalizes_typed_addresses() {
        let ok = |input: &str| normalize_llm_url(input).unwrap();
        assert_eq!(
            ok("http://localhost:11434"),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            ok(" http://localhost:1234/v1/ "),
            "http://localhost:1234/v1/chat/completions"
        );
        assert_eq!(
            ok("https://example.com/api/v1/chat/completions"),
            "https://example.com/api/v1/chat/completions"
        );
        assert_eq!(
            ok("HTTP://127.0.0.1:8080"),
            "http://127.0.0.1:8080/v1/chat/completions"
        );
        for bad in [
            "localhost:11434",
            "ftp://host",
            "http://",
            "file:///c:/x",
            "",
        ] {
            crate::i18n::with_language("en", || {
                assert!(normalize_llm_url(bad).is_err(), "{bad} should be refused");
            });
        }
        assert_eq!(
            models_url("http://h:1/v1/chat/completions"),
            "http://h:1/v1/models"
        );
    }

    /// The address entered in the application takes precedence over the environment, only for
    /// providers whose address is editable.
    #[test]
    fn app_url_precedence_only_for_editable_providers() {
        let env = |key: &str| match key {
            "CUSTOM_LLM_BASE_URL" => Some("http://env-host/v1/chat/completions".to_string()),
            "CUSTOM_LLM_MODEL" => Some("m".to_string()),
            "OPENAI_API_KEY" => Some("sk".to_string()),
            _ => None,
        };
        let app = Some("http://localhost:1234/v1/chat/completions");
        let custom = resolve_llm_with_url("custom", None, None, app, env).unwrap();
        assert_eq!(custom.url, "http://localhost:1234/v1/chat/completions");
        let custom_env = resolve_llm_with_url("custom", None, None, None, env).unwrap();
        assert_eq!(custom_env.url, "http://env-host/v1/chat/completions");
        let ollama = resolve_llm_with_url("ollama", None, None, app, empty_env).unwrap();
        assert_eq!(ollama.url, "http://localhost:1234/v1/chat/completions");
        let openai = resolve_llm_with_url("openai", None, None, app, env).unwrap();
        assert_eq!(openai.url, "https://api.openai.com/v1/chat/completions");
    }

    /// `ready` for the interface: an address is known and the key is present when required.
    #[test]
    fn ready_requires_address_and_needed_key() {
        let path = std::env::temp_dir().join(format!("humcase-ready-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let info = providers_info_in(&path, empty_env);
        let get = |id: &str| info.cleanup.iter().find(|f| f.id == id).unwrap();
        assert!(!get("openai").ready, "no key");
        assert!(get("ollama").ready, "local default address, no key needed");
        assert!(!get("custom").ready, "no address");
        assert!(get("ollama").url_editable && get("custom").url_editable);
        assert!(!get("openai").url_editable);
        let with_key = providers_info_in(&path, |k: &str| {
            (k == "OPENAI_API_KEY").then(|| "sk".to_string())
        });
        assert!(
            with_key
                .cleanup
                .iter()
                .find(|f| f.id == "openai")
                .unwrap()
                .ready
        );
    }

    #[test]
    fn resolves_openai_compatible_provider() {
        let env = |key: &str| match key {
            "OPENAI_API_KEY" => Some("sk-test".to_string()),
            "OPENAI_MODEL" => Some("gpt-4o".to_string()),
            _ => None,
        };
        let r = resolve_llm_with("openai", None, None, env).unwrap();
        assert_eq!(r.url, "https://api.openai.com/v1/chat/completions");
        assert_eq!(r.key.as_deref(), Some("sk-test"));
        assert_eq!(r.model, "gpt-4o");
    }

    #[test]
    fn app_key_takes_precedence_over_environment() {
        let env = |_: &str| Some("key-env".to_string());
        let r = resolve_llm_with("openai", None, Some("key-app"), env).unwrap();
        assert_eq!(r.key.as_deref(), Some("key-app"));

        let r = resolve_llm_with("openai", None, None, env).unwrap();
        assert_eq!(r.key.as_deref(), Some("key-env"));
    }

    #[test]
    fn requested_model_takes_precedence_over_env_var() {
        let env = |key: &str| match key {
            "OPENAI_API_KEY" => Some("sk-test".to_string()),
            "OPENAI_MODEL" => Some("gpt-4o".to_string()),
            _ => None,
        };
        let r = resolve_llm_with("openai", Some("gpt-4o-mini"), None, env).unwrap();
        assert_eq!(r.model, "gpt-4o-mini");
    }

    #[test]
    fn refuses_when_key_missing() {
        let err = crate::i18n::with_language("fr", || {
            resolve_llm_with("cerebras", None, None, empty_env).unwrap_err()
        });
        assert_eq!(err, "[ai_key_missing] CEREBRAS_API_KEY manquante");
    }

    #[test]
    fn missing_key_error_carries_stable_code_in_every_language() {
        // The interface tests this prefix, not the text: it must be identical whatever the language.
        for lang in ["fr", "en"] {
            let err = crate::i18n::with_language(lang, || {
                resolve_llm_with("openai", None, None, empty_env).unwrap_err()
            });
            assert!(
                err.starts_with("[ai_key_missing] "),
                "code missing ({lang}): {err}"
            );
        }
        assert_eq!(format!("[{MISSING_KEY_CODE}] "), "[ai_key_missing] ");
    }

    #[test]
    fn other_errors_do_not_carry_missing_key_code() {
        let err = resolve_llm_with("custom", None, Some("sk-test"), empty_env).unwrap_err();
        assert!(!err.contains(MISSING_KEY_CODE), "unexpected code: {err}");
        let err = resolve_llm_with("unknown", None, None, empty_env).unwrap_err();
        assert!(!err.contains(MISSING_KEY_CODE), "unexpected code: {err}");
    }

    #[test]
    fn local_provider_without_key_is_accepted() {
        let r = resolve_llm_with("ollama", None, None, empty_env).unwrap();
        assert_eq!(r.url, "http://127.0.0.1:11434/v1/chat/completions");
        assert!(r.key.is_none());
        assert_eq!(r.model, "llama3.2");
    }

    #[test]
    fn custom_provider_requires_url() {
        let err = resolve_llm_with("custom", None, None, empty_env).unwrap_err();
        assert!(err.contains("URL manquante"), "unexpected error: {err}");

        let env = |key: &str| match key {
            "CUSTOM_LLM_BASE_URL" => Some("http://localhost:8000/v1/chat/completions".to_string()),
            "CUSTOM_LLM_MODEL" => Some("my-model".to_string()),
            _ => None,
        };
        let r = resolve_llm_with("custom", None, None, env).unwrap();
        assert_eq!(r.url, "http://localhost:8000/v1/chat/completions");
        assert_eq!(r.model, "my-model");
    }

    #[test]
    fn unknown_provider_is_rejected() {
        assert!(resolve_llm_with("nonexistent", None, None, empty_env).is_err());
        assert!(find_stt_provider("nonexistent").is_err());
        assert_eq!(find_stt_provider("deepgram").unwrap().id, "deepgram");
        assert!(is_known_provider("cerebras"));
        assert!(is_known_provider("deepgram"));
        assert!(!is_known_provider("nonexistent"));
    }

    #[test]
    fn extracts_content_or_explains_why_unusable() {
        let valid = serde_json::json!({
            "choices": [{ "message": { "content": "Cleaned text" } }]
        });
        assert_eq!(extract_content(&valid).unwrap(), "Cleaned text");

        let empty = serde_json::json!({ "choices": [{ "message": { "content": "   " } }] });
        assert!(extract_content(&empty).is_err());

        let no_choices = serde_json::json!({ "error": "boom" });
        assert!(extract_content(&no_choices).is_err());
    }

    /// S1 — detection of local hosts for the plain "http://" warning.
    #[test]
    fn is_local_host_recognizes_local_hosts() {
        assert!(is_local_host("http://127.0.0.1:11434/v1/chat/completions"));
        assert!(is_local_host("http://localhost:8080"));
        assert!(is_local_host("http://[::1]:8080"));
        assert!(!is_local_host("http://192.168.1.10/v1"));
        assert!(!is_local_host("https://api.example.com/v1"));
    }

    #[test]
    fn exposed_metadata_contain_no_key() {
        let info = providers_info();
        assert!(info.cleanup.iter().any(|f| f.id == "cerebras"));
        assert!(info.transcription.iter().any(|f| f.id == "deepgram"));
        // The NAME of the environment variable and a boolean are exposed, never a key value.
        assert!(info.cleanup.iter().all(|f| !f.key_env.contains("sk-")));
        assert!(info.cleanup.iter().all(|f| matches!(
            f.key_source,
            KeyProvenance::App | KeyProvenance::Env | KeyProvenance::None
        )));
    }

    #[test]
    fn mistral_large_4_does_not_reason_unless_asked() {
        assert_eq!(effort_for("mistral", "mistral-large-4", None), Some("none"));
        assert_eq!(
            effort_for("mistral", "mistral-large-4", Some("high")),
            Some("high")
        );
        // Another model may refuse the value: nothing is sent for it.
        assert_eq!(effort_for("mistral", "mistral-small-latest", None), None);
        assert_eq!(effort_for("cerebras", "gpt-oss-120b", None), None);
        assert_eq!(
            effort_for("cerebras", "gpt-oss-120b", Some("low")),
            Some("low")
        );
    }
}
