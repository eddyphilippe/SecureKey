# Format Vault V1

Ce document suffit à écrire un autre logiciel capable de lire et d’écrire un coffre SecureZip version 1. Les entiers sont little-endian. Le texte est de l’UTF-8 sans BOM. Les champs binaires des fichiers JSON sont en hexadécimal minuscule. Aucun chemin, permission, ACL, attribut étendu ou flux alternatif n’est utilisé : le coffre est un ensemble de fichiers ordinaires, adapté à exFAT.

`format_version = 1`. Une version inconnue doit être refusée sans modifier le coffre. Une future version 2 devra savoir lire la version 1. Ajouter des champs à un JSON de version 1 n’est pas une stratégie de migration : les structures rejettent les champs inconnus, pour qu’une ancienne application ne les efface pas en réécrivant le fichier.

## Arborescence

```text
Vault/
  vault.meta          en-tête public
  index.enc           catalogue chiffré
  index.enc.bak       catalogue précédent, après la première modification
  objects/
    <32 hex>.dat      un fichier chiffré
```

Les fichiers dont le nom commence par `.` sont des temporaires d’écriture. Ils ne font pas partie du coffre. Un lecteur doit les ignorer.

## vault.meta

JSON UTF-8, joliment imprimé par `serde_json`, terminé par un saut de ligne `\n`.

```json
{
  "format_version": 1,
  "vault_id": "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx",
  "created_at_ms": 0,
  "kdf": {
    "algorithm": "argon2id",
    "version": "0x13",
    "memory_kib": 65536,
    "iterations": 3,
    "parallelism": 4,
    "salt_hex": "<32 caractères hexadécimaux>",
    "output_len": 32
  },
  "master_key": {
    "algorithm": "xchacha20poly1305",
    "nonce_hex": "<48 caractères hexadécimaux>",
    "ciphertext_hex": "<96 caractères hexadécimaux>"
  }
}
```

- `vault_id` : UUID canonique, 16 octets.
- `created_at_ms` : millisecondes Unix, UTC.
- `salt_hex` : 16 octets.
- `nonce_hex` : 24 octets, nonce XChaCha20 complet, tiré pour cette enveloppe uniquement.
- `ciphertext_hex` : 32 octets de clé maître + 16 octets d’étiquette Poly1305.

La clé maître est une suite de 32 octets tirée par le générateur du système à la création du coffre. Elle n’est pas dérivée du mot de passe.

### Enveloppe de la clé maître

- Clé : KEK de 32 octets.
- KEK = Argon2id(mot de passe, sel, paramètres du fichier), sortie 32 octets.
- AEAD : XChaCha20-Poly1305.
- Nonce : les 24 octets de `nonce_hex`.
- Message : la clé maître.
- AAD : `securekey.v1.wrap` (17 octets ASCII) ‖ `vault_id` (16 octets) ‖ `format_version` en `u16` little-endian.

Un échec d’authentification signifie un mauvais mot de passe ou un en-tête modifié. Les deux cas sont indistinguables, volontairement.

Changer le mot de passe réécrit seulement ce fichier : nouveau sel, nouveau nonce, même clé maître, mêmes fichiers.

## Objet chiffré (`SKO1`)

Utilisé pour chaque fichier de `objects/` et pour `index.enc` / `index.enc.bak`.

| Offset | Taille | Champ |
| --- | --- | --- |
| 0 | 4 | magie ASCII `SKO1` |
| 4 | 2 | `u16` = 1 |
| 6 | 4 | `u32`, taille de bloc en clair |
| 10 | 1 | `u8` = 19 |
| 11 | 19 | préfixe de nonce STREAM |
| 30 | … | blocs chiffrés |

La taille de bloc écrite par cette application est 1 048 576 octets pour un fichier, et 65 536 octets pour l’index. Un lecteur doit accepter toute taille entre 1 024 et 16 777 216 octets, lue dans l’en-tête. Au-delà, l’objet est refusé.

### Blocs STREAM BE32

