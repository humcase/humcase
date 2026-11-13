/**
 * Errors from the AI commands on the Rust side (`clean_transcript`, `integrate_dictation`, `structure_note`,
 * `start_transcription`).
 *
 * Contract: a missing API key returns a message prefixed with a stable code,
 * `[ai_key_missing] <translated text>` (`MISSING_KEY_CODE` in `src-tauri/src/providers.rs`).
 * The interface recognizes this case by the code, never by the text, which depends on the language.
 */
export const MISSING_KEY_CODE = 'ai_key_missing';

const MISSING_KEY_PREFIX = `[${MISSING_KEY_CODE}] `;

/** True if the error reports a missing API key (stable code, independent of the language). */
export function isMissingKeyError(error: unknown): boolean {
  return String(error).startsWith(MISSING_KEY_PREFIX);
}

/** Error text to display: the technical code, if any, is removed; the translated message stays. */
export function aiErrorMessage(error: unknown): string {
  const message = String(error);
  return message.startsWith(MISSING_KEY_PREFIX) ? message.slice(MISSING_KEY_PREFIX.length) : message;
}
