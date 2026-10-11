use std::io;

/// Erreur du moteur SecureZip.
///
/// Les messages sont destinés à l'interface. Ils ne contiennent ni mot de
/// passe, ni clé, ni contenu de fichier.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Mot de passe incorrect, ou en-tête du coffre endommagé.")]
    AuthFailed,

    #[error("Le coffre est endommagé ou a été modifié.")]
    Damaged,

    #[error("Opération annulée.")]
    Cancelled,

    #[error("Le coffre est ouvert en lecture seule.")]
    ReadOnly,

    #[error("Ce format de coffre n'est pas pris en charge (version {found}).")]
    UnsupportedFormat { found: u16 },

    #[error("Un élément porte déjà ce nom.")]
    NameConflict,

    #[error("Ce nom ne peut pas être utilisé.")]
    InvalidName,

    #[error("Élément introuvable.")]
    NotFound,

    #[error("Le mot de passe doit contenir au moins 8 caractères.")]
    PasswordTooShort,

    #[error("Les deux mots de passe ne correspondent pas.")]
    PasswordMismatch,

    #[error("Un coffre existe déjà à cet emplacement.")]
    AlreadyExists,

    #[error("Ce dossier n'est pas un coffre SecureZip.")]
    NotAVault,

    #[error("La clé de récupération n'est pas disponible dans cette version.")]
    RecoveryUnavailable,

    #[error("Une opération est déjà en cours.")]
    Busy,

    #[error("Le coffre est verrouillé.")]
    Locked,

    #[error("Le délai de verrouillage choisi n'est pas disponible.")]
    InvalidAutoLock,

    #[error("Impossible d'accéder aux fichiers. La clé est peut-être débranchée.")]
    Io(#[source] io::Error),

    #[error("Opération impossible.")]
    Data,
}

impl Error {
    /// Message affichable. Masque les chemins internes renvoyés par le système.
    pub fn user_message(&self) -> String {
        self.to_string()
    }
}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        if value.kind() == io::ErrorKind::UnexpectedEof {
            Error::Damaged
        } else {
            Error::Io(value)
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
