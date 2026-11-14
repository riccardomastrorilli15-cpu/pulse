//! indexer.rs — scansione e ricerca indicizzata per Pulse.
//!
//! Pipeline:
//!   1. scan_apps()   — .desktop (Linux, incluse Flatpak) / Start Menu (Windows)
//!   2. scan_files()  — home directory con ignore-crate (filtri hidden/gitignore + esclusioni)
//!   3. search()      — nucleo-matcher (fuzzy, stile Helix) × boost frecency (frequenza + recenza)
//!   4. watch_loop()  — notify: ri-indicizza incrementalmente al cambio del filesystem

use ignore::WalkBuilder;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::db::now_secs;

// ---------------------------------------------------------------------------
// Tipi
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    App,
    File,
    Dir,
}

impl ItemKind {
    fn as_str(self) -> &'static str {
        match self {
            ItemKind::App => "app",
            ItemKind::File => "file",
            ItemKind::Dir => "dir",
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexItem {
    pub name: String,
    pub path: String, // chiave frecency stabile
    pub kind: ItemKind,
    pub exec: Option<String>, // solo per le app .desktop: riga Exec pulita
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub exec: Option<String>,
    pub score: f64,
    pub used_count: u64,
    pub last_used: i64,
}

/// Directory escluse di default (personalizzabili in futuro via settings.toml).
const EXCLUDED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".svn",
    ".hg",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    "target",
    "dist",
    "build",
    "out",
    ".gradle",
    ".cargo",
    ".rustup",
    ".cache",
    "cache",
    "Cache",
    "Caches",
    ".npm",
    ".yarn",
    ".pnpm-store",
    ".bun",
    ".deno",
    "AppData",
    "Temp",
    "temp",
    "CacheStorage",
    "GPUCache",
];

const MAX_FILES: usize = 250_000;
const MAX_DEPTH: usize = 12;

// ---------------------------------------------------------------------------
// Scansione applicazioni
// ---------------------------------------------------------------------------

/// Scansiona le applicazioni installate per la piattaforma corrente.
pub fn scan_apps() -> Vec<IndexItem> {
    #[cfg(target_os = "linux")]
    {
        scan_apps_linux()
    }
    #[cfg(target_os = "windows")]
    {
        scan_apps_windows()
    }
    #[cfg(target_os = "macos")]
    {
        // Non target primario: indicizza /Applications in modo minimale
        scan_apps_macos()
    }
}

#[cfg(target_os = "linux")]
fn scan_apps_linux() -> Vec<IndexItem> {
    let mut items = Vec::new();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let mut dirs = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
        home.join(".local/share/applications"),
        PathBuf::from("/var/lib/flatpak/exports/share/applications"),
        home.join(".local/share/flatpak/exports/share/applications"),
        PathBuf::from("/var/lib/snapd/desktop/applications"),
    ];
    dirs.retain(|d| d.is_dir());
    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
                    continue;
                }
                if let Some(app) = parse_desktop_file(&path) {
                    items.push(app);
                }
            }
        }
    }
    items.sort_by_key(|a| a.name.to_lowercase());
    items.dedup_by(|a, b| a.name == b.name && a.exec == b.exec);
    items
}

/// Parsa un file .desktop secondo la specifica freedesktop.org.
/// Ritorna None per entry nascoste/non-applicazione/senza Exec.
#[cfg(target_os = "linux")]
fn parse_desktop_file(path: &Path) -> Option<IndexItem> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line.eq_ignore_ascii_case("[Desktop Entry]");
            continue;
        }
        if !in_entry || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "Type" => {
                if !value.eq_ignore_ascii_case("Application") {
                    return None;
                }
            }
            "NoDisplay" | "Hidden" => {
                if value.eq_ignore_ascii_case("true") {
                    return None;
                }
            }
            "Name" => {
                if name.is_none() {
                    name = Some(value.to_string());
                }
            }
            "Exec" => exec = Some(clean_exec(value)),
            _ => {}
        }
    }
    let exec = exec?;
    if exec.is_empty() {
        return None;
    }
    Some(IndexItem {
        name: name?,
        path: path.to_string_lossy().into_owned(),
        kind: ItemKind::App,
        exec: Some(exec),
    })
}

