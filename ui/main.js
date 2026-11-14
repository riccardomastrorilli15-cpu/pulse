/* Pulse — logica UI (vanilla JS, zero dipendenze).
   IPC con il backend Rust via window.__TAURI__ (withGlobalTauri).
   Senza Tauri (apertura diretta nel browser) gira in modalità DEMO con dati finti. */

'use strict';

/* ============================================================
   Ponte IPC + modalità demo
   ============================================================ */
const tauri = window.__TAURI__;
const isTauri = !!(tauri && tauri.core && tauri.core.invoke);

async function invoke(cmd, args) {
  if (isTauri) return tauri.core.invoke(cmd, args);
  return demoInvoke(cmd, args);
}
function listen(event, cb) {
  if (isTauri && tauri.event) return tauri.event.listen(event, cb);
  return Promise.resolve(() => {});
}

/* ---------- Dati demo (solo browser) ---------- */
const DEMO_ITEMS = [
  { name: 'Visual Studio Code', path: '/usr/share/applications/code.desktop', kind: 'app', exec: 'code', used_count: 214, last_used: nowSec() - 900 },
  { name: 'Firefox',            path: '/usr/share/applications/firefox.desktop', kind: 'app', exec: 'firefox', used_count: 187, last_used: nowSec() - 3600 },
  { name: 'htop', path: '/usr/share/applications/htop.desktop', kind: 'app', exec: 'htop', used_count: 12, last_used: nowSec() - 86400 },
  { name: 'Terminale', path: '/usr/share/applications/org.gnome.Terminal.desktop', kind: 'app', exec: 'gnome-terminal', used_count: 156, last_used: nowSec() - 300 },
  { name: 'Impostazioni', path: '/usr/share/applications/gnome-control-center.desktop', kind: 'app', exec: 'gnome-control-center', used_count: 8, last_used: nowSec() - 400000 },
  { name: 'Documenti', path: '/home/tu/Documenti', kind: 'dir', used_count: 34, last_used: nowSec() - 7200 },
  { name: 'Progetti', path: '/home/tu/Progetti', kind: 'dir', used_count: 91, last_used: nowSec() - 1500 },
  { name: 'fattura-2026-11.pdf', path: '/home/tu/Documenti/fattura-2026-11.pdf', kind: 'file', used_count: 2, last_used: nowSec() - 90000 },
  { name: 'pitch-deck.pptx', path: '/home/tu/Documenti/pitch-deck.pptx', kind: 'file', used_count: 0, last_used: 0 },
  { name: 'main.rs', path: '/home/tu/Progetti/pulse/src-tauri/src/main.rs', kind: 'file', used_count: 47, last_used: nowSec() - 120 },
];
const DEMO_CLIPBOARD = [
  { id: 1, kind: 'code', content: 'fn main() {\n    println!("ciao da Pulse");\n}', preview: 'fn main() { println!("ciao da Pulse"); }', image: null, created_at: nowSec() - 240, pinned: true, bytes: 46 },
  { id: 2, kind: 'link', content: 'https://github.com/TUO-USER/pulse', preview: 'https://github.com/TUO-USER/pulse', image: null, created_at: nowSec() - 900, pinned: false, bytes: 38 },
  { id: 3, kind: 'emoji', content: '🔥🚀✨🎯⚡', preview: '🔥🚀✨🎯⚡', image: null, created_at: nowSec() - 3600, pinned: false, bytes: 20 },
  { id: 4, kind: 'text', content: 'Via Mannelli 10, 50136 Firenze (FI) — aggiusta per la spedizione di giovedì mattina', preview: 'Via Mannelli 10, 50136 Firenze (FI) — aggiusta per la spedizione di giovedì mattina', image: null, created_at: nowSec() - 7200, pinned: false, bytes: 84 },
  { id: 5, kind: 'text', content: 'SELECT * FROM clipboard WHERE kind = \'text\' ORDER BY created_at DESC LIMIT 50;', preview: "SELECT * FROM clipboard WHERE kind = 'text' ORDER BY created_at DESC LIMIT 50;", image: null, created_at: nowSec() - 86400, pinned: false, bytes: 74 },
];

function nowSec() { return Math.floor(Date.now() / 1000); }

