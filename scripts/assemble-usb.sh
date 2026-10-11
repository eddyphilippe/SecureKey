#!/bin/sh
# Copie les binaires déjà compilés vers une arborescence de clé USB.
# Ne cross-compile pas : lancez d'abord le build sur chaque système, ou
# assemblez la clé en copiant les dossiers Mac, Windows et Linux un par un.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
dest=${1:-"$root/NomadVault"}
mkdir -p "$dest/Mac" "$dest/Windows" "$dest/Linux" "$dest/Vault/objects"
cp "$root/packaging/README.txt" "$dest/README.txt"
app=$(find "$root/src-tauri/target/release/bundle/macos" -name "NomadVault.app" -maxdepth 2 2>/dev/null | head -n 1 || true)
if [ -n "$app" ]; then
  rm -rf "$dest/Mac/NomadVault.app"
  cp -R "$app" "$dest/Mac/NomadVault.app"
fi
exe=$(find "$root/src-tauri/target/release" -name "NomadVault.exe" -maxdepth 3 2>/dev/null | head -n 1 || true)
if [ -n "$exe" ]; then
  cp "$exe" "$dest/Windows/NomadVault.exe"
fi
bin="$root/src-tauri/target/release/NomadVault"
if [ -f "$bin" ] && [ "$(uname)" != "Darwin" ]; then
  cp "$bin" "$dest/Linux/NomadVault"
  chmod +x "$dest/Linux/NomadVault"
fi
printf 'Dossier préparé : %s\n' "$dest"