/// Rimuove i field code .desktop (%f %u %F %U %i %c %k ...) e i quoting superflui.
pub fn clean_exec(exec: &str) -> String {
    exec.split_whitespace()
        .filter(|tok| !tok.starts_with('%'))
        .map(|tok| tok.trim_matches('"'))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(target_os = "windows")]
fn scan_apps_windows() -> Vec<IndexItem> {
    let mut items = Vec::new();
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(pd) = std::env::var_os("ProgramData") {
        roots.push(PathBuf::from(pd).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(ap) = std::env::var_os("AppData") {
        roots.push(PathBuf::from(ap).join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(la) = std::env::var_os("LOCALAPPDATA") {
        roots.push(PathBuf::from(la).join("Programs")); // installazioni per-utente
    }
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for entry in WalkBuilder::new(&root)
            .follow_links(false)
            .max_depth(Some(6))
            .build()
        {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            if ext != "lnk" && ext != "exe" {
                continue;
            }
            // Salta utility di disinstallazione
            let name_raw = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let lower = name_raw.to_lowercase();
            if lower.contains("uninstall")
                || lower.contains(" disinstall")
                || lower.contains("setup")
            {
                continue;
            }
            if !name_raw.is_empty() {
                items.push(IndexItem {
                    name: name_raw,
                    path: path.to_string_lossy().into_owned(),
                    kind: ItemKind::App,
                    exec: None, // .lnk ed .exe si aprono via ShellExecute (opener plugin)
                });
            }
        }
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    items.dedup_by(|a, b| a.name == b.name);
    items
}

#[cfg(target_os = "macos")]
fn scan_apps_macos() -> Vec<IndexItem> {
    let mut items = Vec::new();
    for root in [
        "/Applications",
        &format!(
            "{}/Applications",
            dirs::home_dir().unwrap_or_default().display()
        ),
    ] {
        if let Ok(entries) = std::fs::read_dir(root) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("app") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        items.push(IndexItem {
                            name: name.to_string(),
                            path: path.to_string_lossy().into_owned(),
                            kind: ItemKind::App,
                            exec: None,
                        });
                    }
                }
            }
        }
    }
    items
}

// ---------------------------------------------------------------------------
// Scansione filesystem utente
// ---------------------------------------------------------------------------

/// Cammina la home dell'utente con filtri aggressivi; cap a MAX_FILES elementi.
pub fn scan_files(home: &Path, max_items: usize) -> Vec<IndexItem> {
    let mut items = Vec::with_capacity(16_384);
    let mut builder = WalkBuilder::new(home);
    builder
        .hidden(true) // salta dotfile/cartelle nascoste
        .git_ignore(true) // rispetta .gitignore
        .git_global(true)
        .git_exclude(true)
        .require_git(false)
        .follow_links(false)
        .same_file_system(true)
        .max_depth(Some(MAX_DEPTH))
        .filter_entry(|entry| {
            if entry.depth() > 0 {
                if let Some(name) = entry.file_name().to_str() {
                    if EXCLUDED_DIRS.iter().any(|ex| ex.eq_ignore_ascii_case(name)) {
                        return false;
                    }
                }
            }
            true
        });

    for result in builder.build() {
        if items.len() >= max_items.min(MAX_FILES) {
            break;
        }
        let Ok(entry) = result else { continue };
        let path = entry.path();
        if entry.depth() == 0 {
            continue;
        }
        let kind = if path.is_dir() {
            ItemKind::Dir
        } else {
            ItemKind::File
        };
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        items.push(IndexItem {
            name,
            path: path.to_string_lossy().into_owned(),
            kind,
            exec: None,
        });
    }
    items
}

// ---------------------------------------------------------------------------
// Ricerca fuzzy + frecency
// ---------------------------------------------------------------------------

