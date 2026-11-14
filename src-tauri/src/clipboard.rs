//! clipboard.rs — daemon listener degli appunti.
//!
//! Strategia: polling a 450 ms su thread dedicato (arboard è sincrono e
//! cross-platform: WebView2/Win32 su Windows, X11/Wayland su Linux).
//! Il costo per poll è trascurabile; la deviazione/analisi avviene solo
//! quando il contenuto cambia (hash sha256).
//!
//! Privacy:
//!   - Modalità privacy (privacy_mode) → nessun dato viene salvato
//!   - Filtro euristico dei segreti (password manager / token / chiavi private)
//!   - Nessuna trasmissione di rete: tutto in SQLite locale

use arboard::Clipboard;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::Emitter;

use crate::db::Db;

pub fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

/// Classifica un testo copiato per mostrarlo con l'icona giusta nella UI.
pub fn classify_text(t: &str) -> &'static str {
    let trimmed = t.trim();
    if is_emoji_sequence(trimmed) {
        return "emoji";
    }
    let lower = trimmed.to_lowercase();
    if (lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("www."))
        && !trimmed.chars().any(char::is_whitespace)
    {
        return "link";
    }
    "text"
}

/// Euristica: vero solo se TUTTI i caratteri sono emoji/pittogrammi o spazi.
fn is_emoji_sequence(t: &str) -> bool {
    if t.is_empty() || t.chars().count() > 64 {
        return false;
    }
    t.chars().all(|c| {
        let cp = c as u32;
        matches!(cp,
            0x1F000..=0x1FAFF      // pittogrammi, emoticon, simboli supplementari e bandiere (1F1E6–1F1FF inclusi qui)
            | 0x2600..=0x26FF      // simboli vari
            | 0x2700..=0x27BF      // dingbat
            | 0x2B00..=0x2BFF      // frecce/simboli vari
            | 0xFE00..=0xFE0F      // variation selector
            | 0xFE20..=0xFE2F      // combining diacritical marks extended
            | 0x200D               // ZWJ
            | 0x20E3               // keycap combinante
            | 0xE0020..=0xE007F    // tag (sequenze emoji)
            | 0x0030..=0x0039      // cifre (keycap)
            | 0x0023 | 0x002A      // '#' e '*'
        ) || c.is_whitespace()
    })
}

/// Filtro euristico dei segreti. NOTA: il rilevamento del processo sorgente
/// non è portabile cross-platform; questo filtro copre i pattern più comuni
/// (password manager, cloud provider, token GitHub/AWS/OpenAI...).
pub fn is_sensitive(text: &str) -> bool {
    let t = text.trim();
    let upper = t.to_uppercase();

    // Chiavi private in qualsiasi formato PEM
    if upper.contains("-----BEGIN") && upper.contains("PRIVATE KEY") {
        return true;
    }
    if upper.contains("-----BEGIN OPENSSH") || upper.contains("-----BEGIN PGP") {
        return true;
    }
    // AWS Access Key ID
    if t.contains("AKIA") && t.len() >= 16 {
        return true;
    }
    // Prefissi di token comuni
    const PREFIXES: &[&str] = &[
        "ghp_",
        "gho_",
        "ghu_",
        "ghs_",
        "ghr_",
        "github_pat_",
        "xoxb-",
        "xoxa-",
        "xoxp-",
        "xoxs-",
        "xoxe.",
        "sk-",
        "sk_live_",
        "sk_test_",
        "rk_live_",
        "rk_test_",
        "glpat-",
        "ya29.",
        "EAAB",
        "AIza",
    ];
    for p in PREFIXES {
        if t.starts_with(p) && t.len() > p.len() + 8 {
            return true;
        }
    }
    // Coppie chiave=valore tipiche dei segreti
    let lower = t.to_lowercase();
    const KV: &[&str] = &[
        "password=",
        "passwd=",
        "pwd=",
        "passphrase=",
        "client_secret=",
        "secret_access_key",
        "aws_secret_access_key",
        "private_key=",
        "api_key=",
        "apikey=",
        "access_token=",
        "auth_token=",
    ];
    for kv in KV {
        if lower.contains(kv) {
            return true;
        }
    }
    // JSON monolinea con chiavi sensibili
    lower.contains("\"password\"")
        || lower.contains("\"client_secret\"")
        || lower.contains("\"private_key\"")
}

