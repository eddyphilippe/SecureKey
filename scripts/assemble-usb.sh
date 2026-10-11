#!/bin/sh
# Copie les binaires déjà compilés vers une arborescence de clé USB.
# Ne cross-compile pas : lancez d'abord le build sur chaque système, ou
# assemblez la clé en copiant les dossiers Mac, Windows et Linux un par un.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
dest=${1:-"$root/SECUREZIP"}
mkdir -p "$dest/Mac" "$dest/Windows" "$dest/Linux" "$dest/Vault/objects"
cp "$root/packaging/README.txt" "$dest/README.txt"
app=$(find "$root/src-tauri/target/release/bundle/macos" -name "SecureZip.app" -maxdepth 2 2>/dev/null | head -n 1 || true)
if [ -n "$app" ]; then
  rm -rf "$dest/Mac/SecureZip.app"
  cp -R "$app" "$dest/Mac/SecureZip.app"
fi
exe=$(find "$root/src-tauri/target/release" -name "SecureZip.exe" -maxdepth 3 2>/dev/null | head -n 1 || true)
if [ -n "$exe" ]; then
  cp "$exe" "$dest/Windows/SecureZip.exe"
fi
bin="$root/src-tauri/target/release/SecureZip"
if [ -f "$bin" ] && [ "$(uname)" != "Darwin" ]; then
  cp "$bin" "$dest/Linux/SecureZip"
  chmod +x "$dest/Linux/SecureZip"
fi
printf 'Dossier préparé : %s\n' "$dest"
