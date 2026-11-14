//! Pulse — launcher globale istantaneo + clipboard manager.
//!
//! Filo conduttore:
//!   - Thread 1 (main):     event loop Tauri, finestra Spotlight, tray, hotkey globale
//!   - Thread 2 (indexer):  scansione iniziale home + app installate → indice in memoria (RwLock)
//!   - Thread 3 (watcher):  notify sugli eventi filesystem → update incrementale dell'indice
//!   - Thread 4 (clipboard): listener 450 ms → SQLite (WAL) + file immagini in cache locale
//!
//! La UI (vanilla HTML/CSS/JS in ../ui) comunica SOLO via comandi IPC tipizzati.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod db;
mod indexer;
mod paste;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use parking_lot::{Mutex, RwLock};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

const GLOBAL_SHORTCUT: &str = "Alt+Space";
const ICON_BYTES: &[u8] = include_bytes!("../icons/icon.png");
const MAX_RESULTS: usize = 40;

// ---------------------------------------------------------------------------
// Stato condiviso
// ---------------------------------------------------------------------------

pub struct AppState {
    db: Arc<Mutex<db::Db>>,
    index: indexer::ArcRwLockIndex,
    frecency: Arc<RwLock<HashMap<String, (u64, i64)>>>,
    privacy_mode: Arc<Mutex<bool>>,
    images_dir: PathBuf,
}

#[derive(Serialize)]
struct SettingsOut {
    privacy_mode: bool,
    global_shortcut: String,
    index_count: usize,
}

#[derive(Serialize)]
struct ClipboardOut {
    id: i64,
    kind: String,
    content: String,
    preview: String,
    image: Option<String>, // data URL PNG (thumbnail)
    created_at: i64,
    pinned: bool,
    bytes: i64,
}

// ---------------------------------------------------------------------------
// Comandi IPC — ricerca
// ---------------------------------------------------------------------------

#[tauri::command]
fn search(
    state: tauri::State<'_, AppState>,
    query: String,
    mode: String,
) -> Vec<indexer::SearchResult> {
    let items = state.index.read();
    let frecency = state.frecency.read();
    let mut results = indexer::search(&items, &frecency, &query, MAX_RESULTS * 2);
    results.retain(|r| match mode.as_str() {
        "apps" => r.kind == "app",
        "files" => r.kind == "file" || r.kind == "dir",
        _ => true,
    });
    results.truncate(MAX_RESULTS);
    results
}

#[tauri::command]
fn open_item(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    kind: String,
    exec: Option<String>,
) -> Result<(), String> {
    // Aggiorna frecency (frequenza + recenza) prima di lanciare
    let updated = state.db.lock().record_use(&path);
    state.frecency.write().insert(path.clone(), updated);

    match (kind.as_str(), exec.as_deref()) {
        ("app", Some(cmd)) if !cmd.trim().is_empty() => spawn_exec(cmd)?,
        _ => tauri_plugin_opener::open_path(&path, None::<&str>).map_err(|e| e.to_string())?,
    }

    hide_main_window(&app);
    Ok(())
}

#[tauri::command]
fn reveal_item(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn copy_path(path: String) -> Result<(), String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    cb.set_text(path).map_err(|e| e.to_string())
}

/// Anteprima testuale: primi 8 KB, con guardia anti-binari.
#[tauri::command]
fn read_preview(path: String) -> String {
    let meta = match std::fs::metadata(&path) {
        Ok(m) => m,
        Err(_) => return "(file non trovato)".to_string(),
    };
    if meta.is_dir() {
        return format!("(cartella)\n{}", path);
    }
    if meta.len() > 2_000_000 {
        return "(file oltre 2 MB — anteprima non disponibile)".to_string();
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return "(lettura fallita)".to_string(),
    };
    let head = &bytes[..bytes.len().min(8192)];
    let nuls = head.iter().filter(|b| **b == 0).count();
    if nuls > head.len() / 20 + 1 {
        return "(file binario — anteprima non disponibile)".to_string();
    }
    String::from_utf8_lossy(head).into_owned()
}

