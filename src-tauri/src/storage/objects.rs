use std::fs;
use std::path::Path;

use crate::error::{Error, Result};

/// Nom d'objet opaque : 32 hexadécimaux minuscules et l'extension `.dat`.
pub fn parse_object_name(name: &str) -> Option<[u8; 16]> {
    let hex_part = name.strip_suffix(".dat")?;
    if hex_part.len() != 32 {
        return None;
    }
    if !hex_part
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return None;
    }
    let bytes = hex::decode(hex_part).ok()?;
    bytes.try_into().ok()
}

pub fn object_name(id: &[u8; 16]) -> String {
    format!("{}.dat", hex::encode(id))
}

pub fn list_object_dir(dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            return Err(Error::Damaged);
        };
        if entry.file_type()?.is_file() {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}
