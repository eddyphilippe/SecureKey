use crate::error::{Error, Result};

/// Emplacement prévu pour une future clé de récupération.
///
/// La V1 ne la crée pas : une clé de récupération mal conçue permettrait de
/// contourner le mot de passe. Elle ne devra jamais être écrite sur la même
/// clé USB que le coffre.
pub fn create_recovery_key() -> Result<()> {
    Err(Error::RecoveryUnavailable)
}