/// Boost frecency: frequenza con compressione logaritmica × decadimento di recenza.
///   - mai aperto          → 1.0     (neutro)
///   - uso frequente oggi  → ~3-6x
///   - uso di mesi fa      → torna verso 1.0
pub fn frecency_boost(used_count: u64, last_used: i64, now: i64) -> f64 {
    if used_count == 0 {
        return 1.0;
    }
    let age_days = ((now - last_used).max(0) as f64) / 86_400.0;
    let recency = 1.0 / (1.0 + age_days); // 1.0 oggi → ~0.5 dopo 1 giorno → ~0.12 dopo 7
    (1.0 + (used_count as f64).ln_1p()) * (0.35 + 0.65 * recency)
}

/// Esegue la ricerca. Con query vuota restituisce i top-N per frecency.
pub fn search(
    items: &[IndexItem],
    frecency: &HashMap<String, (u64, i64)>,
    query: &str,
    limit: usize,
) -> Vec<SearchResult> {
    let now = now_secs();
    let query = query.trim();

    if query.is_empty() {
        let mut scored: Vec<SearchResult> = items
            .iter()
            .filter_map(|item| {
                let (used, last) = frecency.get(&item.path).copied().unwrap_or((0, 0));
                if used == 0 {
                    return None;
                }
                Some(SearchResult {
                    name: item.name.clone(),
                    path: item.path.clone(),
                    kind: item.kind.as_str().to_string(),
                    exec: item.exec.clone(),
                    score: frecency_boost(used, last, now),
                    used_count: used,
                    last_used: last,
                })
            })
            .collect();
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        return scored;
    }

    let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
    let mut buf = Vec::new();
    let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
    let mut scored: Vec<SearchResult> = Vec::with_capacity(128);

    for item in items {
        let hay = Utf32Str::new(&item.name, &mut buf);
        let Some(fuzzy) = pattern.score(hay, &mut matcher) else {
            continue;
        };
        let (used, last) = frecency.get(&item.path).copied().unwrap_or((0, 0));
        let frec = frecency_boost(used, last, now);
        let final_score = fuzzy as f64 * frec;
        scored.push(SearchResult {
            name: item.name.clone(),
            path: item.path.clone(),
            kind: item.kind.as_str().to_string(),
            exec: item.exec.clone(),
            score: final_score,
            used_count: used,
            last_used: last,
        });
    }
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);
    scored
}

// ---------------------------------------------------------------------------
// Watcher filesystem (testo su canale mpsc → update incrementale)
// ---------------------------------------------------------------------------

/// Avvia il thread di watching. Ritorna immediatamente; il thread vive per sempre.
/// Se il watcher non può essere creato (es. fs.inotify.max_user_watches troppo basso), logga e termina il thread.
pub fn watch_loop(home: PathBuf, index: ArcRwLockIndex) {
    std::thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel::<PathBuf>();
        let watcher_result = notify::RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    for p in event.paths {
                        let _ = tx.send(p);
                    }
                }
            },
            notify::Config::default().with_poll_interval(std::time::Duration::from_secs(2)),
        );

        let mut watcher = match watcher_result {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[pulse] watcher filesystem non disponibile: {e}");
                return;
            }
        };

        use notify::Watcher;
        // Osserva la home in modo ricorsivo; su errori parziali (permessi) continua con ciò che può.
        if let Err(e) = watcher.watch(&home, notify::RecursiveMode::Recursive) {
            eprintln!("[pulse] watch ricorsivo su {} fallito: {e}", home.display());
        }
        eprintln!("[pulse] watcher attivo su {}", home.display());

        loop {
            // Debounce: raccoglie gli eventi per 500 ms
            let mut changed: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
            match rx.recv_timeout(std::time::Duration::from_millis(500)) {
                Ok(p) => {
                    changed.insert(p);
                    while let Ok(p) = rx.try_recv() {
                        changed.insert(p);
                        if changed.len() > 512 {
                            break;
                        }
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            let mut items = index.write();
            for path in changed {
                let is_dir = path.is_dir();
                let exists = path.exists();
                // Rimuovi eventuale entry esistente per questo path
                let before = items.len();
                items.retain(|i| Path::new(&i.path) != path.as_path());
                let removed = before != items.len();

                if !exists {
                    continue; // cancellato: via
                }
                let file_name_ok = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| {
                        !n.starts_with('.')
                            && !EXCLUDED_DIRS.iter().any(|ex| ex.eq_ignore_ascii_case(n))
                    })
                    .unwrap_or(false);
                if !file_name_ok {
                    continue;
                }
                if is_dir {
                    // Cartella nuova: indicizza il suo contenuto immediato (non ricorsivo, bounded)
                    if let Ok(read) = std::fs::read_dir(&path) {
                        for entry in read.flatten().take(256) {
                            let p = entry.path();
                            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                                let kind = if p.is_dir() {
                                    ItemKind::Dir
                                } else {
                                    ItemKind::File
                                };
                                items.push(IndexItem {
                                    name: name.to_string(),
                                    path: p.to_string_lossy().into_owned(),
                                    kind,
                                    exec: None,
                                });
                            }
                        }
                    }
                    items.push(IndexItem {
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        path: path.to_string_lossy().into_owned(),
                        kind: ItemKind::Dir,
                        exec: None,
                    });
                } else {
                    let kind = ItemKind::File;
                    items.push(IndexItem {
                        name: path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        path: path.to_string_lossy().into_owned(),
                        kind,
                        exec: None,
                    });
                }
                let _ = removed; // (informativo in futuro: metriche)
            }
        }
    });
}