Implémentation de référence : crate `aead-stream` 0.6, type `EncryptorBE32<XChaCha20Poly1305>`.

Le nonce de 24 octets présenté à XChaCha20-Poly1305 est :

```text
préfixe de 19 octets ‖ compteur u32 big-endian ‖ dernier_bloc u8
```

Le compteur commence à 0. Il augmente de 1 après chaque bloc qui n’est pas le dernier. `dernier_bloc` vaut 0 pour un bloc intermédiaire et 1 pour le dernier. Le dernier bloc peut être plus court. Un fichier vide a quand même un dernier bloc : 16 octets d’étiquette, sans clair.

Chaque bloc chiffré fait `clair + 16` octets. L’étiquette Poly1305 est en suffixe, comme le prévoit XChaCha20-Poly1305.

Pour lire : tant qu’il reste strictement plus de `taille_de_bloc + 16` octets, le bloc suivant est intermédiaire et fait exactement cette taille. Le reste est le dernier bloc. Cette règle détecte une troncature, parce que le drapeau « dernier bloc » change le nonce et fait échouer l’authentification.

AAD identique pour tous les blocs d’un même objet :

- index : `securekey.v1.index` ‖ `vault_id` (16 octets) ;
- fichier : `securekey.v1.object` ‖ `vault_id` (16 octets) ‖ identifiant d’objet (16 octets).

L’identifiant d’objet est le nom de fichier sans `.dat`, décodé depuis 32 hexadécimaux minuscules. Le renommage visible par l’utilisateur ne change pas ce nom.

## index.enc

Le clair est un JSON UTF-8, joliment imprimé, terminé par `\n`, puis chiffré comme un objet `SKO1`.

```json
{
  "v": 1,
  "name": "Mon coffre",
  "auto_lock_secs": 300,
  "entries": [
    {
      "id": "uuid",
      "parent": null,
      "kind": "file",
      "name": "passeport.pdf",
      "object": "0123456789abcdef0123456789abcdef.dat",
      "size": 1234,
      "created_ms": 0,
      "modified_ms": 0,
      "favorite": false,
      "trashed": false,
      "comment": ""
    }
  ]
}
```

- `kind` vaut `file` ou `folder`.
- Un dossier a `object: null` et `size: 0`.
- `parent` est l’`id` d’un dossier, ou `null` à la racine.
- `name` est un seul composant : 1 à 255 caractères Unicode, sans `/ \ : * ? " < > |`, sans caractère de contrôle, sans espace en tête ou en fin, sans point final, et sans nom réservé Windows (`CON`, `PRN`, `AUX`, `NUL`, `COM1`…`COM9`, `LPT1`…`LPT9`, y compris avec une extension).
- `auto_lock_secs` vaut 0 (jamais), 60, 300, 900 ou 1800 dans l’interface. Un autre entier positif reste lisible.
- Les dates sont des millisecondes Unix.

Le nom du coffre et les noms de fichiers n’apparaissent que dans ce JSON, donc seulement après déchiffrement.

## Écriture

Un remplacement se fait ainsi :

1. écrire un frère `.nom.tmp-<aléa>` ;
2. `fsync` du fichier ;
3. renommage par-dessus la cible (sur Windows, `MoveFileExW` avec `MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH`) ;
4. `fsync` du dossier, au mieux, sans échouer si le système de fichiers le refuse.

Pour l’index : écrire le nouveau contenu, copier l’index actuel vers `index.enc.bak` par le même procédé, puis seulement remplacer `index.enc`. Une coupure avant ce dernier renommage laisse l’ancien index en place.

L’index est mis à jour après l’apparition du fichier `.dat`. L’inverse produirait une entrée pointant vers un fichier absent. Un `.dat` sans entrée est un orphelin : il est signalé, pas effacé.

## Ce qui est hors format

- Pas de clé de récupération dans la V1. Le champ n’existe pas. Il ne devra jamais être écrit automatiquement sur la même clé que le coffre.
- Pas de cloud.
- Pas de dépendance au système pour relire un coffre.
