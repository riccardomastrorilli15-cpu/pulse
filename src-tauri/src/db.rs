//! db.rs — layer di persistenza SQLite (modalità WAL) per Pulse.
//!
//! Tabelle:
//!   frecency  — conteggio utilizzi e timestamp per ordinare i risultati
//!   clipboard — cronologia multimodale degli appunti (testo/link/emoji/immagine)
//!   meta      — chiave/valore generico (versione schema, contatori)

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS frecency (
    path       TEXT PRIMARY KEY,
    used_count INTEGER NOT NULL DEFAULT 0,
    last_used  INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS clipboard (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    kind       TEXT NOT NULL DEFAULT 'text',   -- text | link | emoji | image
    content    TEXT,                           -- testo completo, oppure label es. "1920×1080" per le immagini
    image_path TEXT,                           -- nome file relativo in <cache>/pulse/images (solo kind=image)
    hash       TEXT NOT NULL UNIQUE,           -- sha256 del contenuto: dedup automatica
    created_at INTEGER NOT NULL,
    pinned     INTEGER NOT NULL DEFAULT 0,
    bytes      INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_clipboard_created ON clipboard(pinned DESC, created_at DESC);
"#;

/// Elemento grezzo della cronologia appunti (usato internamente e mappato in main.rs).
#[derive(Debug, Clone, Serialize)]
pub struct ClipItem {
    pub id: i64,
    pub kind: String,
    pub content: Option<String>,
    pub image_rel: Option<String>,
    pub created_at: i64,
    pub pinned: bool,
    pub bytes: i64,
}

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Apre (o crea) il database e garantisce lo schema.
    pub fn open(path: &Path) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        conn.execute(
            "INSERT INTO meta(key, value) VALUES('schema_version', ?1)
             ON CONFLICT(key) DO NOTHING",
            params![SCHEMA_VERSION.to_string()],
        )?;
        Ok(Self { conn })
    }

    // ------------------------------------------------------------------
    // Frecency (frequenza + recenza)
    // ------------------------------------------------------------------

    /// Carica tutta la mappa frecency in memoria (<1ms tipicamente).
    pub fn load_frecency(&self) -> HashMap<String, (u64, i64)> {
        let mut map = HashMap::new();
        if let Ok(mut stmt) = self
            .conn
            .prepare("SELECT path, used_count, last_used FROM frecency")
        {
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, i64>(2)?,
                ))
            });
            if let Ok(rows) = rows {
                for row in rows.flatten() {
                    map.insert(row.0, (row.1, row.2));
                }
            }
        }
        map
    }

    /// Registra un utilizzo e restituisce la coppia aggiornata (used_count, last_used).
    pub fn record_use(&self, path: &str) -> (u64, i64) {
        let now = now_secs();
        let _ = self.conn.execute(
            "INSERT INTO frecency(path, used_count, last_used) VALUES(?1, 1, ?2)
             ON CONFLICT(path) DO UPDATE SET
                used_count = used_count + 1,
                last_used  = excluded.last_used",
            params![path, now],
        );
        self.conn
            .query_row(
                "SELECT used_count, last_used FROM frecency WHERE path = ?1",
                params![path],
                |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)?)),
            )
            .unwrap_or((1, now))
    }

    // ------------------------------------------------------------------
    // Clipboard
    // ------------------------------------------------------------------

    /// Inserisce un elemento; ignora silenziosamente i duplicati (stesso hash).
    pub fn insert_clipboard(
        &self,
        kind: &str,
        content: Option<&str>,
        image_rel: Option<&str>,
        hash: &str,
        bytes: i64,
    ) -> Result<bool, rusqlite::Error> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO clipboard(kind, content, image_path, hash, created_at, pinned, bytes)
             VALUES(?1, ?2, ?3, ?4, ?5, 0, ?6)",
            params![kind, content, image_rel, hash, now_secs(), bytes],
        )?;
        Ok(n > 0)
    }

    /// Lista la cronologia (opzionale filtro testuale, case-insensitive).
    pub fn list_clipboard(&self, query: &str, limit: i64) -> Vec<ClipItem> {
        let q = escape_like(query.trim());
        let mut stmt = match self.conn.prepare(
            "SELECT id, kind, content, image_path, created_at, pinned, bytes
             FROM clipboard
             WHERE (?1 = '' OR content LIKE '%' || ?1 || '%' ESCAPE '\\')
             ORDER BY pinned DESC, created_at DESC
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let rows = stmt.query_map(params![q, limit], row_to_clipitem);
        match rows {
            Ok(r) => r.flatten().collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Recupera un elemento completo (per incollare).
    pub fn clipboard_full(&self, id: i64) -> Option<ClipItem> {
        self.conn
            .query_row(
                "SELECT id, kind, content, image_path, created_at, pinned, bytes
                 FROM clipboard WHERE id = ?1",
                params![id],
                row_to_clipitem,
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Elimina un elemento; restituisce image_rel se c'era un'immagine da rimuovere.
    pub fn delete_clipboard(&self, id: i64) -> Result<Option<String>, rusqlite::Error> {
        let img: Option<String> = self
            .conn
            .query_row(
                "SELECT image_path FROM clipboard WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        self.conn
            .execute("DELETE FROM clipboard WHERE id = ?1", params![id])?;
        Ok(img)
    }

    /// Svuota la cronologia; restituisce (elementi eliminati, immagini relative da cancellare).
    pub fn clear_clipboard(&self) -> Result<(usize, Vec<String>), rusqlite::Error> {
        let mut paths = Vec::new();
        if let Ok(mut stmt) = self
            .conn
            .prepare("SELECT image_path FROM clipboard WHERE image_path IS NOT NULL")
        {
            if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                for p in rows.flatten() {
                    paths.push(p);
                }
            }
        }
        let n = self.conn.execute("DELETE FROM clipboard", [])?;
        Ok((n, paths))
    }

    /// Inverte lo stato pin; restituisce il nuovo valore.
    pub fn toggle_pin(&self, id: i64) -> Result<bool, rusqlite::Error> {
        self.conn.execute(
            "UPDATE clipboard SET pinned = 1 - pinned WHERE id = ?1",
            params![id],
        )?;
        self.conn.query_row(
            "SELECT pinned FROM clipboard WHERE id = ?1",
            params![id],
            |r| r.get::<_, i64>(0).map(|v| v != 0),
        )
    }
}

fn row_to_clipitem(r: &rusqlite::Row) -> Result<ClipItem, rusqlite::Error> {
    Ok(ClipItem {
        id: r.get(0)?,
        kind: r.get(1)?,
        content: r.get(2)?,
        image_rel: r.get(3)?,
        created_at: r.get(4)?,
        pinned: r.get::<_, i64>(5)? != 0,
        bytes: r.get(6)?,
    })
}

/// Escape dei metacaratteri LIKE (`%`, `_`, `\`) per ricerche letterali.
pub fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> (tempfile_path::Tmp, Db) {
        let t = tempfile_path::Tmp::new();
        let db = Db::open(t.path()).unwrap();
        (t, db)
    }

    // piccolo helper per non dipendere dal crate tempfile
    mod tempfile_path {
        use std::path::{Path, PathBuf};
        pub struct Tmp {
            p: PathBuf,
        }
        impl Tmp {
            pub fn new() -> Self {
                let p = std::env::temp_dir().join(format!(
                    "pulse-test-{}-{}.db",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos()
                ));
                Self { p }
            }
            pub fn path(&self) -> &Path {
                &self.p
            }
        }
        impl Drop for Tmp {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.p);
            }
        }
    }

    #[test]
    fn frecency_increments() {
        let (_t, db) = tmp_db();
        assert_eq!(db.record_use("/tmp/app").0, 1);
        assert_eq!(db.record_use("/tmp/app").0, 2);
        assert_eq!(db.record_use("/tmp/app").0, 3);
    }

    #[test]
    fn clipboard_dedup_by_hash() {
        let (_t, db) = tmp_db();
        assert!(db
            .insert_clipboard("text", Some("ciao"), None, "hash1", 4)
            .unwrap());
        assert!(!db
            .insert_clipboard("text", Some("ciao"), None, "hash1", 4)
            .unwrap());
        assert_eq!(db.list_clipboard("", 10).len(), 1);
    }

    #[test]
    fn clipboard_search_escapes_like() {
        let (_t, db) = tmp_db();
        db.insert_clipboard("text", Some("100% coverage"), None, "h-like", 13)
            .unwrap();
        assert_eq!(db.list_clipboard("%", 10).len(), 1); // '%' trattato come letterale
        assert_eq!(db.list_clipboard("100%", 10).len(), 1);
        assert_eq!(db.list_clipboard("coverage", 10).len(), 1);
        assert_eq!(db.list_clipboard("xyz", 10).len(), 0);
    }

    #[test]
    fn clipboard_clear_counts() {
        let (_t, db) = tmp_db();
        db.insert_clipboard("text", Some("a"), None, "ha", 1)
            .unwrap();
        db.insert_clipboard("text", Some("b"), None, "hb", 1)
            .unwrap();
        let (n, imgs) = db.clear_clipboard().unwrap();
        assert_eq!(n, 2);
        assert!(imgs.is_empty());
        assert_eq!(db.list_clipboard("", 10).len(), 0);
    }

    #[test]
    fn pin_toggle_roundtrip() {
        let (_t, db) = tmp_db();
        db.insert_clipboard("text", Some("pin me"), None, "hp", 6)
            .unwrap();
        let id = db.list_clipboard("", 1)[0].id;
        assert!(db.toggle_pin(id).unwrap());
        assert!(!db.toggle_pin(id).unwrap());
    }

    #[test]
    fn escape_like_works() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }
}
