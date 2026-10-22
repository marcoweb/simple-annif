use crate::annif::{AnnifRuntime, ProcessResult};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
pub struct Suggestion {
    pub uri: String,
    pub label: String,
    pub notation: Option<String>,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DocumentResult {
    pub path: String,
    pub project_id: String,
    pub suggestions: Vec<Suggestion>,
    pub error: Option<String>,
    pub cancelled: bool,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u128,
}

pub fn parse_suggestions(text: &str) -> Result<Vec<Suggestion>> {
    text.lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with("Suggestions for "))
        .map(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            anyhow::ensure!(
                parts.len() == 3 || parts.len() == 4,
                "Sugestão em formato inesperado: {line}"
            );
            let uri = parts[0]
                .strip_prefix('<')
                .and_then(|s| s.strip_suffix('>'))
                .context("URI da sugestão inválida")?;
            let score: f64 = parts[parts.len() - 1].parse().context("Score inválido")?;
            anyhow::ensure!(score.is_finite(), "Score não finito");
            Ok(Suggestion {
                uri: uri.into(),
                label: parts[1].into(),
                notation: (parts.len() == 4).then(|| parts[2].into()),
                score,
            })
        })
        .collect()
}

// Diretórios são expandidos em TXT, sem seguir links ou percorrer subdiretórios.
pub fn input_files(paths: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for path in paths {
        let path = Path::new(path);
        if path.is_dir() {
            for entry in std::fs::read_dir(path)
                .with_context(|| format!("Não foi possível ler {}", path.display()))?
            {
                let entry = entry?;
                if entry.file_type()?.is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("txt"))
                {
                    files.push(entry.path().canonicalize()?);
                }
            }
        } else {
            anyhow::ensure!(path.is_file(), "Arquivo não encontrado: {}", path.display());
            files.push(path.canonicalize()?);
        }
    }
    files.sort();
    files.dedup();
    anyhow::ensure!(
        !files.is_empty(),
        "Nenhum arquivo TXT encontrado na seleção"
    );
    Ok(files)
}

pub fn suggest(runtime: &impl AnnifRuntime, project_id: &str, path: &Path) -> DocumentResult {
    let mut result = DocumentResult {
        path: path.display().to_string(),
        project_id: project_id.into(),
        suggestions: Vec::new(),
        error: None,
        cancelled: false,
        stdout: String::new(),
        stderr: String::new(),
        duration_ms: 0,
    };
    let operation = (|| -> Result<()> {
        std::fs::read_to_string(path).context("O documento deve ser um arquivo de texto UTF-8")?;
        let ProcessResult {
            stdout,
            stderr,
            duration,
            exit_code,
            cancelled,
            ..
        } = runtime.suggest(project_id, &path.to_string_lossy())?;
        result.stdout = stdout;
        result.stderr = stderr;
        result.duration_ms = duration.as_millis();
        result.cancelled = cancelled;
        anyhow::ensure!(
            !cancelled,
            "Suggest cancelado pelo usuário; resultado incompleto descartado"
        );
        anyhow::ensure!(
            exit_code == Some(0),
            "Suggest falhou (código {exit_code:?}): {}",
            result.stderr
        );
        result.suggestions = parse_suggestions(&result.stdout)?;
        Ok(())
    })();
    if let Err(error) = operation {
        result.error = Some(format!("{error:#}"));
    }
    result
}

pub fn export(path: &Path, results: &[DocumentResult]) -> Result<()> {
    let json = serde_json::to_string_pretty(results)?;
    std::fs::write(path, json)
        .with_context(|| format!("Não foi possível salvar {}", path.display()))
}

pub fn suggest_batch(
    runtime: &impl AnnifRuntime,
    project_id: &str,
    files: &[PathBuf],
    cancellation: &crate::annif::Cancellation,
    mut publish: impl FnMut(DocumentResult) -> bool,
) {
    for path in files {
        let result = if cancellation.is_requested() {
            DocumentResult {
                path: path.display().to_string(),
                project_id: project_id.into(),
                suggestions: Vec::new(),
                error: Some("Não processado: lote cancelado pelo usuário".into()),
                cancelled: true,
                stdout: String::new(),
                stderr: String::new(),
                duration_ms: 0,
            }
        } else {
            suggest(runtime, project_id, path)
        };
        if !publish(result) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_batch_keeps_prior_results_and_records_every_skipped_document() -> Result<()> {
        use crate::{annif::Cancellation, services::runtime::ApplicationRuntime};
        let runtime =
            ApplicationRuntime::Unavailable("runtime não deve ser chamado após cancelar".into());
        let cancellation = Cancellation::default();
        let files = vec![
            PathBuf::from("one.txt"),
            PathBuf::from("two.txt"),
            PathBuf::from("three.txt"),
        ];
        let mut results = Vec::new();
        suggest_batch(&runtime, "teste", &files, &cancellation, |result| {
            results.push(result);
            cancellation.cancel();
            true
        });
        assert_eq!(results.len(), 3);
        assert!(!results[0].cancelled); // earlier failure remains a failure
        assert!(results[0].error.is_some());
        assert!(results[1..].iter().all(|result| result.cancelled
            && result.error.as_ref().unwrap().starts_with("Não processado")));
        let dir = crate::test_support::TestDirectory::new()?;
        let path = dir.path().join("cancelled.json");
        export(&path, &results)?;
        let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(json[1]["cancelled"], true);
        assert!(crate::services::evametrics::render(&results).is_err());
        Ok(())
    }
    #[test]
    fn parses_unicode_scores_and_optional_notation() -> Result<()> {
        let rows = parse_suggestions(
            "Suggestions for C:\\texto com espaços.txt\n<https://ex/1>\tComputação\t0.8750\n<https://ex/2>\tÁrvores\tA2\t0.2500\n",
        )?;
        assert_eq!(rows[0].label, "Computação");
        assert_eq!(rows[0].score, 0.875);
        assert_eq!(rows[1].notation.as_deref(), Some("A2"));
        assert!(parse_suggestions("<x>\tx\tNaN").is_err());
        assert!(parse_suggestions("unexpected output").is_err());
        assert!(parse_suggestions("Suggestions for empty.txt\n")?.is_empty());
        Ok(())
    }
    #[test]
    fn expands_txt_and_deduplicates_without_including_keys() -> Result<()> {
        let dir = crate::test_support::TestDirectory::new()?;
        let file = dir.path().join("ação.TXT");
        std::fs::write(&file, "texto")?;
        std::fs::write(dir.path().join("ação.key"), "uri")?;
        let files = input_files(&[dir.path().display().to_string(), file.display().to_string()])?;
        assert_eq!(files, vec![file.canonicalize()?]);
        assert!(export(&dir.path().join("ausente/out.json"), &[]).is_err());
        Ok(())
    }
}