function demoInvoke(cmd, args = {}) {
  switch (cmd) {
    case 'search': {
      const q = (args.query || '').trim().toLowerCase();
      let pool = DEMO_ITEMS.filter(i => args.mode === 'apps' ? i.kind === 'app' : args.mode === 'files' ? i.kind !== 'app' : true);
      if (q) pool = pool.filter(i => i.name.toLowerCase().includes(q));
      else pool = pool.filter(i => i.used_count > 0).sort((a, b) => b.used_count - a.used_count);
      return Promise.resolve(pool.slice(0, 30));
    }
    case 'list_clipboard': {
      let pool = DEMO_CLIPBOARD.slice();
      const q = (args.query || '').trim().toLowerCase();
      if (q) pool = pool.filter(i => i.content.toLowerCase().includes(q));
      return Promise.resolve(pool);
    }
    case 'open_item': return Promise.resolve().then(() => showToast(`(demo) Aprirei: ${args.path}`));
    case 'reveal_item': return Promise.resolve().then(() => showToast(`(demo) Aprirei la cartella di: ${args.path}`));
    case 'copy_path': return Promise.resolve().then(() => showToast('(demo) Copiato negli appunti'));
    case 'read_preview': return Promise.resolve('// anteprima demo\nfn main() {\n    println!("Ciao da Pulse!");\n}\n');
    case 'paste_clipboard': return Promise.resolve().then(() => showToast('(demo) Elemento rimesso negli appunti — premi Ctrl+V'));
    case 'delete_clipboard_item': return Promise.resolve();
    case 'toggle_pin': return Promise.resolve(true);
    case 'clear_clipboard': return Promise.resolve(DEMO_CLIPBOARD.length);
    case 'get_settings': return Promise.resolve({ privacy_mode: false, global_shortcut: 'Alt+Space', index_count: DEMO_ITEMS.length });
    case 'set_privacy_mode': return Promise.resolve();
    case 'hide_window': return Promise.resolve().then(() => showToast('(demo) La finestra si chiuderebbe qui'));
    default: return Promise.reject(new Error('comando sconosciuto: ' + cmd));
  }
}

/* ============================================================
   Stato & riferimenti
   ============================================================ */
const state = {
  mode: 'all',            // all | apps | files | clipboard
  query: '',
  results: [],            // elementi correnti (file o clipboard)
  selected: 0,
  preview: false,         // pannello anteprima visibile
};

const $ = (sel) => document.querySelector(sel);
const searchInput = $('#search');
const resultsEl = $('#results');
const modesEl = $('#modes');
const statusEl = $('#status');
const previewEl = $('#preview');
const previewBody = $('#preview-body');
const previewTitle = $('#preview-title');
const previewFoot = $('#preview-foot');
const clearBtn = $('#clear-clip');
const toastEl = $('#toast');

const MODES = ['all', 'apps', 'files', 'clipboard'];
const MODE_LABEL = { all: 'Tutto', apps: 'App', files: 'File', clipboard: 'Appunti' };

/* ============================================================
   Icone SVG (stroke, stile Feather)
   ============================================================ */
const SVG = {
  app: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3.5" y="3.5" width="7" height="7" rx="1.6"/><rect x="13.5" y="3.5" width="7" height="7" rx="1.6"/><rect x="3.5" y="13.5" width="7" height="7" rx="1.6"/><rect x="13.5" y="13.5" width="7" height="7" rx="1.6"/></svg>',
  file: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><polyline points="13 2 13 9 20 9"/></svg>',
  dir: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>',
  link: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/></svg>',
  text: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><line x1="4" y1="7" x2="20" y2="7"/><line x1="4" y1="12" x2="20" y2="12"/><line x1="4" y1="17" x2="13" y2="17"/></svg>',
  code: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><polyline points="16 18 22 12 16 6"/><polyline points="8 6 2 12 8 18"/></svg>',
  image: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>',
  clipboard: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1"/></svg>',
  empty: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"><circle cx="11" cy="11" r="7"/><line x1="21" y1="21" x2="16.5" y2="16.5"/><line x1="8.5" y1="11" x2="13.5" y2="11"/></svg>',
};
function iconFor(item) {
  if (state.mode === 'clipboard' || item.clipKind) {
    const k = item.kind;
    switch (k) {
      case 'link': return SVG.link;
      case 'emoji': return null; // render emoji as text
      case 'image': return SVG.image;
      case 'code': return SVG.code;
      default: return SVG.text;
    }
  }
  switch (item.kind) {
    case 'app': return SVG.app;
    case 'dir': return SVG.dir;
    default: return SVG.file;
  }
}

/* ============================================================
   Utilità
   ============================================================ */