/// Avvia il listener in un thread daemon. `privacy_mode` condiviso col resto dell'app.
pub fn spawn_listener(
    app: tauri::AppHandle,
    db: Arc<parking_lot::Mutex<Db>>,
    privacy_mode: Arc<parking_lot::Mutex<bool>>,
    images_dir: PathBuf,
) {
    std::thread::spawn(move || {
        // Inizializzazione clipboard con retry: su Wayland il socket può non essere
        // pronto al primo giro, su Windows il thread deve attendere il desktop.
        let mut cb = match Clipboard::new() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[pulse] clipboard init iniziale fallita ({e}); retry ogni 3s");
                loop {
                    std::thread::sleep(Duration::from_secs(3));
                    match Clipboard::new() {
                        Ok(c) => break c,
                        Err(_) => continue,
                    }
                }
            }
        };

        let mut last_hash = String::new();
        eprintln!("[pulse] clipboard listener attivo");

        loop {
            std::thread::sleep(Duration::from_millis(450));
            if *privacy_mode.lock() {
                continue; // modalità privacy: non acquisire nulla
            }

            // 1) Prova il testo (caso più comune: copie frequenti, basso costo)
            match cb.get_text() {
                Ok(text) if !text.trim().is_empty() => {
                    let hash = sha256_hex(text.as_bytes());
                    if hash == last_hash {
                        continue;
                    }
                    last_hash = hash.clone();
                    if is_sensitive(&text) {
                        continue; // segreto rilevato: non salvare
                    }
                    let kind = classify_text(&text);
                    let _ = db.lock().insert_clipboard(
                        kind,
                        Some(&text),
                        None,
                        &hash,
                        text.len() as i64,
                    );
                    let _ = app.emit("clipboard-changed", ());
                    continue;
                }
                _ => {}
            }

            // 2) Altrimenti prova un'immagine (screenshot, copia immagine...)
            if let Ok(img) = cb.get_image() {
                let hash = sha256_hex(img.bytes.as_ref());
                if hash == last_hash {
                    continue;
                }
                last_hash = hash.clone();

                let Some(rgba) = image::RgbaImage::from_raw(
                    img.width as u32,
                    img.height as u32,
                    img.bytes.into_owned(),
                ) else {
                    eprintln!("[pulse] immagine clipboard non decodificabile");
                    continue;
                };
                let dyn_img = image::DynamicImage::ImageRgba8(rgba);

                let png_rel = format!("{hash}.png");
                let thumb_rel = format!("{hash}_thumb.png");
                let png_path = images_dir.join(&png_rel);
                let thumb_path = images_dir.join(&thumb_rel);

                if let Err(e) = dyn_img.save_with_format(&png_path, image::ImageFormat::Png) {
                    eprintln!("[pulse] salvataggio PNG fallito: {e}");
                    continue;
                }
                // Thumbnail 384px lato max, PNG ottimizzato
                let thumb = dyn_img.thumbnail(384, 384);
                let _ = thumb.save_with_format(&thumb_path, image::ImageFormat::Png);

                let size = std::fs::metadata(&png_path)
                    .map(|m| m.len() as i64)
                    .unwrap_or(0);
                let label = format!("{}×{}", dyn_img.width(), dyn_img.height());
                let _ =
                    db.lock()
                        .insert_clipboard("image", Some(&label), Some(&png_rel), &hash, size);
                let _ = app.emit("clipboard-changed", ());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_link() {
        assert_eq!(classify_text("https://example.com"), "link");
        assert_eq!(classify_text("www.rust-lang.org"), "link");
        assert_eq!(classify_text("testo normale"), "text");
    }

    #[test]
    fn classify_emoji() {
        assert_eq!(classify_text("🔥🚀✨"), "emoji");
        assert_eq!(classify_text("🔥🔥 🔥🔥"), "emoji");
        assert_eq!(classify_text("🔥 fuoco"), "text");
    }

    #[test]
    fn sensitive_detection() {
        assert!(is_sensitive(
            "-----BEGIN OPENSSH PRIVATE KEY-----\nb3BlbnNzaA"
        ));
        assert!(is_sensitive("AKIAIOSFODNN7EXAMPLE"));
        assert!(is_sensitive("ghp_1234567890abcdefghij1234567890abcdef"));
        assert!(is_sensitive("password=hunter2"));
        assert!(is_sensitive("https://user:pass@host.com/x?password=secret"));
        assert!(!is_sensitive("https://github.com/riccardomastrorilli15-cpu/pulse"));
        assert!(!is_sensitive("Ricetta della carbonara"));
    }
}
