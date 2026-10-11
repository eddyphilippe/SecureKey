//! Primitives cryptographiques de SecureZip.
//!
//! Ce module ne dépend d'aucune interface graphique. Il n'invente aucun
//! algorithme : Argon2id, XChaCha20-Poly1305 et la construction STREAM
//! proviennent des crates RustCrypto.

mod aead;
mod kdf;
mod keys;
mod recovery;
mod stream;

pub use aead::{unwrap_master_key, wrap_master_key, WrappedKey, WRAP_LABEL};
pub use kdf::{derive_kek, KdfParams, RECOMMENDED_ITERATIONS, RECOMMENDED_MEMORY_KIB, RECOMMENDED_PARALLELISM};
pub use keys::{random_bytes, MasterKey, Kek, KEY_LEN, SALT_LEN};
pub use recovery::create_recovery_key;
pub use stream::{
    index_aad, object_aad, open_from_file, seal_to_writer, Header, NopProgress, Progress,
    DEFAULT_CHUNK_SIZE, HEADER_LEN, INDEX_CHUNK_SIZE, INDEX_LABEL, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE,
    OBJECT_LABEL, TAG_LEN,
};
