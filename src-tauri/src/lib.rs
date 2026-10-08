pub mod agent_hooks;
pub mod agents_seen;
pub mod ai_settings;
pub mod ai_usage;
pub mod ask;
pub mod backup;
pub mod commands;
pub mod common_words;
pub mod data_location;
pub mod data_migration;
pub mod dates;
pub mod db;
pub mod dictation_journal;
pub mod gather;
pub mod germinate;
pub mod i18n;
pub mod images;
pub mod jev;
pub mod mcp;
pub mod meaning;
pub mod models;
pub mod natures;
pub mod process_lease;
pub mod profile;
pub mod providers;
pub mod recognize;
pub mod reflect;
pub mod test_profile;
pub mod tidy;

use commands::{AppState, MutationGate};
use process_lease::ProcessLease;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // System language until the interface passes its own (`set_ui_language`):
    // startup dialogs are already shown in the right language.
    if let Some(locale) = sys_locale::get_locale() {
        i18n::set_language(&locale);
    }
    // Data folder `~/.humcase`, moved out of AppData before anything opens it: without a
    // verified copy, the application would open an empty database in the new folder.
    let data_dir = match data_location::prepare_on_startup(data_location::Process::Interface) {
        Ok(dir) => dir,
        Err(error) => {
            eprintln!("humcase: data migration failed: {error}");
            show_startup_error(&crate::tr!("app.dataMigrationFailed", error = error));
            std::process::exit(1);
        }
    };
    let window_state_file = data_dir.join(data_location::WINDOW_STATE_FILE);
    let setup_data_dir = data_dir.clone();
    let mut builder = tauri::Builder::default();
    // Single interface: relaunching Humcase brings the existing window to the front instead of
    // opening a second process. First plugin registered, so the second instance stops before
    // anything else. The MCP server (`Humcase.exe mcp`) does not go through here and remains
    // multi-process; an isolated test instance (debug) can run alongside.
    if !test_profile::is_requested() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }));
    }
    builder
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // Remembers window size, position and maximized state between launches.
        // Deliberately without VISIBLE: the quick capture window must stay hidden
        // until the shortcut, otherwise the plugin reopens it at startup.
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                // Absolute path: kept in the data folder rather than in AppData.
                .with_filename(window_state_file.to_string_lossy())
                .build(),
        )
        .setup(move |app| {
            // The application state is registered **as early as possible**: a command received
            // before `app.manage` would fail with "state not managed". Observed once during a
            // startup under heavy contention (kill/relaunch burst); the window is reduced to
            // a minimum by managing the state before any fallible step (tray icon, menu...).
            let app_dir = setup_data_dir.clone();
            let lease = ProcessLease::acquire_shared(&app_dir).map_err(|e| {
                crate::tr!(
                    "app.sessionUnavailable",
                    path = app_dir.display(),
                    error = e
                )
            })?;
            let conn = db::init_db(&app_dir).map_err(|e| {
                crate::tr!(
                    "app.databaseUnavailable",
                    path = app_dir.display(),
                    error = e
                )
            })?;
            // Everything the interface writes is the person's (journal of who did what, v24).
            db::journal::install(&conn, &db::journal::Actor::Human)?;
            // Trash: notes deleted more than 30 days ago are erased when the interface starts
            // (never by the MCP or the CLI). A failure does not prevent startup.
            match db::queries::purge_trash(
                &conn,
                db::queries::TrashPurge::OlderThanDays(db::queries::TRASH_RETENTION_DAYS),
            ) {
                Ok(0) => {}
                Ok(n) => eprintln!(
                    "humcase: trash, {n} note(s) older than {} days erased",
                    db::queries::TRASH_RETENTION_DAYS
                ),
                Err(e) => eprintln!("humcase: unable to purge the trash: {e}"),
            }
            // Empty cases left behind (crash, window closed on a new case) go to the trash.
            match db::queries::trash_empty_cases(&conn, db::queries::EMPTY_CASE_GRACE_MINUTES) {
                Ok(0) => {}
                Ok(n) => eprintln!("humcase: {n} empty case(s) placed in the trash"),
                Err(e) => eprintln!("humcase: unable to tidy empty cases: {e}"),
            }
            let db_path = app_dir.join(db::DB_FILE);
            let db_path_for_meaning = db_path.clone();
            let db_path_for_natures = db_path.clone();
            app.manage(Mutex::new(lease));
            app.manage(AppState {
                db: Mutex::new(conn),
                db_path,
                mutation_gate: std::sync::RwLock::new(MutationGate::Open),
            });
            app.manage(commands::TranscriptionState::default());
            // Search by meaning: the cases are read in the background, by the worker.
            let meaning_state = meaning::MeaningState::default();
            meaning::start_reader(db_path_for_meaning, app_dir.clone(), &meaning_state);
            app.manage(meaning_state);
            // Natures to check: Jev reads the cases in the background, when the person turned
            // the review on and a TypeSafe key is set.
            let nature_state = natures::NatureState::default();
            natures::start_reader(db_path_for_natures, app_dir.clone(), &nature_state);
            app.manage(nature_state);

            // Pasted images are served from the data folder only.
            app.asset_protocol_scope()
                .allow_directory(app_dir.join(data_location::IMAGES_DIR), false)
                .map_err(|e| e.to_string())?;
            // Windows are created here rather than from the configuration (`create: false`) so
            // that their WebView2 profile (local storage, including the draft escrow) lives in the
            // data folder, not in AppData. Both windows share it.
            let webview_dir = app_dir.join(data_location::WEBVIEW_DIR);
            for config in app.config().app.windows.clone() {
                tauri::WebviewWindowBuilder::from_config(app.handle(), &config)?
                    .data_directory(webview_dir.clone())
                    .build()?;
            }

            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            let menu = tray_menu(app)?;

            let mut tray_builder = TrayIconBuilder::with_id(TRAY_ID)
                .menu(&menu)
                // Isolated test profile: distinct name, so it is not mistaken for the real application.
                .tooltip(if test_profile::is_requested() {
                    "Humcase (test)"
                } else {
                    "Humcase"
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => show_main_window(app),
                    "quick_capture" => {
                        // The global shortcut was invisible from the tray.
                        if let Some(window) = app.get_webview_window("quick_capture") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let is_visible = window.is_visible().unwrap_or(false);
                            if is_visible {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                });

            // The window icon is optional: do not panic at startup if it is missing.
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let _tray = tray_builder.build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_projects,
            commands::notes_change_marker,
            commands::integrate_dictation,
            commands::get_images_dir,
            commands::create_project,
            commands::update_project_space,
            commands::rename_project,
            commands::set_project_color,
            commands::set_project_archived,
            commands::delete_project,
            commands::set_note_archived,
            commands::get_activities,
            commands::create_activity,
            commands::rename_activity,
            commands::delete_activity,
            commands::set_project_activities,
            commands::get_or_create_inbox,
            commands::move_note_to_project,
            commands::get_notes,
            commands::get_note_by_id,
            commands::get_mcp_permissions,
            commands::set_mcp_allow_note_edit,
            commands::get_note_history,
            commands::list_agent_revisions_since,
            commands::get_brief,
            commands::list_agent_events_since,
            commands::mark_brief_seen,
            commands::mark_case_seen,
            commands::get_case_journal,
            commands::review_case,
            commands::create_note,
            commands::update_note,
            commands::retry_note_update,
            commands::delete_note,
            commands::trash_if_empty,
            commands::get_trashed_notes,
            commands::restore_from_trash,
            commands::purge_note,
            commands::empty_trash,
            commands::get_note_relations,
            commands::create_linked_task,
            commands::link_note_task,
            commands::set_ui_language,
            commands::unlink_note_task,
            commands::get_case_context,
            commands::create_sub_case,
            commands::open_link,
            commands::export_json_canvas,
            commands::import_json_canvas,
            commands::move_case,
            commands::get_case_positions,
            commands::set_case_position,
            commands::set_case_positions,
            commands::append_note_content,
            commands::append_raw_content,
            commands::save_image_to_disk,
            commands::get_daily_standup,
            commands::find_similar_notes,
            commands::recognize_case,
            commands::meaning_status,
            commands::set_meaning_enabled,
            commands::search_by_meaning,
            commands::nature_review,
            commands::answer_nature,
            commands::germinate_case,
            commands::ask_cases,
            commands::keep_answer,
            commands::reflect_reply,
            commands::reflect_summarize,
            commands::keep_reflection,
            commands::list_proposals,
            commands::answer_proposals,
            commands::undo_proposals,
            commands::tidy_list,
            commands::tidy_check_decision,
            commands::answer_tidy,
            commands::undo_tidy,
            commands::case_supersession,
            commands::keep_decision_in_force,
            commands::superseded_ids,
            commands::case_conflicts,
            commands::project_summary,
            commands::work_profile,
            commands::open_work_profile,
            commands::answer_profile,
            commands::undo_profile,
            commands::nudges,
            commands::answer_nudge,
            commands::profile_settings,
            commands::set_profile_settings,
            commands::tidy_settings,
            commands::set_tidy_settings,
            commands::nature_reading_status,
            commands::set_nature_reading,
            commands::recheck_nature_reading,
            commands::open_data_dir,
            commands::backup_database,
            commands::create_backup_archive,
            commands::inspect_backup_archive,
            commands::restore_backup_archive,
            commands::get_handoff,
            commands::clean_transcript,
            commands::structure_note,
            commands::apply_structured_note,
            commands::list_ai_providers,
            commands::read_ai_usage,
            commands::list_agents_seen,
            commands::humcase_executable_path,
            commands::set_ai_key,
            commands::set_ai_url,
            commands::test_ai_connection,
            commands::start_transcription,
            commands::send_audio_chunk,
            commands::stop_transcription
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("humcase: runtime error: {e}");
            // The graphical executable has no console: without a message, a failed startup (most
            // often a second instance whose WebView2 folder is already taken) went unnoticed.
            show_startup_error(&crate::tr!("app.startFailed", error = e));
            std::process::exit(1);
        });
}

/// Identifier of the notification area icon: its menu is rebuilt when the language changes.
pub(crate) const TRAY_ID: &str = "main";

/// Notification icon menu, in the engine's current language.
pub(crate) fn tray_menu<R: tauri::Runtime, M: tauri::Manager<R>>(
    manager: &M,
) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, MenuItem};
    let quit_i = MenuItem::with_id(manager, "quit", crate::tr!("tray.quit"), true, None::<&str>)?;
    let show_i = MenuItem::with_id(manager, "show", crate::tr!("tray.show"), true, None::<&str>)?;
    let quick_i = MenuItem::with_id(
        manager,
        "quick_capture",
        crate::tr!("tray.quickCapture"),
        true,
        None::<&str>,
    )?;
    Menu::with_items(manager, &[&show_i, &quick_i, &quit_i])
}

