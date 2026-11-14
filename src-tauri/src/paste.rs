//! paste.rs — incolla automatico (Ctrl+V) nella finestra precedentemente attiva.
//!
//! Ordine dei tentativi:
//!   1. enigo (X11 via x11rb, Wayland via virtual-keyboard dove supportato)
//!   2. fallback su Linux/Wayland: `wtype` (piccolo tool Wayland, se installato)
//!   3. altrimenti: l'elemento resta negli appunti e la UI mostra "Premi Ctrl+V"

use std::process::Command;

pub fn paste_into_active_window() -> Result<(), String> {
    let result = std::panic::catch_unwind(enigo_paste);
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => {
            eprintln!("[pulse] enigo paste fallito: {e}");
            wayland_fallback()
        }
        Err(_) => {
            eprintln!("[pulse] enigo panic durante l'incolla");
            wayland_fallback()
        }
    }
}

fn enigo_paste() -> Result<(), String> {
    use enigo::{Direction, Enigo, Key, Keyboard, Settings};

    let mut enigo = Enigo::new(&Settings::default()).map_err(|e| format!("init enigo: {e:?}"))?;

    #[cfg(target_os = "macos")]
    let modifier = Key::Meta;
    #[cfg(not(target_os = "macos"))]
    let modifier = Key::Control;

    enigo
        .key(modifier, Direction::Press)
        .map_err(|e| format!("press modifier: {e:?}"))?;
    let click_result = enigo.key(Key::Unicode('v'), Direction::Click);
    let release_result = enigo.key(modifier, Direction::Release);

    click_result.map_err(|e| format!("v click: {e:?}"))?;
    release_result.map_err(|e| format!("release modifier: {e:?}"))?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn wayland_fallback() -> Result<(), String> {
    // wtype: minuscolo tool di virtual-keyboard per Wayland (apt install wtype)
    match Command::new("wtype")
        .args(["-M", "ctrl", "-k", "v", "-m", "ctrl"])
        .status()
    {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!(
            "wtype terminato con {s}. Elemento copiato: premi Ctrl+V per incollare."
        )),
        Err(_) => Err(
            "Incolla automatico non disponibile (Compositor Wayland senza virtual-keyboard o X11 non attivo). \
             Elemento copiato: premi Ctrl+V per incollare."
                .to_string(),
        ),
    }
}

#[cfg(not(target_os = "linux"))]
fn wayland_fallback() -> Result<(), String> {
    Err("Elemento copiato: premi Ctrl+V per incollare.".to_string())
}