// ---------------------------------------------------------------------------
// Comandi IPC — clipboard
// ---------------------------------------------------------------------------

#[tauri::command]
fn list_clipboard(
    state: tauri::State<'_, AppState>,
    query: Option<String>,
    limit: Option<i64>,
) -> Vec<ClipboardOut> {
    let q = query.unwrap_or_default();
    let lim = limit.unwrap_or(60).clamp(1, 200);
    state
        .db
        .lock()
        .list_clipboard(&q, lim)
        .into_iter()
        .map(|item| clip_out(&state.images_dir, item))
        .collect()
}

#[tauri::command]
fn paste_clipboard(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<String, String> {
    let item = state
        .db
        .lock()
        .clipboard_full(id)
        .ok_or_else(|| "elemento non trovato".to_string())?;

    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    match item.kind.as_str() {
        "image" => {
            let rel = item.image_rel.clone().ok_or("immagine mancante")?;
            let full = state.images_dir.join(&rel);
            let img = image::open(&full).map_err(|e| format!("decode immagine: {e}"))?;
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            cb.set_image(arboard::ImageData {
                width: w as usize,
                height: h as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            })
            .map_err(|e| e.to_string())?;
        }
        _ => {
            let text = item.content.clone().unwrap_or_default();
            cb.set_text(text).map_err(|e| e.to_string())?;
        }
    }

    hide_main_window(&app);

    // Attendi che la finestra si nasconda e il focus torni all'app sottostante,
    // poi simula Ctrl+V. Il messaggio d'errore (se c'è) viene notificato via evento.
    let app2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(220));
        match paste::paste_into_active_window() {
            Ok(()) => {}
            Err(msg) => {
                let _ = app2.emit("toast", msg);
            }
        }
    });
    Ok("ok".to_string())
}

#[tauri::command]
fn delete_clipboard_item(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let rel = state
        .db
        .lock()
        .delete_clipboard(id)
        .map_err(|e| e.to_string())?;
    if let Some(rel) = rel {
        let _ = std::fs::remove_file(state.images_dir.join(&rel));
        let _ = std::fs::remove_file(state.images_dir.join(thumb_of(&rel)));
    }
    Ok(())
}

#[tauri::command]
fn clear_clipboard(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let (n, rels) = state
        .db
        .lock()
        .clear_clipboard()
        .map_err(|e| e.to_string())?;
    for rel in rels {
        let _ = std::fs::remove_file(state.images_dir.join(&rel));
        let _ = std::fs::remove_file(state.images_dir.join(thumb_of(&rel)));
    }
    Ok(n)
}

#[tauri::command]
fn toggle_pin(state: tauri::State<'_, AppState>, id: i64) -> Result<bool, String> {
    state.db.lock().toggle_pin(id).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Comandi IPC — settings / finestra
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> SettingsOut {
    SettingsOut {
        privacy_mode: *state.privacy_mode.lock(),
        global_shortcut: GLOBAL_SHORTCUT.to_string(),
        index_count: state.index.read().len(),
    }
}

#[tauri::command]
fn set_privacy_mode(state: tauri::State<'_, AppState>, enabled: bool) {
    *state.privacy_mode.lock() = enabled;
}

#[tauri::command]
fn hide_window(app: tauri::AppHandle) {
    hide_main_window(&app);
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn hide_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
}

fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.center();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}

fn thumb_of(png_rel: &str) -> String {
    png_rel.replace(".png", "_thumb.png")
}

/// Decodifica l'icona PNG embedded (RGBA) nell'Image attesa dalla tray Tauri.
/// In caso di decode impossibile ricade su un pixel viola (mai un panic all'avvio).
fn tray_icon() -> Image<'static> {
    match image::load_from_memory(ICON_BYTES) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            Image::new_owned(rgba.into_raw(), w, h)
        }
        Err(e) => {
            eprintln!("[pulse] decode icona tray fallita: {e}");
            Image::new_owned(vec![0x7c, 0x5c, 0xff, 0xff], 1, 1)
        }
    }
}

