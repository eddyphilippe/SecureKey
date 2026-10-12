NomadVault
=========

NomadVault garde des fichiers dans un coffre chiffré. Le coffre peut être
sur une clé USB ou dans un dossier de l'ordinateur. Mac, Windows et Linux
ouvrent le même coffre, sans conversion.

Le dossier Vault livré ici est vide. Ce n'est pas encore un coffre :
le fichier vault.meta n'existe pas, et objects est vide.
Il faut créer le coffre une fois (voir plus bas).

  Mac/NomadVault.app           Mac Apple Silicon
  Mac/NomadVault Intel.app     Mac Intel
  Windows/NomadVault.exe       Windows
  Linux/NomadVault             Linux
  Vault/                       coffre, une fois créé
  README.txt


Compatibilité
-------------

Mac Apple Silicon (M1, M2, M3, M4 et suivants)
  Fichier : Mac/NomadVault.app
  Système : macOS 11 (Big Sur) ou plus récent
  Essai réalisé : l'application s'ouvre

Mac Intel
  Fichier : Mac/NomadVault Intel.app
  Système : macOS 11 (Big Sur) ou plus récent.
  Le dernier grand système pour la plupart des Mac Intel est macOS 26.
  Essai réalisé : Big Sur 11.7.11
  L'autre icône, NomadVault.app, est refusée par un Mac Intel
  (« application endommagée »).

Linux
  Fichier : Linux/NomadVault
  Machine : PC 64 bits (processeur Intel ou AMD)
  Système : une distribution récente, par exemple Linux Mint, Ubuntu,
  Debian ou Fedora
  Essai réalisé : Linux Mint, bureau Cinnamon 6.2.9
  Non prévu : Linux 32 bits, Raspberry Pi et autres machines ARM

Windows
  Fichier : Windows/NomadVault.exe
  Système visé : Windows 10 ou Windows 11, 64 bits, à jour
  Essai réalisé : aucun. Le fichier est compilé, pas encore ouvert
  sur un PC Windows.
  Non prévu : Windows 7, Windows 8, Windows 32 bits,
  Windows sur processeur ARM

Clé USB
  Format : exFAT, lisible sur Mac, Windows et Linux.
  Un fichier du coffre peut dépasser 4 Go.
  Le FAT16 ne convient pas : il s'arrête vers 2 Go.
  Le FAT32 bloque les fichiers de plus de 4 Go.


Lancer sur Mac
--------------

Double-cliquez l'icône qui correspond au Mac :
  NomadVault.app           Mac Apple Silicon
  NomadVault Intel.app     Mac Intel

Si macOS dit que l'application est endommagée, téléchargez à nouveau
NomadVault.zip depuis la page du projet, et ouvrez l'icône qui s'y trouve.
N'utilisez pas une copie plus ancienne.


Lancer sur Linux
----------------

La clé exFAT ne permet pas d'exécuter le programme. Copiez-le sur le bureau,
puis autorisez son lancement.

1. Ouvrez le dossier Linux de la clé.
2. Copiez le fichier NomadVault (sans extension) sur le bureau.
3. Clic droit sur cette copie, puis Propriétés.
4. Onglet Permissions : cochez
   « Autoriser l'exécution du fichier comme un programme ».
5. Double-cliquez l'icône.

Si le double-clic ne fait rien, ouvrez un terminal dans le bureau
(clic droit, Ouvrir dans un terminal) et lancez :

  chmod +x NomadVault
  ./NomadVault --appimage-extract-and-run

Le dossier du bureau s'appelle Bureau ou Desktop selon la langue du système.
S'il s'appelle Desktop :

  cd ~/Desktop
  chmod +x NomadVault
  ./NomadVault --appimage-extract-and-run


Lancer sur Windows
-------------------

1. Ouvrez le dossier Windows de la clé.
2. Copiez NomadVault.exe sur le bureau.
3. Double-cliquez cette copie.

Windows peut afficher « Windows a protégé votre ordinateur »,
parce que le fichier n'est pas signé chez Microsoft.
Choisissez Informations complémentaires, puis Exécuter quand même.

Si le fichier reste bloqué : clic droit sur NomadVault.exe, Propriétés.
En bas de l'onglet Général, cochez Débloquer s'il est proposé, puis OK.

Si une fenêtre demande d'installer Microsoft Edge WebView2, acceptez.
Windows 10 et 11 à jour l'ont en général déjà.


Créer ou ouvrir le coffre
-------------------------

Le programme copié sur le bureau ne voit pas la clé tout seul.

Créer le coffre
  1. Sur la clé, créez un dossier vide, par exemple MonCoffre.
  2. Dans NomadVault : Créer un nouveau coffre.
  3. Choisir l'emplacement, puis sélectionnez MonCoffre.
  4. Mot de passe d'au moins 8 caractères, confirmé, puis Créer le coffre.
  NomadVault ajoute un dossier Vault dans MonCoffre.
  Ce Vault contient vault.meta. C'est le coffre.

Ouvrir un coffre déjà créé
  1. Ouvrir un coffre existant.
  2. Allez sur la clé, puis dans le dossier qui contient vault.meta.
     Ce dossier s'appelle en général Vault.
  3. Sélectionnez ce dossier Vault, pas la clé entière,
     et pas le dossier objects.
  4. Saisissez le mot de passe, puis Déverrouiller.

Le dossier Vault vide livré avec le téléchargement n'a pas de vault.meta.
Si vous le sélectionnez, NomadVault propose de créer le coffre à cet endroit.
Choisissez le mot de passe, puis Créer le coffre.

Sur une machine inconnue, cochez Ouvrir en lecture seule.
Ce mode permet d'exporter une copie, sans rien écrire dans le coffre.

Verrouillez le coffre avant de retirer la clé.


Limite du chiffrement
---------------------

Le chiffrement protège les données lorsque le coffre est verrouillé.
Il ne protège pas contre un ordinateur compromis, un keylogger ou un malware
lorsque vous saisissez votre mot de passe.