function el(tag, cls, text) {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
}
function timeAgo(epochSec) {
  if (!epochSec) return 'mai';
  const diff = Math.max(0, nowSec() - epochSec);
  if (diff < 60) return 'adesso';
  const min = Math.floor(diff / 60);
  if (min < 60) return `${min}m fa`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h}h fa`;
  const d = Math.floor(h / 24);
  if (d === 1) return 'ieri';
  if (d < 30) return `${d}g fa`;
  return new Date(epochSec * 1000).toLocaleDateString('it-IT');
}
function formatBytes(n) {
  if (!n) return '';
  const units = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  while (n >= 1024 && i < units.length - 1) { n /= 1024; i++; }
  return `${n.toFixed(n >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}
let toastTimer = null;
function showToast(msg) {
  toastEl.textContent = msg;
  toastEl.classList.add('show');
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => toastEl.classList.remove('show'), 2600);
}

/* ============================================================
   Caricamento dati
   ============================================================ */
async function loadResults() {
  try {
    if (state.mode === 'clipboard') {
      state.results = await invoke('list_clipboard', { query: state.query, limit: 60 });
    } else {
      state.results = await invoke('search', { query: state.query, mode: state.mode });
    }
  } catch (err) {
    console.error(err);
    state.results = [];
  }
  state.selected = 0;
  render();
}

function render() {
  resultsEl.textContent = '';
  clearBtn.hidden = state.mode !== 'clipboard';

  if (state.results.length === 0) {
    const empty = el('div', 'empty');
    empty.innerHTML = SVG.empty;
    const p = el('p', null, state.mode === 'clipboard' ? 'Cronologia appunti vuota' : 'Nessun risultato');
    const small = el('small', null, state.mode === 'clipboard'
      ? 'Tutto ciò che copi apparirà qui. Usa la modalità privacy dal menu nella tray per sospendere.'
      : 'Prova con parole meno restrittive — la ricerca è fuzzy (es. “vscod” → Visual Studio Code)');
    empty.appendChild(p);
    empty.appendChild(small);
    resultsEl.appendChild(empty);
    updatePreview();
    return;
  }

  state.results.forEach((item, i) => {
    const row = el('div', 'row');
    row.setAttribute('role', 'option');
    row.id = `row-${i}`;
    row.setAttribute('aria-selected', String(i === state.selected));
    row.style.animationDelay = `${Math.min(i, 16) * 16}ms`;

    // Icona o thumbnail immagine
    const isImg = state.mode === 'clipboard' && item.kind === 'image' && item.image;
    if (isImg) {
      const img = document.createElement('img');
      img.className = 'thumb';
      img.alt = item.content ? `Anteprima immagine ${item.content}` : 'Immagine copiata';
      img.src = item.image;
      row.appendChild(img);
    } else if (state.mode === 'clipboard' && item.kind === 'emoji') {
      const icon = el('div', 'icon k-emoji', item.content.slice(0, 4));
      row.appendChild(icon);
    } else {
      const icon = el('div', `icon k-${iconKey(item)}`);
      icon.innerHTML = iconFor(item) || SVG.text;
      row.appendChild(icon);
    }

    const meta = el('div', 'meta');
    const name = el('div', 'name');
    if (state.mode === 'clipboard' && item.pinned) {
      const pin = el('span', 'pin', '📌');
      name.appendChild(pin);
    }
    name.appendChild(document.createTextNode(
      state.mode === 'clipboard' ? clipTitle(item) : item.name
    ));
    const sub = el('div', 'sub',
      state.mode === 'clipboard'
        ? `${timeAgo(item.created_at)} · ${formatBytes(item.bytes) || 'testo'}`
        : item.path);
    meta.appendChild(name);
    meta.appendChild(sub);
    row.appendChild(meta);

    const badge = el('span', 'badge', badgeText(item));
    row.appendChild(badge);

    row.addEventListener('mousemove', () => { if (state.selected !== i) { state.selected = i; paintSelection(); } });
    row.addEventListener('click', () => { state.selected = i; paintSelection(); activateSelected(); });
    resultsEl.appendChild(row);
  });
  updatePreview();
}

function iconKey(item) {
  if (state.mode === 'clipboard') {
    return ['link', 'emoji', 'image', 'text', 'code'].includes(item.kind) ? (item.kind === 'code' ? 'text' : item.kind) : 'text';
  }
  return item.kind;
}
function clipTitle(item) {
  const one = (item.preview || item.content || '').replace(/\s+/g, ' ').trim();
  return one.length > 110 ? one.slice(0, 110) + '…' : one || '(immagine)';
}
function badgeText(item) {
  if (state.mode === 'clipboard') {
    return { text: 'Testo', link: 'Link', emoji: 'Emoji', image: 'Img', code: 'Codice' }[item.kind] || 'Appunti';
  }
  return item.kind === 'app' ? 'App' : item.kind === 'dir' ? 'Cartella' : 'File';
}

