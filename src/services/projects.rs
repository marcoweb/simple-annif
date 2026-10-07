use anyhow::{Context, Result};
use std::path::Path;

pub const EXAMPLE: &str = r#"[meu-tfidf]
name = "Meu projeto TF-IDF"
backend = "tfidf"
language = "pt"
analyzer = "snowball(portuguese)"
vocab = "meu-vocab"

[meu-mllm]
name = "Meu projeto MLLM"
backend = "mllm"
language = "pt"
analyzer = "snowball(portuguese)"
vocab = "meu-vocab"

[meu-ensemble]
name = "Meu Ensemble"
backend = "ensemble"
language = "pt"
vocab = "meu-vocab"
sources = "meu-tfidf:1.0,meu-mllm:1.0"
"#;

pub fn save_for_runtime(
    runtime: &crate::services::runtime::ApplicationRuntime,
    path: &Path,
    text: &str,
) -> Result<()> {
    if let crate::services::runtime::ApplicationRuntime::Embedded(embedded) = runtime {
        crate::services::workspace::Workspace::prepare(embedded.workspace())?;
        let projects = embedded.workspace().join("projects").canonicalize()?;
        let parent = path
            .parent()
            .context("Caminho de projeto inválido")?
            .canonicalize()?;
        anyhow::ensure!(
            parent == projects,
            "Salve o arquivo na pasta projects do workspace ativo: {}",
            projects.display()
        );
        if path.exists() {
            anyhow::ensure!(
                path.canonicalize()?.parent() == Some(projects.as_path()),
                "O arquivo não pode apontar para fora de projects"
            );
        }
    }
    save(path, text)
}

pub fn save(path: &Path, text: &str) -> Result<()> {
    anyhow::ensure!(
        path.extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml")),
        "Use a extensão .toml"
    );
    let config: toml::Table =
        toml::from_str(text).context("TOML inválido; o arquivo não foi alterado")?;
    anyhow::ensure!(!config.is_empty(), "Defina pelo menos um projeto");
    for (id, value) in config {
        anyhow::ensure!(
            !id.is_empty() && !id.chars().any(char::is_whitespace),
            "ID de projeto inválido: {id}"
        );
        let project = value
            .as_table()
            .context("Cada projeto deve ser uma seção TOML")?;
        for field in ["backend", "language", "vocab"] {
            anyhow::ensure!(
                project
                    .get(field)
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| !v.trim().is_empty()),
                "Projeto {id}: informe {field}"
            );
        }
    }
    std::fs::write(path, text)
        .with_context(|| format!("Não foi possível salvar {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_edit_preserves_existing_file() -> Result<()> {
        let dir = crate::test_support::TestDirectory::new()?;
        let path = dir.path().join("projetos.toml");
        save(&path, EXAMPLE)?;
        assert!(save(&path, "[quebrado").is_err());
        assert_eq!(std::fs::read_to_string(path)?, EXAMPLE);
        Ok(())
    }
}
