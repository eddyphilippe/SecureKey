# Compiler SecureZip

SecureZip est une application [Tauri 2](https://v2.tauri.app/) dont le moteur est en Rust. Chaque système se compile sur lui-même. Le format du coffre ne dépend pas du système : un coffre créé sur l’un s’ouvre sur les autres.

## Prérequis

- Rust stable, 1.85 minimum (`rustup`).
- Node.js 22 et npm.
- Les dépendances système de Tauri : <https://v2.tauri.app/start/prerequisites/>.

```bash
cargo test --manifest-path src-tauri/Cargo.toml
npm install
npm run tauri dev
npm run tauri build
```

Le binaire s’appelle `SecureZip`. Les artefacts Tauri se trouvent sous `src-tauri/target/release/bundle/`.

## Clé USB

```text
SECUREZIP/
  Mac/SecureZip.app           Mac Apple Silicon
  Mac/SecureZip Intel.app     Mac Intel
  Windows/SecureZip.exe
  Linux/SecureZip
  Vault/
  README.txt
```

`scripts/assemble-usb.sh` copie ce qui a déjà été compilé sur la machine courante et le texte d’accompagnement. Il ne cross-compile pas. Le dossier `Vault` est créé vide : le coffre lui-même naît au premier lancement, sur n’importe quel système.

Les préférences du coffre (délai de verrouillage, favoris, commentaires) sont dans l’index chiffré. Rien de sensible n’est écrit dans le profil de l’utilisateur.

## macOS

Cibles utiles :

- `aarch64-apple-darwin` (Apple Silicon) ;
- `x86_64-apple-darwin` (Intel).

Un binaire universel se construit avec les deux cibles installées (`rustup target add …`) puis le réglage de compilation universel de Tauri, documenté ici : <https://v2.tauri.app/distribute/macos/>.

Signature et notarisation, une fois un compte Apple Developer disponible :

1. Créer un certificat **Developer ID Application** dans le trousseau.
2. Renseigner `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (mot de passe d’application) et `APPLE_TEAM_ID`.
3. Dans `src-tauri/tauri.conf.json`, section `bundle.macOS`, ajouter l’identité de signature. Tauri signe pendant `npm run tauri build` si l’identité est présente.
4. Notariser l’archive :

```bash
xcrun notarytool submit "SecureZip.app.zip" \
  --apple-id "$APPLE_ID" \
  --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_PASSWORD" \
  --wait
xcrun stapler staple "SecureZip.app"
```

5. Vérifier : `spctl --assess --type execute -v SecureZip.app`.

Sans signature, Gatekeeper bloque l’application lancée depuis une clé. La signature et la notarisation ne changent pas le format du coffre. Cette version du dépôt ne contient pas de certificat.

`minimumSystemVersion` est 11.0.

## Windows

Cible de cette version : `x86_64-pc-windows-msvc`. ARM64 pourra être ajouté plus tard (`aarch64-pc-windows-msvc`) sans changer le coffre.

Le build Tauri produit un exécutable autonome. Ne pas proposer d’installateur comme seul mode : copier `SecureZip.exe` dans `Windows/` sur la clé. Les WebView2 sont déjà présents sur Windows 10/11 à jour ; sinon, l’amorçage documenté par Tauri s’applique.

La signature de code (Authenticode) n’est pas branchée. Elle pourra utiliser `signtool` sur l’exécutable final, avec un certificat de l’éditeur, sans effet sur les fichiers du coffre.

## Linux

Sur une machine Linux :

```bash
npm run tauri build
```

Tauri peut produire un AppImage et un `.deb` selon les cibles activées dans la configuration de bundle. Pour la clé, copier le binaire portable ou l’AppImage vers `Linux/SecureZip` et le rendre exécutable (`chmod +x`). Le bit exécutable ne fait pas partie du coffre : exFAT peut le perdre, et l’utilisateur devra parfois le rétablir après une copie. Les données, elles, restent de simples fichiers.

## Tests croisés

`cargo test` couvre la création, les mots de passe, les fichiers vides et multi-blocs, Unicode, la corruption, l’annulation, le changement de mot de passe, l’export et la lecture seule. Le test de 2 Gio est ignoré par défaut.

Un essai manuel indispensable, une fois les trois binaires construits :

1. Créer un coffre sur macOS.
2. L’ouvrir et le modifier sur Windows.
3. L’ouvrir sur Linux.
4. L’ouvrir à nouveau sur macOS.

Aucune conversion ne doit être nécessaire.
