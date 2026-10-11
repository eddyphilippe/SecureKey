# SecureZip

SecureZip est un gestionnaire de fichiers chiffré et portable. Il transforme un dossier — en pratique une clé USB exFAT — en coffre : les fichiers, leurs noms et l’arborescence sont chiffrés. Il n’y a pas de disque virtuel, pas de FUSE et pas de pilote à installer.

L’application se lance depuis la clé, sur macOS, Windows ou Linux. Le format du coffre est le même partout.

## Utilisation

1. Formater la clé en exFAT.
2. Copier SecureZip pour chaque système, selon la disposition décrite dans [BUILD.md](BUILD.md).
3. Lancer SecureZip.
4. Créer un coffre, ou ouvrir le dossier `Vault` déjà présent.
5. Choisir un mot de passe d’au moins 8 caractères. Une phrase longue est préférable.
6. Ajouter, renommer, exporter ou supprimer des fichiers.
7. Verrouiller le coffre avant de retirer la clé.

Un coffre créé ici s’ouvre sans conversion sur un autre système.

Le mode **lecture seule** n’écrit rien dans le coffre. Seuls l’export et l’ouverture temporaire sont possibles. Sur une machine inconnue, c’est le mode à utiliser.

## Ce que voit une personne qui branche la clé

```text
SECUREZIP/
  Mac/SecureZip.app           Mac Apple Silicon
  Mac/SecureZip Intel.app     Mac Intel
  Windows/SecureZip.exe
  Linux/SecureZip
  Vault/
    vault.meta
    index.enc
    objects/
      1a7c832f….dat
  README.txt
```

`vault.meta` contient les paramètres publics de dérivation et la clé maître sous forme chiffrée. Il ne contient ni le mot de passe, ni les noms de fichiers. Le détail du format est dans [FORMAT.md](FORMAT.md). Les limites du chiffrement sont dans [SECURITY.md](SECURITY.md).

## Téléchargement

Les trois versions et le mode d’emploi sont réunis dans `SECUREZIP.zip`, sur la page des publications du dépôt GitHub. Copiez le dossier obtenu sur la clé, ou gardez-le sur l’ordinateur.

## Développement

Prérequis : Rust stable (1.85 ou plus récent), Node.js 22, et les outils de compilation indiqués par Tauri pour votre système.

```bash
cargo test --manifest-path src-tauri/Cargo.toml
cargo audit --file src-tauri/Cargo.lock
npm install
npm run tauri dev
```

Le test d’un fichier de 2 Gio est ignoré par défaut :

```bash
cargo test --manifest-path src-tauri/Cargo.toml -- --ignored streams_multiple_gigabytes
```

## Licence

MIT. Le projet est écrit pour pouvoir être lu et audité : la cryptographie n’est pas obscurcie, et aucune primitive n’est inventée ici.