fn clip_out(images_dir: &Path, item: db::ClipItem) -> ClipboardOut {
    let content = item.content.unwrap_or_default();
    let preview = {
        let one_line: String = content.split_whitespace().collect::<Vec<_>>().join(" ");
        if one_line.chars().count() > 160 {
            format!("{}…", one_line.chars().take(160).collect::<String>())
        } else {
            one_line
        }
    };
    let image = item.image_rel.as_ref().and_then(|rel| {
        let thumb = images_dir.join(thumb_of(rel));
        let path = if thumb.exists() {
            thumb
        } else {
            images_dir.join(rel)
        };
        std::fs::read(&path)
            .ok()
            .map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
    });
    ClipboardOut {
        id: item.id,
        kind: item.kind,
        content,
        preview,
        image,
        created_at: item.created_at,
        pinned: item.pinned,
        bytes: item.bytes,
    }
}

/// Lancia la Exec di un .desktop: via host se dentro Flatpak, altrimenti shell.
fn spawn_exec(exec: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        if Path::new("/.flatpak-info").exists() {
            std::process::Command::new("flatpak-spawn")
                .args(["--host", "sh", "-c", exec])
                .spawn()
                .map_err(|e| format!("flatpak-spawn fallito: {e}"))?;
            return Ok(());
        }
        std::process::Command::new("sh")
            .arg("-c")
            .arg(exec)
            .spawn()
            .map_err(|e| format!("spawn fallito: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", exec])
            .spawn()
            .map_err(|e| format!("spawn fallito: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(exec)
            .spawn()
            .map_err(|e| format!("spawn fallito: {e}"))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    tauri::Builder::default()
        // Single instance: una seconda copia mostra la finestra esistente
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.center();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // --- Directory dati (SQLite) e cache immagini ---
            let data_dir = dirs::data_local_dir()
                .or_else(dirs::data_dir)
                .unwrap_or_else(|| PathBuf::from("."))
                .join("pulse");
            let cache_dir = dirs::cache_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("pulse");
            let images_dir = cache_dir.join("images");
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(&images_dir)?;
            eprintln!("[pulse] db: {}", data_dir.join("pulse.db").display());
            eprintln!("[pulse] immagini clipboard: {}", images_dir.display());

            // --- Database + frecency in memoria ---
            let database = Arc::new(Mutex::new(db::Db::open(&data_dir.join("pulse.db"))?));
            let initial_frecency = database.lock().load_frecency();

            let index_arc: indexer::ArcRwLockIndex = Arc::new(RwLock::new(Vec::new()));
            let frecency_arc = Arc::new(RwLock::new(initial_frecency));
            let privacy_arc = Arc::new(Mutex::new(false));

            app.manage(AppState {
                db: database.clone(),
                index: index_arc.clone(),
                frecency: frecency_arc.clone(),
                privacy_mode: privacy_arc.clone(),
                images_dir: images_dir.clone(),
            });

            // --- Thread indice: scansione iniziale, poi notifica alla UI ---
            {
                let index = index_arc.clone();
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                    let mut items = indexer::scan_apps();
                    let apps = items.len();
                    items.extend(indexer::scan_files(&home, 250_000));
                    let count = items.len();
                    *index.write() = items;
                    eprintln!("[pulse] indicizzati {count} elementi ({apps} app)");
                    let _ = handle.emit("index-ready", count);
                });
            }

            // --- Thread watcher: aggiornamento incrementale ---
            {
                let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                indexer::watch_loop(home, index_arc.clone());
            }

            // --- Thread clipboard ---
            clipboard::spawn_listener(
                app.handle().clone(),
                database.clone(),
                privacy_arc.clone(),
                images_dir.clone(),
            );

            // --- System tray ---
            let open_i = MenuItem::with_id(app, "open", "Apri Pulse    (Alt+Space)", true, None::<&str>)?;
            let clear_i = MenuItem::with_id(app, "clear-clip", "Cancella cronologia appunti", true, None::<&str>)?;
            let privacy_i = CheckMenuItem::with_id(app, "privacy", "Modalità privacy (non registrare)", true, false, None::<&str>)?;
            let autostart_i = CheckMenuItem::with_id(app, "autostart", "Avvia all'avvio del sistema", true, false, None::<&str>)?;
            let quit_i = MenuItem::with_id(app, "quit", "Esci da Pulse", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[
                &open_i,
                &PredefinedMenuItem::separator(app)?,
                &clear_i,
                &privacy_i,
                &autostart_i,
                &PredefinedMenuItem::separator(app)?,
                &quit_i,
            ])?;

            let privacy_for_menu = privacy_i.clone();
            let autostart_for_menu = autostart_i.clone();
            let db_for_menu = database.clone();
            let images_for_menu = images_dir.clone();
            let privacy_arc_for_menu = privacy_arc.clone();

            let _tray = TrayIconBuilder::with_id("pulse-tray")
                .tooltip("Pulse — Alt+Space per aprire")
                .icon(tray_icon())
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main_window(tray.app_handle());
                    }
                })
                .on_menu_event(move |app, event| match event.id.as_ref() {
                    "open" => toggle_main_window(app),
                    "clear-clip" => {
                        if let Ok((n, rels)) = db_for_menu.lock().clear_clipboard() {
                            for rel in rels {
                                let _ = std::fs::remove_file(images_for_menu.join(&rel));
                                let _ = std::fs::remove_file(images_for_menu.join(thumb_of(&rel)));
                            }
                            let _ = app.emit("clipboard-changed", ());
                            let _ = app.emit("toast", format!("{n} elementi eliminati dagli appunti"));
                        }
                    }
                    "privacy" => {
                        let mut v = privacy_arc_for_menu.lock();
                        *v = !*v;
                        let _ = privacy_for_menu.set_checked(*v);
                        let _ = app.emit(
                            "toast",
                            if *v { "Modalità privacy attiva — appunti non registrati" } else { "Acquisizione appunti ripresa" },
                        );
                    }
                    "autostart" => {
                        let mgr = app.autolaunch();
                        let enabled = mgr.is_enabled().unwrap_or(false);
                        let _ = if enabled { mgr.disable() } else { mgr.enable() };
                        let _ = autostart_for_menu.set_checked(!enabled);
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .build(app)?;
            let _ = autostart_i.set_checked(app.autolaunch().is_enabled().unwrap_or(false));

            // --- Shortcut globale: Alt+Space ---
            if let Err(e) = app.global_shortcut().on_shortcut(GLOBAL_SHORTCUT, |app, _shortcut, event| {
                if event.state() == ShortcutState::Pressed {
                    toggle_main_window(app);
                }
            }) {
                eprintln!("[pulse] shortcut {GLOBAL_SHORTCUT} non registrata: {e:?}");
                eprintln!("[pulse] suggerimento: chiudi l'app che la occupa oppure cambia GLOBAL_SHORTCUT in main.rs");
            }

            // --- Nascondi quando perde il focus (comportamento launcher) ---
            if let Some(window) = app.get_webview_window("main") {
                let w = window.clone();
                window.on_window_event(move |event| {
                    if let WindowEvent::Focused(false) = event {
                        let _ = w.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search,
            open_item,
            reveal_item,
            copy_path,
            read_preview,
            list_clipboard,
            paste_clipboard,
            delete_clipboard_item,
            clear_clipboard,
            toggle_pin,
            get_settings,
            set_privacy_mode,
            hide_window,
        ])
        .run(tauri::generate_context!())
        .expect("errore fatale nell'avvio di Pulse");
}
