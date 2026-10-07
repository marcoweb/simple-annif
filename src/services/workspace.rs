use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Workspace {
    root: PathBuf,
}

impl Workspace {
    pub fn prepare(root: &Path) -> Result<Self> {
        anyhow::ensure!(
            root.is_absolute(),
            "O caminho do workspace deve ser absoluto"
        );
        for directory in [
            "projects",
            "vocabularies",
            "datasets",
            "models",
            "results",
            "logs",
        ] {
            let path = root.join(directory);
            std::fs::create_dir_all(&path).with_context(|| {
                format!(
                    "Não foi possível preparar o workspace em {}. Verifique as permissões",
                    path.display()
                )
            })?;
        }
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}
