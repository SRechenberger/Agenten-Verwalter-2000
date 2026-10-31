#!/usr/bin/env bash
# Startet Agenten Verwalter 2000 im Entwicklungsmodus.
cd "$(dirname "$(readlink -f "$0")")" || exit 1
pnpm tauri dev || read -rp "Fehler – Enter schließt das Fenster."