function paintSelection() {
  const rows = resultsEl.querySelectorAll('.row');
  rows.forEach((row, i) => row.setAttribute('aria-selected', String(i === state.selected)));
  const active = rows[state.selected];
  if (active) {
    active.scrollIntoView({ block: 'nearest' });
    searchInput.setAttribute('aria-activedescendant', active.id);
  }
  updatePreview();
}

/* ============================================================
   Anteprima
   ============================================================ */
async function updatePreview() {
  if (!state.preview) { previewEl.hidden = true; return; }
  const item = state.results[state.selected];
  if (!item) { previewEl.hidden = true; return; }
  previewEl.hidden = false;
  previewBody.textContent = '';
  previewFoot.textContent = '';

  if (state.mode === 'clipboard') {
    previewTitle.textContent = item.kind === 'image' ? 'Immagine' : 'Contenuto';
    if (item.kind === 'image' && item.image) {
      const img = document.createElement('img');
      img.src = item.image;
      img.alt = 'Immagine copiata negli appunti';
      previewBody.appendChild(img);
      previewFoot.textContent = `${item.content || ''} · ${formatBytes(item.bytes)} · salvata in cache locale`;
    } else {
      previewBody.textContent = item.content || '';
      previewFoot.textContent = `Copiata ${timeAgo(item.created_at)} · ${formatBytes(item.bytes)}`;
    }
    return;
  }

  if (item.kind === 'app') {
    previewTitle.textContent = 'Applicazione';
    previewBody.textContent = `${item.name}\n\nEseguibile / Exec:\n${item.exec || '(apertura via ShellExecute)'}\n\nFile desktop:\n${item.path}`;
    previewFoot.textContent = 'Invio per aprire · Ctrl+C per copiare il nome';
    return;
  }

  previewTitle.textContent = item.kind === 'dir' ? 'Cartella' : 'File';
  previewFoot.textContent = item.path;
  if (item.kind === 'dir') {
    previewBody.textContent = '(cartella)\n\nPremi Ctrl+Enter per aprirla nel file manager.';
    return;
  }
  previewBody.textContent = '…';
  try {
    previewBody.textContent = await invoke('read_preview', { path: item.path });
  } catch {
    previewBody.textContent = '(anteprima non disponibile)';
  }
}

/* ============================================================
   Azioni
   ============================================================ */
async function activateSelected() {
  const item = state.results[state.selected];
  if (!item) return;

  if (state.mode === 'clipboard') {
    try {
      await invoke('paste_clipboard', { id: item.id });
      showToast('Elemento copiato negli appunti — premi Ctrl+V');
    } catch (e) {
      showToast('Errore: ' + (e && e.message ? e.message : e));
    }
    return;
  }
  try {
    await invoke('open_item', { path: item.path, kind: item.kind, exec: item.exec || null });
  } catch (e) {
    showToast('Errore apertura: ' + (e && e.message ? e.message : e));
  }
}

async function revealSelected() {
  const item = state.results[state.selected];
  if (!item || state.mode === 'clipboard') return;
  try { await invoke('reveal_item', { path: item.path }); }
  catch (e) { showToast('Impossibile rivelare il percorso'); }
}

async function copySelected() {
  const item = state.results[state.selected];
  if (!item) return;
  const text = state.mode === 'clipboard' ? (item.content || '') : item.path;
  try {
    await invoke('copy_path', { path: text });
    showToast(state.mode === 'clipboard' ? 'Contenuto copiato' : 'Percorso copiato negli appunti');
  } catch { showToast('Copia non riuscita'); }
}

async function deleteSelected() {
  const item = state.results[state.selected];
  if (!item || state.mode !== 'clipboard') return;
  try {
    await invoke('delete_clipboard_item', { id: item.id });
    state.results.splice(state.selected, 1);
    state.selected = Math.max(0, Math.min(state.selected, state.results.length - 1));
    render();
  } catch { showToast('Eliminazione non riuscita'); }
}

async function togglePinSelected() {
  const item = state.results[state.selected];
  if (!item || state.mode !== 'clipboard') return;
  try {
    item.pinned = await invoke('toggle_pin', { id: item.id });
    render();
  } catch { showToast('Pin non riuscito'); }
}

