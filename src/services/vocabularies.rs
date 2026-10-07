use crate::annif::{AnnifRuntime, ProcessResult};
use anyhow::{Context, Result};
use std::{path::Path, thread::JoinHandle};

pub fn start(
    runtime: &impl AnnifRuntime,
    id: &str,
    path: &str,
    language: &str,
    logs: crossbeam_channel::Sender<String>,
) -> Result<JoinHandle<Result<ProcessResult>>> {
    anyhow::ensure!(
        !id.trim().is_empty(),
        "Informe o ID do vocabulário definido no projeto"
    );
    anyhow::ensure!(
        !language.trim().is_empty(),
        "Informe o idioma do vocabulário (ex.: pt)"
    );
    let path = Path::new(path.trim());
    anyhow::ensure!(path.is_file(), "Arquivo de vocabulário não encontrado");
    let path = path
        .canonicalize()
        .context("Não foi possível acessar o vocabulário")?;
    validate_source(&path)?;
    runtime.load_vocab(id.trim(), &path.to_string_lossy(), language.trim(), logs)
}
use std::io::Read;

fn validate_source(path: &Path) -> Result<()> {
    let mut prefix = [0u8; 1024];
    let count = std::fs::File::open(path)?.read(&mut prefix)?;
    anyhow::ensure!(count > 0, "O arquivo de vocabulário está vazio");
    let text = String::from_utf8_lossy(&prefix[..count]).to_lowercase();
    anyhow::ensure!(
        !text.contains("<!doctype html") && !text.contains("<html"),
        "O arquivo contém uma página HTML, não um vocabulário. No GitHub, baixe o arquivo pelo botão Raw/Download raw file; não salve a página do navegador como RDF."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_html_saved_with_rdf_extension() -> Result<()> {
        let dir = crate::test_support::TestDirectory::new()?;
        let path = dir.path().join("nalt.rdf");
        std::fs::write(&path, "<!DOCTYPE html><html>GitHub</html>")?;
        assert!(
            validate_source(&path)
                .unwrap_err()
                .to_string()
                .contains("HTML")
        );
        std::fs::write(&path, "<?xml version=\"1.0\"?><rdf:RDF/>")?;
        validate_source(&path)?;
        Ok(())
    }
}
