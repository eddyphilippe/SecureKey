use std::fs::File;

use crate::crypto::{object_aad, open_from_file, NopProgress};
use crate::error::Result;
use crate::metadata::EntryKind;
use crate::platform::diag;
use crate::storage::{list_object_dir, parse_object_name};

use super::index::read_catalog;
use super::VaultSession;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyIssue {
    pub severity: String,
    pub code: String,
    pub detail: String,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    pub ok: bool,
    pub checked: u64,
    pub issues: Vec<VerifyIssue>,
}

impl VaultSession {
    /// Contrôle le coffre sans jamais l'écrire.
    pub fn verify(&self) -> Result<VerifyReport> {
        diag("vault.verify");
        let mut issues = Vec::new();
        let mut referenced = Vec::new();
        let mut checked = 0u64;

        for entry in &self.catalog.entries {
            if entry.kind != EntryKind::File {
                continue;
            }
            let Some(object) = entry.object.as_deref() else {
                issues.push(issue("error", "index.incoherent", &entry.id));
                continue;
            };
            let Some(object_id) = parse_object_name(object) else {
                issues.push(issue("error", "index.incoherent", &entry.id));
                continue;
            };
            referenced.push(object.to_string());
            let path = self.paths.object(object);
            if !path.is_file() {
                issues.push(issue("error", "object.missing", object));
                continue;
            }
            checked += 1;
            let mut file = File::open(&path)?;
            let mut sink = Discard;
            match open_from_file(
                &self.master,
                &object_aad(&self.vault_id, &object_id),
                &mut file,
                &mut sink,
                &mut NopProgress,
            ) {
                Ok(_) => {}
                Err(crate::error::Error::AuthFailed)
                | Err(crate::error::Error::Damaged)
                | Err(crate::error::Error::UnsupportedFormat { .. }) => {
                    issues.push(issue("error", "object.corrupt", object));
                }
                Err(error) => return Err(error),
            }
        }

        if let Ok(names) = list_object_dir(&self.paths.objects_dir()) {
            for name in names {
                if name.starts_with('.') {
                    issues.push(issue("warning", "object.temporary", &name));
                    continue;
                }
                if parse_object_name(&name).is_none() {
                    issues.push(issue("warning", "object.unexpected", &name));
                    continue;
                }
                if !referenced.iter().any(|candidate| candidate == &name) {
                    issues.push(issue("warning", "object.orphan", &name));
                }
            }
        } else if self.catalog.entries.iter().any(|entry| entry.kind == EntryKind::File) {
            issues.push(issue("error", "objects.missing", "objects"));
        }

        if self.paths.index_backup().is_file() {
            match read_catalog(&self.master, &self.vault_id, &self.paths.index_backup()) {
                Ok(_) => {}
                Err(_) => issues.push(issue("warning", "index.backup", "index.enc.bak")),
            }
        }

        let ok = !issues.iter().any(|item| item.severity == "error");
        Ok(VerifyReport {
            ok,
            checked,
            issues,
        })
    }
}

fn issue(severity: &str, code: &str, detail: &str) -> VerifyIssue {
    VerifyIssue {
        severity: severity.to_string(),
        code: code.to_string(),
        detail: detail.to_string(),
    }
}

struct Discard;

impl std::io::Write for Discard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
