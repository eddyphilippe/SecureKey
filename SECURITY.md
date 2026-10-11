# Sécurité

NomadVault chiffre des fichiers et leurs métadonnées. Ce document décrit ce que cela protège, ce que cela ne protège pas, et comment les clés sont manipulées.

## Modèle de menace

**Protégé.** Une personne qui obtient la clé USB, ou une copie du dossier `Vault`, sans le mot de passe, ne doit pas pouvoir :

- lire le contenu des fichiers ;
- lire les noms, les extensions, l’arborescence ou les commentaires ;
- modifier un fichier ou l’index sans que l’ouverture ou la vérification échoue ;
- retrouver le mot de passe ou la clé maître en clair sur la clé.

**Non protégé.**

- Un ordinateur compromis, un keylogger ou un malware pendant que le mot de passe est saisi ou pendant que le coffre est déverrouillé.
- Une personne qui regarde l’écran, ou qui lit les fichiers déjà exportés ou ouverts dans une application.
- La suppression garantie d’une copie temporaire sur un SSD : l’effacement par écrasement est une précaution, pas une garantie, à cause de l’usure répartie et de la sur-allocation.
- Une sauvegarde ou un export que l’utilisateur place lui-même à un endroit non chiffré.
- La disponibilité de la clé : le chiffrement ne répare pas une clé physiquement détruite. Il n’existe pas, dans cette version, de clé de récupération.

NomadVault ne prétend pas offrir une sécurité absolue.

Le chiffrement protège les données lorsque le coffre est verrouillé. Il ne protège pas contre un ordinateur compromis, un keylogger ou un malware lorsque vous saisissez votre mot de passe.

## Algorithmes

Tous proviennent de RustCrypto, sans primitive locale.

| Rôle | Algorithme | Crate |
| --- | --- | --- |
| Dérivation du mot de passe | Argon2id, version 0x13 | `argon2` 0.6.0 |
| Chiffrement authentifié | XChaCha20-Poly1305 | `chacha20poly1305` 0.11.0 |
| Gros fichiers | STREAM BE32 | `aead` 0.6.1, `aead-stream` 0.6.0 |
| Aléa | Générateur du système | `getrandom` 0.4.3 |
| Comparaison en temps constant | Égalité de clés | `subtle` 2.6.1 |
| Effacement mémoire | Zeroize à la destruction | `zeroize` 1.9.1 |

