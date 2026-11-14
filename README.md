<div align="center">

# ⚡ Pulse Launcher

**Launcher globale istantaneo + clipboard manager per Windows e Linux.**
Ricerca fuzzy con frecency, cronologia appunti multimodale, finestra Spotlight/Raycast, **tutto in locale**, zero telemetria.

[![CI](https://github.com/riccardomastrorilli15-cpu/pulse/actions/workflows/ci.yml/badge.svg)](https://github.com/riccardomastrorilli15-cpu/pulse/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%C2%B7%20Linux-blue.svg)](#build)
[![Built with Tauri v2](https://img.shields.io/badge/Tauri-v2-24c8db.svg)](https://tauri.app)

</div>

---

## Perché Pulse

| | Windows Search | PowerToys Run | Raycast (macOS) | **Pulse** |
|---|---|---|---|---|
| Apertura | lento, indicizzazione OS | media | istantanea | **istantanea (Alt+Space)** |
| Ricerca fuzzy + frecency | parziale | no frecency | sì | **sì (nucleo-matcher + frecency custom)** |
| Clipboard manager immagini | no | no | no (testo) | **sì (PNG ottimizzato + thumbnail)** |
| Offline / zero cloud | sì, ma invia dati a Bing per web | sì | parziale | **sì, 100% locale** |
| Footprint | pesante | medio | medio | **obiettivo < 50 MB idle** |
| Codice aperto | no | sì | no | **sì (MIT)** |

> Onestà sui numeri: latenza < 5 ms e RAM < 30–50 MB sono **obiettivi di progetto** raggiunti grazie
> all'indice in memoria (`RwLock<Vec<IndexItem>>`), matcher `nucleo` (Helix) e LTO fat —
> misurali sul tuo hardware con `time` + Task Manager/`htop`. Non sono benchmark certificati.

### Funzionalità

- 🔍 **Ricerca globale** — app installate (`.desktop` Linux incluse Flatpak/Snap, Start Menu Windows) + home completa; ranking **fuzzy × frecency** (più usi qualcosa, più in alto compare)
- 📋 **Clipboard manager** — testo, link, codice, emoji ✨, **immagini** (screenshot → PNG + thumbnail 384px in cache locale)
- 📌 **Pin & privacy** — fissa elementi importanti; un click (o un toggle in tray) per svuotare la cronologia o sospendere l'acquisizione
- 🔐 **Filtro segreti** — chiavi private PEM, token AWS/GitHub/OpenAI/Slack, `password=...` non vengono salvati
- ⚡ **Incolla automatico** — `Invio` sull'elemento → torna negli appunti e simula `Ctrl+V` (enigo; fallback `wtype` su Wayland)
- 🖥️ **Spotlight-like** — finestra senza bordi, `border-radius: 18px`, blur nativo (WebView2/Win11 Mica-like, WebKitGTK su Linux), sempre in primo piano
- 🎹 **100% tastiera** — `↑↓` naviga, `Tab` cambia modalità, `Esc` nasconde, `Ctrl+P` anteprima, `Ctrl+Enter` apre la cartella, `Ctrl+C` copia il percorso
- 🗂️ **Modalità** — Tutto / App / File / Appunti (prefisso `cb ` nel campo di ricerca per saltare agli appunti)
- 👁️ **Preview pane** — anteprima file di testo, contenuto appunti, immagini, metadati app (`Ctrl+P`)
- 🔔 **System tray** — apri / svuota appunti / privacy / avvio automatico / esci
- 📦 **Pacchetti** — `.msi`/`.exe` (Windows), binario nativo + `.deb`/`.AppImage` (Linux), **Flatpak** incluso

---

## Build

### Requisiti

- **Rust** ≥ 1.77 (installa via [rustup](https://rustup.rs))
- **Windows**: Windows 10/11 (WebView2 incluso in Win11; su Win10 l'installer WebView2 viene proposto automaticamente da Tauri)
- **Linux**: WebKitGTK 4.1 + GTK3 + libsoup3, `pkg-config`
  ```bash
  sudo apt-get update && sudo apt-get install -y --no-install-recommends \
    build-essential pkg-config curl file xdg-utils \
    libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
    libgtk-3-dev libglib2.0-dev libglib2.0-dev-bin libayatana-appindicator3-dev \
    librsvg2-dev libwayland-dev libxkbcommon-dev libssl-dev
  ```
- *(Opzionale)* Tauri CLI per `dev`/`build` dei pacchetti: `cargo install tauri-cli --version '^2' --locked`
- **Nessun Node.js richiesto** — il frontend è HTML/CSS/JS statico vanilla

### Sviluppo

```bash
git clone https://github.com/riccardomastrorilli15-cpu/pulse.git
cd pulse

# con tauri-cli
cargo tauri dev          # avvio in dev; la finestra parte nascosta → Alt+Space

# oppure solo compila ed esegui
cargo run --release --manifest-path src-tauri/Cargo.toml
```

### Produzione

```bash
# Linux: binario nativo + install in ~/.local
./scripts/install-linux.sh          # build + install binario, .desktop e icona

# pacchetti (.msi/.exe su Windows, .deb/.AppImage su Linux)
cargo tauri build
```

Il profilo release è già ottimizzato (`Cargo.toml`): `lto = true`, `codegen-units = 1`, `strip = true`, `opt-level = 3`.
Il link con **fat LTO richiede ~4+ GB di RAM**; su macchine piccole usa
`CARGO_PROFILE_RELEASE_LTO=thin CARGO_PROFILE_RELEASE_CODEGEN_UNITS=4 cargo build --release`
(verificato in sandbox con 1.9 GB RAM: build ok, binario ~20 MB).

---

## Uso quotidiano

| Tasto | Azione |
|---|---|
| `Alt+Space` | Mostra/nascondi Pulse (globale) |
| `↑` / `↓` | Naviga i risultati |
| `Tab` / `Shift+Tab` | Cambia modalità: Tutto → App → File → Appunti |
| `Invio` | Apri / avvia / **incolla** (negli appunti) |
| `Ctrl+Invio` | Apri la **cartella contenente** (`reveal_item_in_dir`) |
| `Ctrl+C` | Copia **percorso** (file) o **contenuto** (appunti) |
| `Ctrl+P` | Toggle pannello anteprima |
| `Delete` | (Appunti) elimina l'elemento selezionato |
| `Ctrl+Shift+X` | (Appunti) fissa/sblocca (pin) l'elemento |
| `Esc` | Nascondi la finestra |
| `cb ` + testo | Prefisso: salta direttamente agli appunti e filtra |

**Frecency in breve**: `score = fuzzy_nucelo × (1 + ln(1 + utilizzi)) × f(età)` dove `f(età) = 1/(1+giorni)` — più usi qualcosa *di recente*, più sale. Dunque scrivere `vsc` dopo aver aperto VS Code 100 volte lo mette al primo posto.

---

## Architettura

```
┌────────────────────────── processo Pulse ──────────────────────────┐
│                                                                    │
│  Thread UI (webview)            Thread indice (std::thread)        │
│  ┌────────────────┐             ┌───────────────────────────┐       │
│  │ ui/*.html|js   │             │ scan_apps()  (.desktop /  │       │
│  │   ↓ invoke()   │             │   Start Menu / Flatpak)   │       │
│  │ main.rs IPC    │   legge     │ scan_files() (ignore +    │       │
│  │ comandi ───────┼────────────►│   esclusioni node_modules…)│      │
│  └──────┬─────────┘  indice     └────────────┬──────────────┘       │
│         │                                     │ write (RwLock)      │
│         │        ┌────────────────────────────▼─────┐               │
│         │        │  Arc<RwLock<Vec<IndexItem>>>     │  ~250k cap    │
│         │        └──────────────────────────────────┘               │
│         │                                                           │
│  Thread watcher (notify)             Thread clipboard (arboard)     │
│  ┌──────────────────────┐            ┌──────────────────────────┐   │
│  │ eventi FS → debounce │            │ poll 450ms: testo/img    │   │
│  │ 500ms → upsert/drop  │            │ sha256 dedup → SQLite    │   │
│  └──────────────────────┘            │ img → PNG + thumb 384px  │   │
│                                      └────────────┬─────────────┘   │
│                                                   ▼                 │
│                              SQLite (WAL) ~/.local/share/pulse/     │
│                              frecency · clipboard · meta            │
│                                                   ▲                 │
│                        frecency cache in memoria (HashMap)          │
└───────────────────────────────────────────────────────────────────┘
```

**Flusso di una ricerca**: la UI invia `search(query, mode)` → il backend parsa la query con `nucleo-matcher` (smart case, smart normalization), valuta ogni candidato in memoria, moltiplica per il boost frecency, ordina e restituisce i top-40. Nessun accesso al disco: tipicamente microsecondi.

**Flusso clipboard**: ogni 450 ms il listener legge testo/immagine; se l'hash sha256 cambia: se il testo è *sensibile* (PEM/token/password=) viene scartato; altrimenti salvato in SQLite (`INSERT OR IGNORE` sui duplicati). Le immagini vengono scritte come PNG in `<cache>/pulse/images/<hash>.png` + thumbnail `..._thumb.png`.

### Struttura del repository

```
pulse/
├── .github/workflows/        # CI (fmt, clippy, test) + Release (tauri-action su tag v*)
├── flatpak/
│   ├── com.pulse.Launcher.yml        # manifest Flatpak (freedesktop SDK 24.08 + rust-stable)
│   ├── com.pulse.Launcher.desktop    # voce menu (installata nel flatpak)
│   └── com.pulse.Launcher.metainfo.xml  # AppStream (flathub-ready)
├── scripts/
│   ├── build-release.sh
│   └── install-linux.sh
├── src-tauri/
│   ├── Cargo.toml            # profilo release LTO/strip/codegen-units=1
│   ├── tauri.conf.json       # finestra undecorated, trasparente, sempre-in-alto
│   ├── capabilities/default.json
│   ├── icons/icon.png
│   └── src/
│       ├── main.rs           # finestra, tray, Alt+Space, comandi IPC, focus-loss → hide
│       ├── indexer.rs        # scan apps/file, fuzzy nucleo, frecency, watcher notify
│       ├── db.rs             # SQLite WAL: schema, frecency, CRUD clipboard (+ test)
│       ├── clipboard.rs      # listener daemon, dedup, filtro segreti, PNG/thumb
│       └── paste.rs          # Ctrl+V simulato (enigo) + fallback wtype (Wayland)
├── ui/                       # frontend statico vanilla (zero build step)
│   ├── index.html
│   ├── styles.css
│   └── main.js
└── README.md
```

---

## Flatpak

Quick start (build locale dal repo):

```bash
sudo apt install flatpak flatpak-builder
sudo flatpak remote-add --if-not-exists flathub https://flathub.org/repo/flathub.flatpakrepo
sudo flatpak install flathub org.freedesktop.Platform//24.08 org.freedesktop.Sdk//24.08
sudo flatpak install flathub org.freedesktop.Sdk.Extension.rust-stable//24.08

flatpak-builder --force-clean build-dir flatpak/com.pulse.Launcher.yml
flatpak-builder --user --install --force-clean build-dir flatpak/com.pulse.Launcher.yml
flatpak run com.pulse.Launcher
```

**Perché quei `finish-args`** (ognuno è necessario):

| Permesso | Perché |
|---|---|
| `--socket=wayland` (+ `--socket=fallback-x11`, `--share=ipc`) | Clipboard Wayland nativa; X11 per app XWayland; IPC per prestazioni shared-memory |
| `--device=dri` | Accelerazione GPU della webview |
| `--filesystem=home` | Un launcher **deve** vedere i tuoi file; è limitato alla home, mai `--filesystem=host` |
| `--socket=session-bus` + `--talk-name=org.freedesktop.portal.Desktop` + `...GlobalShortcuts` | Hotkey globali via **xdg-desktop-portal** (compatibile Wayland, dove X11-grab non funziona) e tray/notifiche |
| `--talk-name=org.freedesktop.Flatpak` | `flatpak-spawn --host <app>`: lanciare le app **host** (altrimenti nel sandbox non apri nulla) |

**Note Wayland (oneste)**:
- Gli hotkey globali passano dal portale `GlobalShortcuts` (xdg-desktop-portal ≥ 1.20, GNOME 47+/KDE 6). Se il portale non è disponibile, l'app resta usabile dal tray; su X11 l'hook globale funziona ovunque via `Alt+Space`.
- `Alt+Space` su GNOME potrebbe già essere occupato (attività/menu): riassegnalo in *Impostazioni → Tastiera → Scorciatoie*.
- Il paste automatico richiede il protocollo `virtual-keyboard` (KWin/Mutter: ok) o `wtype` installato; in caso contrario Pulse copia negli appunti e ti chiede `Ctrl+V` (toast).

**Per Flathub** (build 100% offline, richiesta): il manifest scarica crate con `--share=network`, quindi **sparirebbe da Flathub**. Per generare i sorgenti vendorizzati:

```bash
python3 -m pip install aiohttp
wget https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
python3 flatpak-cargo-generator.py src-tauri/Cargo.lock -o flatpak/cargo-sources.json
```

poi sostituisci il blocco `build-commands`/`sources` con:

```yaml
    build-commands:
      - cargo --offline build --release --locked --manifest-path src-tauri/Cargo.toml
      - install -Dm755 src-tauri/target/release/pulse /app/bin/pulse
      # ... (gli stessi install -Dm644 del manifest attuale)
    build-options:
      append-path: /usr/lib/sdk/rust-stable/bin
      env:
        CARGO_HOME: /run/build/pulse/cargo
    sources:
      - type: dir
        path: ..
        skip: [target, .git, build-dir]
      - cargo-sources.json
```

---

## Pubblicare sul tuo GitHub — guida completa

> Il progetto è già pronto: repo git con il primo commit, `.gitignore`, `LICENSE` (MIT), workflow CI/CD. Mancano solo le tue credenziali: **nessun comando qui sotto va eseguito senza di te**. Scegli il metodo A o B.

**Prerequisiti (una tantum)**

1. Account GitHub gratuito: https://github.com/signup
2. `git` installato sul tuo PC — controlla con `git --version`
   - Linux: `sudo apt install git` · macOS: `xcode-select --install` · Windows: https://git-scm.com/download/win
3. Identità git (adatta a te):
   ```bash
   git config --global user.name "Il Tuo Nome"
   git config --global user.email "tu@esempio.com"
   ```

**Metodo A — GitHub CLI `gh` (consigliato, un comando)**

```bash
# installa gh (https://cli.github.com): Linux → vedi repo ufficiale o 'sudo apt install gh';
# macOS → 'brew install gh'; Windows → 'winget install GitHub.cli'
gh auth login                       # GitHub.com → HTTPS → Login with a web browser → incolla il codice
cd pulse                            # cartella che contiene src-tauri/, ui/, .gitignore
gh repo create pulse --public --source=. --remote=origin --push
```

**Metodo B — dal sito github.com (senza CLI)**

1. Vai su https://github.com/new
2. *Repository name*: `pulse` — scegli *Public* o *Private*
3. **NON spuntare** "Add a README", "Add .gitignore", "Choose a license": il repo locale li ha già; selezionandoli dovrai risolvere conflitti
4. Clicca **Create repository**
5. Nella pagina che si apre copia l'URL (HTTPS o SSH), poi nel terminale:

   ```bash
   cd pulse
   git remote add origin https://github.com/riccardomastrorilli15-cpu/pulse.git   # oppure git@github.com:riccardomastrorilli15-cpu/pulse.git
   git branch -M main
   git push -u origin main
   ```

   - **HTTPS**: al primo push GitHub apre il browser per autorizzare; se chiede credenziali nel terminale, usa un **Personal Access Token** con permesso `repo` (https://github.com/settings/tokens) — le password normali non sono più accettate
   - **SSH** (niente password dopo la configurazione): `ssh-keygen -t ed25519 -C "tu@esempio.com"` → incolla il contenuto di `~/.ssh/id_ed25519.pub` su https://github.com/settings/keys → usa l'URL SSH sopra

**Dopo il push — verifica in 30 secondi**

- Apri `https://github.com/riccardomastrorilli15-cpu/pulse` → ci devono essere tutti i file
- Tab **Actions** → il run *CI* parte da solo (fmt + clippy + test): verde ✅ = tutto ok
- Settings → *General* → aggiungi descrizione e topics: `launcher`, `clipboard-manager`, `rust`, `tauri`, `spotlight`

**Nota sui link nel repo**

I link del README, gli badge CI e i metadati Flatpak puntano a `riccardomastrorilli15-cpu/pulse`. Se in futuro rinomini il repo o usi un'organizzazione diversa, aggiornali tutti con:

```bash
grep -rl riccardomastrorilli15-cpu --exclude-dir=.git --exclude-dir=target . | xargs sed -i 's/riccardomastrorilli15-cpu/nuovo-nome/g'
git add -A && git commit -m "docs: aggiorna link del repo" && git push
```

**Opzionale — release con installer**

```bash
git tag v0.1.0 && git push origin v0.1.0
```

Il workflow *Release* compila su Windows e Linux e apre una **bozza di release** con `.msi`/`.exe` e `.deb`/`.AppImage` (sezione Releases, un click per pubblicare).

**Opzionale — il “sito”**

Puoi ospitare gratis una pagina del progetto con **GitHub Pages**: repo → *Settings* → *Pages* → *Deploy from branch* → `main` / cartella `/ui` (mostra la demo interattiva della UI con dati finti). Se invece vuoi una vera **landing page** di presentazione (funzionalità, screenshot, bottoni download), chiedimelo e te la preparo.

**Problemi comuni**

- `remote origin already exists` → `git remote set-url origin https://github.com/riccardomastrorilli15-cpu/pulse.git`
- "repository not empty" / `fetch first` → il repo su GitHub non era vuoto: cancellalo e ricrealo vuoto (consigliato) oppure `git pull origin main --allow-unrelated-histories`
- `403 Permission denied` in push → auth fallita: Personal Access Token (HTTPS) oppure sistema la chiave SSH (`ssh -T git@github.com` per testare)
- `fatal: not a git repository` → sei nella cartella sbagliata: la corretta contiene `src-tauri/`, `ui/` e `.gitignore`
- Actions fallisce per WebKitGTK su `ubuntu-22.04` → il workflow installa già le dipendenze; se cambi runner aggiorna la lista `apt-get` in `.github/workflows/*.yml`

> Nota: l'anteprima/scaricabile di questo ambiente può scartare la cartella `.git`; in tal caso, dopo aver scaricato il progetto, riesegui:
> `git init -b main && git add -A && git commit -m "feat: Pulse v0.1.0"` e poi il `git remote add` + `push` dei punti precedenti.

---

## Test

```bash
cargo test --locked --manifest-path src-tauri/Cargo.toml   # db, frecency, matcher, is_sensitive, clean_exec
cargo clippy --locked --manifest-path src-tauri/Cargo.toml
cargo fmt --all -- --check
```

---

## Privacy & sicurezza

- **Zero rete**: nessuna richiesta HTTP esce dall'app; gli appunti restano sul disco locale in `~/.local/share/pulse/pulse.db` (Linux) / `%LOCALAPPDATA%\pulse\` (Windows)
- **Dedup**: hash sha256, un duplicato non viene mai reinserito
- **Filtro segreti**: PEM/`OPENSSH PRIVATE KEY`/`PGP`, AWS `AKIA…`, GitHub `ghp_…`/`github_pat_…`, Slack `xox…`, OpenAI `sk-…`, Stripe `sk_live_…`, GitLab `glpat-…`, `password=`/`client_secret=`/`access_token=`, JSON con `"password"`/`"private_key"` — tutti **scartati**
- **Limiti onesti di questa euristica**: non rileva la *finestra sorgente* (non portabile) né segreti con formato non noto; per copia da password manager il modo infallibile è il toggle **Modalità privacy** nella tray, che sospende ogni acquisizione
- **Cancellazione totale**: tray → *Cancella cronologia appunti* (un click) — elimina righe DB + file immagine in cache
- I **file sensibili non vengono letti**: l'anteprima di testo (`Ctrl+P`) salta file binari e oltre 2 MB

---

## Roadmap

- [ ] File di configurazione `settings.toml` (hotkey, esclusioni, tema chiaro)
- [ ] Calcolatrice / conversioni unità inline
- [ ] Window switching (X11 `_NET_ACTIVE_WINDOW`, portal `Window` su Wayland)
- [ ] Plugin WASM per comandi custom
- [ ] Sync opzionale e cifrato della sola frecency tra macchine personali

## FAQ

**Alt+Space non fa nulla su GNOME/Wayland.** Vai in Impostazioni → Tastiera → Scorciatoie e libera `Alt+Space`, oppure assegna un'altra combinazione in `src-tauri/src/main.rs` (`GLOBAL_SHORTCUT`).
**"Too many open files / inotify watches"** durante indicizzazione Linux: `sudo sysctl fs.inotify.max_user_watches=524288` e persiste in `/etc/sysctl.d/99-pulse.conf`.
**Windows 10: manca WebView2** → Tauri propone il bootstrapper; oppure installa [Evergreen WebView2](https://developer.microsoft.com/microsoft-edge/webview2/).
**Pulse non trova le app Flatpak.** Verifica che esistano `/var/lib/flatpak/exports/share/applications` o `~/.local/share/flatpak/exports/share/applications` (vengono scansionate).
**Inotify mangia RAM sull'indice?** No: l'indice vive in RAM come `Vec` (~pochi MB per 250k voci); i watch inotify sono contati a parte e standard su ogni desktop Linux.

## Licenza

[MIT](LICENSE) © 2026 Pulse contributors — libero per uso personale e commerciale. Nessuna garanzia.
