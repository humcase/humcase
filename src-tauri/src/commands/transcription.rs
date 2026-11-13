use crate::providers;
use futures_util::{SinkExt, StreamExt};
use tauri::Emitter;
use tauri::Manager;
use tauri::State;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

/// Deepgram model of the dictation, also written in the consumption journal. Nova-3 covers the
/// launch languages and, with `language=multi`, several languages in one dictation
/// (developers.deepgram.com/docs/models-languages-overview, read on 2026-10-03).
const DEEPGRAM_MODEL: &str = "nova-3";

/// Deepgram language code for a dictation language: the launch languages as such, `multi` for
/// several languages in one dictation, and `multi` too for a language outside this list, since a
/// wrong fixed language returns nothing at all (an English dictation sent as `fr` came back empty).
pub fn deepgram_language(requested: &str) -> &'static str {
    let code = requested.trim().to_ascii_lowercase();
    let base = code.split(['-', '_']).next().unwrap_or("");
    match base {
        "fr" => "fr",
        "en" => "en",
        "es" => "es",
        "de" => "de",
        "it" => "it",
        "pt" if code == "pt" || code == "pt-pt" || code == "pt_pt" => "pt",
        "pt" => "pt-BR",
        _ => "multi",
    }
}

/// Internal command of the transcription proxy (sent to the Deepgram WebSocket writer).
pub enum AudioCmd {
    Chunk(Vec<u8>),
    Stop,
}

/// Active transcription session: generation identifier + sender to the writer.
pub struct TranscriptionSession {
    pub id: u64,
    pub tx: tokio::sync::mpsc::Sender<AudioCmd>,
}

