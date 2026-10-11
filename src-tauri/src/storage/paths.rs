use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct VaultPaths {
    root: PathBuf,
}

impl VaultPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn meta(&self) -> PathBuf {
        self.root.join("vault.meta")
    }

    pub fn index(&self) -> PathBuf {
        self.root.join("index.enc")
    }

    pub fn index_backup(&self) -> PathBuf {
        self.root.join("index.enc.bak")
    }

    pub fn objects_dir(&self) -> PathBuf {
        self.root.join("objects")
    }

    pub fn object(&self, name: &str) -> PathBuf {
        self.objects_dir().join(name)
    }
}