Paramètres Argon2id d’un coffre créé par l’application, soit la seconde recommandation du [RFC 9106](https://www.rfc-editor.org/rfc/rfc9106) :

- mémoire : 64 Mio (`memory_kib = 65536`) ;
- passes : 3 ;
- parallélisme : 4 ;
- sel : 16 octets ;
- sortie : 32 octets.

Ces paramètres sont stockés dans `vault.meta`. Un coffre plus ancien reste ouvrable avec les paramètres qui y sont écrits. Cette implémentation refuse toutefois des paramètres abusifs (plus de 256 Mio, plus de 8 passes ou un parallélisme supérieur à 8) pour ne pas bloquer la machine sur un en-tête hostile.

Le mode parallèle d’Argon2 (`rayon`) accélère le calcul. Il ne change pas le résultat.

STREAM suit la construction de Rogaway et al., [ePrint 2015/189](https://eprint.iacr.org/2015/189), telle qu’implémentée par `aead-stream` en variante BE32 : compteur 32 bits big-endian et un octet « dernier bloc » dans les 5 derniers octets du nonce XChaCha20. Le préfixe aléatoire de 19 octets est stocké dans l’en-tête de l’objet. Chaque bloc a donc un nonce distinct. Un nouveau préfixe est tiré pour chaque fichier et pour chaque réécriture de l’index.

## Clés

```text
mot de passe + sel
        │
        ▼
     Argon2id
        │
        ▼
       KEK  (32 octets, uniquement en mémoire)
        │
        ▼
 déchiffrement de la clé maître
        │
        ▼
   Master Key  (32 octets aléatoires, créée à l’initialisation)
        │
        ▼
 chiffrement de l’index et des fichiers
```

- Le mot de passe n’est pas enregistré.
- La clé maître n’est enregistrée que chiffrée, dans `vault.meta`.
- Changer le mot de passe régénère le sel et un nonce, puis ré-enveloppe la même clé maître. Les fichiers ne sont pas rechiffrés.
- L’interface ne reçoit jamais la clé maître. Elle reçoit les noms seulement après déverrouillage, parce qu’ils doivent être affichés.
- Les secrets Rust (`MasterKey`, `Kek`) sont marqués `ZeroizeOnDrop`. Les tampons de chiffrement sont effacés après usage. Des copies peuvent néanmoins survivre dans l’allocateur, dans Argon2, dans l’état interne du chiffrement, ou dans le canal Tauri qui a transporté le mot de passe. Ce n’est pas un effacement prouvable de toute la mémoire du processus.

Données associées (AAD), pour lier un chiffré à son rôle :

- enveloppe de la clé maître : `securekey.v1.wrap` ‖ identifiant du coffre ‖ version du format ;
- index : `securekey.v1.index` ‖ identifiant du coffre ;
- fichier : `securekey.v1.object` ‖ identifiant du coffre ‖ identifiant d’objet.

Un objet déplacé sous un autre nom, ou copié dans un autre coffre, échoue à l’authentification.

## Fichiers et intégrité

Chaque fichier est chiffré à part, par blocs de 1 Mio, sans être chargé entièrement en mémoire. L’annulation ou une coupure pendant l’écriture laisse un fichier temporaire (`.….tmp-…`) qui n’est pas un objet valide. L’index n’est mis à jour qu’après la synchronisation et le renommage du fichier chiffré.

L’index précédent est conservé dans `index.enc.bak`. NomadVault ne restaure pas automatiquement un coffre endommagé : **Vérifier le coffre** produit un rapport.

Le mode lecture seule refuse toute écriture du coffre, y compris le changement de mot de passe et le verrouillage automatique s’il devait réécrire l’index.

Si la clé qui contient l’application ou le coffre est retirée, NomadVault se ferme. Il ne tente pas de relire ce volume : cet accès peut bloquer l’ordinateur. Les clés en mémoire sont effacées si le verrou n’est pas déjà pris par une copie en cours.

## Journaux

Les journaux techniques ne contiennent que des codes fixes (`vault.open.ok`, `object.write`, …). Ils ne contiennent pas le mot de passe, les clés, les noms réels ni le contenu. Ils sont écrits sur la sortie d’erreur du processus, pas dans le coffre.

## Clé de récupération

La fonction existe dans l’architecture (`crypto/recovery.rs`) et retourne une erreur. Elle n’est pas activée. Une future clé de récupération ne devra jamais être stockée automatiquement sur la même clé USB que le coffre, et devra être auditée avant d’être proposée.

## Pile autorisée

Les versions de cette pile sont exactes dans `src-tauri/Cargo.toml` (`=`) et figées par `src-tauri/Cargo.lock`. Ne pas lancer `cargo update` sur ces crates sans lire l’avis RustSec correspondant.

Autorisées, et rien d’autre sans justification écrite ici :

- `argon2`, Argon2id uniquement ;
- `chacha20poly1305`, XChaCha20-Poly1305 uniquement ;
- `aead` et `aead-stream`, uniquement pour ce même algorithme en flux ;
- `zeroize`, `getrandom`, `subtle`.

Blake2 est une dépendance interne d’Argon2id. Le coffre ne l’appelle pas. SHA-2 peut apparaître dans l’arbre Tauri. Le coffre ne l’utilise pas pour chiffrer.

AES, RSA, les courbes elliptiques, OpenSSL, ring, et toute primitive écrite dans ce dépôt sont refusés.

## Avis et audit

Avant une version destinée à des fichiers réels :

```bash
cargo audit --file src-tauri/Cargo.lock
```

Le même contrôle est lancé par l’intégration continue. RustCrypto publie ses problèmes dans la base RustSec ; un avis concerne cette pile tant qu’il n’est pas traité.

Contrôle du 11 octobre 2026, base RustSec de 1 296 avis : aucune vulnérabilité sur la pile du coffre. Deux avertissements concernent des dépendances de l’interface, pas le chiffrement : `proc-macro-error` 1.0.4 (RUSTSEC-2024-0370, plus maintenu) et `glib` 0.18.5 (RUSTSEC-2024-0429). Ils viennent de Tauri.

## Bibliothèques examinées

Une bibliothèque de chiffrement n’est ajoutée que si un rapport public est indiqué ici.

| Bibliothèque | Examen public |
| --- | --- |
| `chacha20poly1305`, `chacha20`, `poly1305` | NCC Group pour MobileCoin, décembre 2019, [rapport du 12 février 2020](https://research.nccgroup.com/2020/02/26/public-report-rustcrypto-aes-gcm-and-chacha20poly1305-implementation-review/). Aucune vulnérabilité. La version figée ici, `chacha20poly1305` 0.11.0, est postérieure à ce rapport. |
| `aead-stream` | Construction STREAM publiée par Rogaway et al. (ePrint 2015/189), appliquée au chiffrement ci-dessus. |
| `argon2` | Argon2id, algorithme retenu par la Password Hashing Competition en juillet 2015. Mise en œuvre RustCrypto 0.6.0. |

## Machine inconnue

Préférez la lecture seule. Déverrouiller un coffre sur une machine non fiable expose le mot de passe et, ensuite, les fichiers ouverts. Le chiffrement au repos ne corrige pas cela.