/// Brings the main window to the front, even hidden in the notification area or minimized.
fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Native message box for a startup error (Windows only, no dependency).
#[cfg(windows)]
fn show_startup_error(message: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut core::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            kind: u32,
        ) -> i32;
    }
    const MB_ICONERROR: u32 = 0x10;
    let wide = |s: &str| {
        OsStr::new(s)
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<u16>>()
    };
    let (text, caption) = (wide(message), wide("Humcase"));
    // SAFETY: zero-terminated UTF-16 strings, alive during the call; null parent window.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_ICONERROR,
        );
    }
}

/// Startup error outside Windows: always on stderr (terminal, journal), plus a best-effort
/// dialog through `zenity` or `kdialog` when one is installed and a graphical session exists.
/// Both are optional: nothing here may fail or block the exit.
#[cfg(not(windows))]
fn show_startup_error(message: &str) {
    eprintln!("Humcase: {message}");
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        return;
    }
    use std::process::{Command, Stdio};
    let dialogs: [(&str, &[&str]); 2] = [
        (
            "zenity",
            &["--error", "--title=Humcase", "--no-markup", "--text"],
        ),
        ("kdialog", &["--title", "Humcase", "--error"]),
    ];
    for (program, args) in dialogs {
        let shown = Command::new(program)
            .args(args)
            .arg(message)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if shown.is_ok() {
            return;
        }
    }
}