/// State of the transcription proxy: one session at a time. The sender feeds the writer.
///
/// Each session carries a **generation identifier**. The reader of a finished session must only
/// free the slot if it *still* holds its own session: otherwise, a `stop` followed by a
/// new start would see the old reader overwrite the new one's slot (dictation 2
/// would stay alive but become unusable — violation of dictation safeguard no. 3).
#[derive(Default, Clone)]
pub struct TranscriptionState {
    pub slot: std::sync::Arc<tokio::sync::Mutex<Option<TranscriptionSession>>>,
    pub next_id: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl TranscriptionState {
    /// Frees the slot **only if it still holds session `id`**.
    /// Returns true if the slot was actually freed by this call.
    pub async fn release_if_ours(&self, id: u64) -> bool {
        let mut slot = self.slot.lock().await;
        if slot.as_ref().is_some_and(|s| s.id == id) {
            *slot = None;
            true
        } else {
            false
        }
    }

    /// **Cleanly** closes (`CloseStream`) session `id` if it is still the current one.
    /// Returns true if a close was requested. Used by the session duration cap.
    pub async fn close_if_ours(&self, id: u64) -> bool {
        // Take the sender **outside** the lock: `send().await` must not hold it.
        let to_stop = {
            let mut slot = self.slot.lock().await;
            if slot.as_ref().is_some_and(|s| s.id == id) {
                slot.take()
            } else {
                None
            }
        };
        match to_stop {
            Some(session) => {
                let _ = session.tx.send(AudioCmd::Stop).await;
                true
            }
            None => false,
        }
    }
}

/// Duration cap of a dictation session. A real dictation lasts from a few seconds to a few
/// minutes; 15 min leaves a very wide margin while cleanly closing an orphaned session
/// (webview gone, network stuck) — never a raw `close()`, so nothing is lost.
const MAX_DICTATION_SESSION: std::time::Duration = std::time::Duration::from_secs(15 * 60);

/// Opens the Deepgram connection **on the Rust side** and returns transcriptions as Tauri events.
///
/// The key is read from the process environment (never sent back to the webview). The frontend sends
/// audio through `send_audio_chunk` and closes with `stop_transcription` (`CloseStream`, so the
/// service processes the remaining audio before closing). A second session is refused while the
/// first is active.
#[tauri::command]
pub async fn start_transcription(
    app: tauri::AppHandle,
    state: State<'_, TranscriptionState>,
    provider: Option<String>,
    language: Option<String>,
) -> Result<(), String> {
    let provider_id = provider.unwrap_or_else(|| "deepgram".to_string());
    // Without a language from the interface, the interface language of the engine.
    let language = deepgram_language(language.as_deref().unwrap_or(crate::i18n::language()));
    let key = providers::stt_key(&provider_id)?;

    // Reserve the slot **before** opening the connection: a second concurrent start is refused.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<AudioCmd>(64);
    let session_id = state
        .next_id
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        + 1;
    {
        let mut slot = state.slot.lock().await;
        if slot.is_some() {
            return Err(crate::tr!("ai.transcription.inProgress"));
        }
        *slot = Some(TranscriptionSession { id: session_id, tx });
    }

    let url = format!("wss://api.deepgram.com/v1/listen?model={DEEPGRAM_MODEL}&language={language}&smart_format=true&interim_results=true");
    let mut request = url
        .into_client_request()
        .map_err(|e| crate::tr!("ai.transcription.badRequest", error = e))?;
    request.headers_mut().insert(
        "Sec-WebSocket-Protocol",
        format!("token, {key}")
            .parse()
            .map_err(|_| crate::tr!("ai.transcription.badAuthHeader"))?,
    );

    // Bounded connection timeout: without it, a network that leaves the SYN hanging would block
    // `start_transcription` indefinitely (slot reserved, dictation unusable until restart).
    let connection = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(request),
    )
    .await;
    let (ws, _) = match connection {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            // Connection failure: free the reserved slot (our own), otherwise any new
            // start would be refused.
            state.release_if_ours(session_id).await;
            return Err(crate::tr!("ai.transcription.connectFailed", error = e));
        }
        Err(_) => {
            state.release_if_ours(session_id).await;
            return Err(crate::tr!("ai.transcription.connectTimeout"));
        }
    };
    let (mut write, mut read) = ws.split();

    // "Reading is finished" signal: the writer waits for it after `CloseStream` to let the
    // server send its last transcriptions before closing the connection.
    let (read_done_tx, read_done_rx) = tokio::sync::oneshot::channel::<()>();

    // Writer: sends the audio then CloseStream, in the order the user produces them.
    tokio::spawn(async move {
        while let Some(cmd) = rx.recv().await {
            match cmd {
                AudioCmd::Chunk(bytes) => {
                    if write.send(Message::Binary(bytes.into())).await.is_err() {
                        break;
                    }
                }
                AudioCmd::Stop => {
                    let _ = write
                        .send(Message::Text("{\"type\":\"CloseStream\"}".into()))
                        .await;
                    // Wait for the end on the server side (last transcriptions), with a safety
                    // timeout so it never stays blocked.
                    let _ =
                        tokio::time::timeout(std::time::Duration::from_secs(5), read_done_rx).await;
                    let _ = write.close().await;
                    break;
                }
            }
        }
    });

    // Reader: each message goes to the window, then the closing is signaled. The audio duration
    // the provider reports is recorded in the consumption journal when the session ends.
    let app_reader = app.clone();
    let state_reader = state.inner().clone();
    let db_path = app.state::<super::AppState>().db_path.clone();
    let started = std::time::Instant::now();
    tokio::spawn(async move {
        let mut meter = crate::ai_usage::AudioMeter::default();
        while let Some(message) = read.next().await {
            match message {
                Ok(Message::Text(text)) => {
                    meter.read(&text);
                    let _ = app_reader.emit("transcription://message", text.to_string());
                }
                Ok(Message::Close(_)) | Err(_) => break,
                _ => {}
            }
        }
        let seconds = meter.seconds();
        crate::ai_usage::record(
            &db_path,
            &crate::ai_usage::UsageEntry {
                operation: crate::ai_usage::operation::TRANSCRIPTION,
                provider: &provider_id,
                model: DEEPGRAM_MODEL,
                language: Some(language),
                tokens: Default::default(),
                audio_seconds: Some(seconds),
                duration_ms: started.elapsed().as_millis() as u64,
                ok: true,
                estimated_cost_usd: crate::ai_usage::audio_price_per_minute(&provider_id, language)
                    .map(|price| price * seconds / 60.0),
                prices_as_of: crate::ai_usage::PRICES_AS_OF,
            },
        );
        // The session ends: only free the slot if it holds **this session**.
        // A reader of an old session must never overwrite the slot of a newer one.
        state_reader.release_if_ours(session_id).await;
        let _ = read_done_tx.send(());
        let _ = app_reader.emit("transcription://closed", ());
    });

    // Duration cap: at the deadline, **cleanly** close the session (CloseStream through the writer,
    // which waits for the last transcriptions) if it is still ours. Safeguard against an
    // orphaned session (webview gone, network stuck) that would stay open indefinitely.
    let state_timer = state.inner().clone();
    tokio::spawn(async move {
        tokio::time::sleep(MAX_DICTATION_SESSION).await;
        state_timer.close_if_ours(session_id).await;
    });

    Ok(())
}

