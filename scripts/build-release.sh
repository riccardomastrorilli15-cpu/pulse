#!/usr/bin/env bash
# Build di produzione per il binario nativo (Linux; su Windows usare cargo tauri build).
set -euo pipefail
cd "$(dirname "$0")/.."
command -v cargo >/dev/null || { echo "cargo non trovato: installa Rust via rustup" >&2; exit 1; }
echo "▸ cargo build --release --locked"
cargo build --release --locked --manifest-path src-tauri/Cargo.toml
BIN=src-tauri/target/release/pulse
echo "▸ Dimensione binario: $(du -h "$BIN" | cut -f1)"
command -v sha256sum >/dev/null && sha256sum "$BIN"
echo "✔ Fatto: $BIN"