/// Tipo condiviso per l'indice (definito qui per evitare cicli).
pub type ArcRwLockIndex = std::sync::Arc<parking_lot::RwLock<Vec<IndexItem>>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_exec_strips_field_codes() {
        assert_eq!(clean_exec("firefox %u"), "firefox");
        assert_eq!(clean_exec("code --new-window %F"), "code --new-window");
        assert_eq!(
            clean_exec("env FOO=bar \"/opt/app/run.sh\" --flag %i"),
            "env FOO=bar /opt/app/run.sh --flag"
        );
    }

    #[test]
    fn frecency_never_used_is_neutral() {
        assert_eq!(frecency_boost(0, 0, 1_800_000_000), 1.0);
    }

    #[test]
    fn frecency_recent_beats_old() {
        let now = 1_800_000_000;
        let recent = frecency_boost(5, now - 3600, now);
        let old = frecency_boost(5, now - 180 * 86_400, now);
        assert!(recent > old);
    }

    #[test]
    fn search_matches_and_ranks() {
        let items = vec![
            IndexItem {
                name: "Visual Studio Code".into(),
                path: "/apps/code".into(),
                kind: ItemKind::App,
                exec: Some("code".into()),
            },
            IndexItem {
                name: "Visual Studio 2022".into(),
                path: "/apps/vs".into(),
                kind: ItemKind::App,
                exec: Some("devenv".into()),
            },
            IndexItem {
                name: "firefox".into(),
                path: "/apps/firefox".into(),
                kind: ItemKind::App,
                exec: None,
            },
        ];
        let mut frec = HashMap::new();
        frec.insert("/apps/firefox".to_string(), (25u64, now_secs())); // uso intenso e recente
        let out = search(&items, &frec, "vis", 10);
        assert!(out.iter().all(|r| r.name.to_lowercase().contains("vis")));
        let out_ff = search(&items, &frec, "fire", 10);
        assert_eq!(out_ff[0].name, "firefox");
        assert_eq!(out_ff[0].used_count, 25);
    }

    #[test]
    fn search_empty_query_returns_top_frecency() {
        let items = vec![
            IndexItem {
                name: "a".into(),
                path: "/a".into(),
                kind: ItemKind::File,
                exec: None,
            },
            IndexItem {
                name: "b".into(),
                path: "/b".into(),
                kind: ItemKind::File,
                exec: None,
            },
        ];
        let mut frec = HashMap::new();
        frec.insert("/b".to_string(), (9u64, now_secs()));
        let out = search(&items, &frec, "", 10);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].path, "/b");
    }
}