/// Sends an audio fragment (webm/opus) to the Deepgram writer.
#[tauri::command]
pub async fn send_audio_chunk(
    state: State<'_, TranscriptionState>,
    bytes: Vec<u8>,
) -> Result<(), String> {
    // Clone the sender **outside** the lock: waiting on a full channel (slowed writer) must not
    // hold the mutex and delay `stop_transcription`.
    let tx = {
        let slot = state.slot.lock().await;
        slot.as_ref()
            .map(|s| s.tx.clone())
            .ok_or_else(|| crate::tr!("ai.transcription.notRunning"))?
    };
    tx.send(AudioCmd::Chunk(bytes))
        .await
        .map_err(|_| crate::tr!("ai.transcription.interrupted"))
}

/// Requests the clean closing of the transcription session.
#[tauri::command]
pub async fn stop_transcription(state: State<'_, TranscriptionState>) -> Result<(), String> {
    let mut slot = state.slot.lock().await;
    if let Some(session) = slot.take() {
        let _ = session.tx.send(AudioCmd::Stop).await;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_session(id: u64) -> (TranscriptionSession, tokio::sync::mpsc::Receiver<AudioCmd>) {
        let (tx, rx) = tokio::sync::mpsc::channel(4);
        (TranscriptionSession { id, tx }, rx)
    }

    /// The dictation follows the language asked, never a fixed French: an English dictation sent
    /// to Deepgram as `fr` came back empty (checked on a real stream, 2026-10-03).
    #[test]
    fn the_dictation_language_is_the_one_asked() {
        assert_eq!(deepgram_language("en"), "en");
        assert_eq!(deepgram_language("en-US"), "en");
        assert_eq!(deepgram_language("FR-ca"), "fr");
        assert_eq!(deepgram_language("es"), "es");
        assert_eq!(deepgram_language("de"), "de");
        assert_eq!(deepgram_language("it"), "it");
        assert_eq!(deepgram_language("pt-BR"), "pt-BR");
        assert_eq!(deepgram_language("pt"), "pt");
        assert_eq!(deepgram_language("multi"), "multi");
        assert_eq!(deepgram_language("nl"), "multi");
        assert_eq!(deepgram_language(""), "multi");
        assert_eq!(
            crate::ai_usage::audio_price_per_minute("deepgram", "multi"),
            Some(0.0092)
        );
        assert_eq!(
            crate::ai_usage::audio_price_per_minute("deepgram", "en"),
            Some(0.0077)
        );
    }

    /// Audit 4 #6 — the reader of a finished session must only free the slot if it still holds
    /// **its own** session. Otherwise, a `stop` followed by an immediate restart would see
    /// the old reader clear the new session's slot (dictation 2 unusable).
    #[tokio::test]
    async fn test_release_slot_does_not_overwrite_new_session() {
        let state = TranscriptionState::default();

        // Session 1 reserved, then stopped: `stop_transcription` removes the slot.
        let (s1, _rx1) = test_session(1);
        *state.slot.lock().await = Some(s1);
        assert!(
            state.release_if_ours(1).await,
            "session 1 must be able to free its slot"
        );
        assert!(state.slot.lock().await.is_none());

        // Immediate restart: session 2 occupies the slot.
        let (s2, mut rx2) = test_session(2);
        *state.slot.lock().await = Some(s2);

        // The old reader (session 1) finishes and tries to free: it must NOT overwrite
        // session 2's slot.
        assert!(
            !state.release_if_ours(1).await,
            "an old reader must not free the slot"
        );
        {
            let slot = state.slot.lock().await;
            assert_eq!(
                slot.as_ref().map(|s| s.id),
                Some(2),
                "session 2 must stay in place"
            );
        }

        // Session 2 stays usable: a chunk is accepted...
        let tx = state.slot.lock().await.as_ref().unwrap().tx.clone();
        tx.send(AudioCmd::Chunk(vec![1, 2, 3])).await.unwrap();
        assert!(matches!(rx2.recv().await, Some(AudioCmd::Chunk(_))));

        // ...and can be cleanly closed (`take` does remove session 2).
        let taken = state.slot.lock().await.take();
        assert_eq!(taken.map(|s| s.id), Some(2));
        assert!(state.slot.lock().await.is_none());
    }

    /// P2-R2#4 — the duration cap **cleanly** closes (Stop/CloseStream) the session, and
    /// only if it is still its own.
    #[tokio::test]
    async fn test_cap_closes_cleanly_and_only_its_own() {
        let state = TranscriptionState::default();
        let (s1, mut rx1) = test_session(1);
        *state.slot.lock().await = Some(s1);

        // The deadline of another session must close nothing.
        assert!(!state.close_if_ours(0).await);
        assert_eq!(state.slot.lock().await.as_ref().map(|s| s.id), Some(1));

        // The deadline of the active session requests a Stop (clean close) and frees the slot.
        assert!(state.close_if_ours(1).await);
        assert!(matches!(rx1.recv().await, Some(AudioCmd::Stop)));
        assert!(state.slot.lock().await.is_none());
    }
}
