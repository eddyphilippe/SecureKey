# NomadVault

NomadVault est un gestionnaire de fichiers chiffré et portable. Il transforme un dossier — en pratique une clé USB exFAT — en coffre : les fichiers, leurs noms et l’arborescence sont chiffrés. Il n’y a pas de disque virtuel, pas de FUSE et pas de pilote à installer.

L’application se lance depuis la clé, sur macOS, Windows ou Linux. Le format du coffre est le même partout.

## Compatibilité

| Machine | Fichier | Système | Essai |
| --- | --- | --- | --- |
| Mac Apple Silicon | `Mac/NomadVault.app` | macOS 11 ou plus récent | L’application s’ouvre |
| Mac Intel | `Mac/NomadVault Intel.app` | macOS 11 (Big Sur) ou plus récent | Big Sur 11.7.11 |
| Linux, PC 64 bits | `Linux/NomadVault` | Distribution récente (Mint, Ubuntu, Debian, Fedora) | Linux Mint, Cinnamon 6.2.9 |
| Windows, PC 64 bits | `Windows/NomadVault.exe` | Windows 10 ou 11, à jour | Pas encore ouvert sur un PC Windows |

Ne sont pas visés : macOS avant Big Sur, Linux 32 bits, Raspberry Pi et autres machines ARM, Windows 7, Windows 8, Windows 32 bits, Windows sur processeur ARM.

Sur un Mac Intel, ouvrir `NomadVault Intel.app`. L’autre icône est celle des Mac Apple Silicon ; un Mac Intel la signale comme endommagée.

La clé se formate en exFAT, lisible sur les trois systèmes. Un fichier du coffre peut dépasser 4 Go. Le FAT16 s’arrête vers 2 Go. Le FAT32 bloque les fichiers de plus de 4 Go.

## Utilisation

Le dossier `Vault` livré dans le téléchargement est vide. Il n’y a pas encore de fichier `vault.meta`. Si vous le sélectionnez avec **Ouvrir un coffre existant**, NomadVault propose de créer le coffre à cet endroit.

1. Lancer l’application de son système (voir ci-dessous).
2. Choisir **Créer un nouveau coffre**, puis un dossier vide sur la clé, par exemple `MonCoffre`.
3. Choisir un mot de passe d’au moins 8 caractères. Une phrase longue est préférable.
4. Ajouter, renommer, exporter ou supprimer des fichiers.
5. Verrouiller le coffre avant de retirer la clé.

Pour rouvrir : **Ouvrir un coffre existant**, puis sélectionner le dossier `Vault` qui contient `vault.meta`. Pas la clé entière, et pas le dossier `objects`.

Un coffre créé ici s’ouvre sans conversion sur un autre système.

Le mode **lecture seule** n’écrit rien dans le coffre. Seuls l’export et l’ouverture temporaire sont possibles. Sur une machine inconnue, c’est le mode à utiliser.

Le programme copié sur le bureau ne découvre pas la clé tout seul. Il faut choisir le dossier du coffre.

### Mac

Double-clic sur `NomadVault.app` (Apple Silicon) ou `NomadVault Intel.app` (Intel).

### Linux

La clé exFAT n’autorise pas l’exécution. Copier `Linux/NomadVault` sur le bureau. Clic droit, **Propriétés**, onglet **Permissions**, cocher **Autoriser l’exécution du fichier comme un programme**, puis double-cliquer.

Si rien ne s’ouvre, dans un terminal ouvert sur le bureau (`Bureau` ou `Desktop`) :

```bash
chmod +x NomadVault
./NomadVault --appimage-extract-and-run
```

### Windows

Copier `Windows/NomadVault.exe` sur le bureau, puis double-cliquer. Windows peut afficher « Windows a protégé votre ordinateur », parce que le fichier n’est pas signé chez Microsoft : **Informations complémentaires**, puis **Exécuter quand même**. Si l’onglet Général des propriétés propose **Débloquer**, cocher cette case.

Si une fenêtre demande Microsoft Edge WebView2, l’installer. Windows 10 et 11 à jour l’ont en général déjà. Cette version Windows n’a pas encore été ouverte sur un PC Windows.

## Ce que voit une personne qui branche la clé

```text
NomadVault/
  Mac/NomadVault.app           Mac Apple Silicon
  Mac/NomadVault Intel.app     Mac Intel
  Windows/NomadVault.exe
  Linux/NomadVault
  Vault/
    vault.meta
    index.enc
    objects/
      1a7c832f….dat
  README.txt
```

`vault.meta` contient les paramètres publics de dérivation et la clé maître sous forme chiffrée. Il ne contient ni le mot de passe, ni les noms de fichiers. Le détail du format est dans [FORMAT.md](FORMAT.md). Les limites du chiffrement sont dans [SECURITY.md](SECURITY.md).

## Téléchargement

Les trois versions et le mode d’emploi sont réunis dans `NomadVault.zip`, sur la page des publications du dépôt GitHub. Copiez le dossier obtenu sur la clé, ou gardez-le sur l’ordinateur.

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