function setMode(mode) {
  if (!MODES.includes(mode)) return;
  state.mode = mode;
  state.query = '';
  searchInput.value = '';
  modesEl.querySelectorAll('.mode').forEach(b => {
    const active = b.dataset.mode === mode;
    b.classList.toggle('active', active);
    b.setAttribute('aria-selected', String(active));
  });
  searchInput.placeholder = mode === 'clipboard'
    ? 'Filtra gli appunti… (incolla con Invio)'
    : mode === 'apps' ? 'Cerca tra le applicazioni…'
    : mode === 'files' ? 'Cerca file e cartelle…'
    : 'Cerca app, file, cartelle… (scrivi “cb ” per gli appunti)';
  loadResults();
  searchInput.focus();
}

function cycleMode(delta) {
  const idx = MODES.indexOf(state.mode);
  setMode(MODES[(idx + delta + MODES.length) % MODES.length]);
}

/* ============================================================
   Eventi
   ============================================================ */
searchInput.addEventListener('input', () => {
  const raw = searchInput.value;
  // Prefisso "cb " → modalità appunti (come da specifica)
  if (state.mode !== 'clipboard' && /^cb[\s]/i.test(raw)) {
    const rest = raw.replace(/^cb[\s]+/i, '');
    setMode('clipboard');
    searchInput.value = rest;
    state.query = rest;
    loadResults();
    return;
  }
  state.query = raw;
  loadResults();
});

window.addEventListener('keydown', (e) => {
  const isMod = e.ctrlKey || e.metaKey;

  switch (e.key) {
    case 'ArrowDown': e.preventDefault(); moveSelection(1); return;
    case 'ArrowUp':   e.preventDefault(); moveSelection(-1); return;
    case 'Enter':
      e.preventDefault();
      if (isMod) { revealSelected(); } else { activateSelected(); }
      return;
    case 'Tab': e.preventDefault(); cycleMode(e.shiftKey ? -1 : 1); return;
    case 'Escape':
      e.preventDefault();
      invoke('hide_window').catch(() => {});
      return;
    case 'Delete':
      if (state.mode === 'clipboard') { e.preventDefault(); deleteSelected(); }
      return;
    case 'p': case 'P':
      if (isMod) { e.preventDefault(); state.preview = !state.preview; updatePreview(); }
      return;
    case 'c': case 'C':
      if (isMod && !window.getSelection().toString()) { e.preventDefault(); copySelected(); }
      return;
    case 'x': case 'X':
      if (isMod && e.shiftKey && state.mode === 'clipboard') { e.preventDefault(); togglePinSelected(); }
      return;
  }
});

function moveSelection(delta) {
  if (!state.results.length) return;
  state.selected = (state.selected + delta + state.results.length) % state.results.length;
  paintSelection();
}

modesEl.addEventListener('click', (e) => {
  const btn = e.target.closest('.mode');
  if (btn) setMode(btn.dataset.mode);
});

clearBtn.addEventListener('click', async () => {
  try {
    const n = await invoke('clear_clipboard');
    showToast(`Cronologia eliminata (${n} elementi)`);
    loadResults();
  } catch { showToast('Operazione non riuscita'); }
});

$('#preview-close').addEventListener('click', () => { state.preview = false; updatePreview(); });

/* ============================================================
   Eventi dal backend (indice pronto, clipboard cambiata, toast)
   ============================================================ */
listen('index-ready', (e) => {
  const n = e && e.payload != null ? e.payload : '?';
  statusEl.innerHTML = '';
  const b = document.createElement('b');
  b.textContent = String(n);
  statusEl.appendChild(b);
  statusEl.appendChild(document.createTextNode(' elementi indicizzati'));
});
listen('clipboard-changed', () => {
  if (state.mode === 'clipboard') loadResults();
});
listen('toast', (e) => { if (e && e.payload) showToast(String(e.payload)); });

/* ============================================================
   Avvio
   ============================================================ */
(async function init() {
  if (!isTauri) {
    statusEl.textContent = 'Demo — apri dentro Pulse per i dati reali';
  } else {
    try {
      const s = await invoke('get_settings');
      statusEl.innerHTML = '';
      const b = document.createElement('b');
      b.textContent = String(s.index_count);
      statusEl.appendChild(b);
      statusEl.appendChild(document.createTextNode(` elementi · ${s.global_shortcut}`));
    } catch { statusEl.textContent = 'Pulse'; }
  }
  searchInput.focus();
  loadResults();
})();
