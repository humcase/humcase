// French texts: one file per interface zone (top-level key = file name).
// `engine.json` is not here: Rust embeds it (`src-tauri/src/i18n.rs`), the interface never reads it.
import common from './common.json';
import editor from './editor.json';
import notes from './notes.json';
import quickCapture from './quickCapture.json';
import settings from './settings.json';
import trash from './trash.json';
import workspace from './workspace.json';

export default { common, editor, notes, quickCapture, settings, trash, workspace };
