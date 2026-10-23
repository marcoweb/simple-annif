//! Dataset contract from marcoweb/evametrics, dataset-load-control.tsx,
//! revision 9b2b9dc3d8d28bb53722ad9de63349d33598077b.
//! Its reader splits lines/semicolons/commas literally, without CSV escaping.
use super::processing::DocumentResult;
use anyhow::{Context, Result, ensure};
use std::{collections::HashSet, path::Path};

pub fn render(results: &[DocumentResult]) -> Result<String> {
    ensure!(!results.is_empty(), "Não há resultados para exportar.");
    let project = &results[0].project_id;
    let mut ids = HashSet::new();
    let mut output = String::from("ID;Titulo;Descriptores\r\n");
    for result in results {
        ensure!(
            result.project_id == *project,
            "Exporte apenas um projeto por arquivo Evametrics."
        );
        ensure!(
            !result.cancelled && result.error.is_none(),
            "O documento {} falhou na geração de sugestões. Corrija a falha e processe novamente, ou use JSON para preservar os erros.",
            result.path
        );
        ensure!(
            !result.suggestions.is_empty(),
            "O documento {} não tem sugestões. O importador atual do Evametrics não representa uma lista vazia corretamente. Use JSON para preservar esse resultado.",
            result.path
        );
        let filename = result
            .path
            .rsplit(['/', '\\'])
            .next()
            .context("Nome de arquivo ausente")?;
        let stem = Path::new(filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .context("Nome de arquivo inválido")?;
        ensure!(
            !stem.is_empty()
                && stem == stem.trim()
                && !stem.contains([';', ':', ',', '"', '\r', '\n']),
            "O nome do arquivo {filename} não pode ser usado como ID no Evametrics. Remova delimitadores ou espaços nas extremidades."
        );
        let id = if stem.starts_with('D') {
            stem.to_owned()
        } else {
            format!("D{stem}")
        };
        ensure!(
            ids.insert(id.clone()),
            "ID repetido no Evametrics: {id}. Use nomes de arquivo únicos, considerando o prefixo D."
        );
        let mut seen = HashSet::new();
        let mut labels = Vec::new();
        for suggestion in &result.suggestions {
            let label = suggestion.label.trim();
            ensure!(
                !label.is_empty() && !label.contains([',', ';', '"', '\r', '\n']),
                "O descritor {:?} em {filename} contém delimitadores ou está vazio. O importador atual do Evametrics não o preserva; use JSON para este resultado.",
                suggestion.label
            );
            if seen.insert(label.to_lowercase()) {
                labels.push(label);
            }
        }
        output.push_str(&format!("{id};{stem};\"{}\"\r\n", labels.join(", ")));
    }
    Ok(output)
}

pub fn export(path: &Path, results: &[DocumentResult]) -> Result<()> {
    let csv = render(results)?;
    std::fs::write(path, csv).with_context(|| format!("Não foi possível salvar {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::processing::Suggestion;

    fn document(path: &str, labels: &[&str]) -> DocumentResult {
        DocumentResult {
            path: path.into(),
            project_id: "teste".into(),
            error: None,
            cancelled: false,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            suggestions: labels
                .iter()
                .map(|label| Suggestion {
                    uri: "https://ex/1".into(),
                    label: (*label).into(),
                    notation: None,
                    score: 0.7,
                })
                .collect(),
        }
    }

    #[test]
    fn matches_evametrics_dataset_with_unicode_and_stable_ids() -> Result<()> {
        let data = [
            document(
                r"C:\textos\D8001.txt",
                &["Computação", "Árvores", "computação"],
            ),
            document("/textos/2.txt", &["soil"]),
        ];
        assert_eq!(
            render(&data)?,
            "ID;Titulo;Descriptores\r\nD8001;D8001;\"Computação, Árvores\"\r\nD2;2;\"soil\"\r\n"
        );
        Ok(())
    }

    #[test]
    fn rejects_lossy_or_ambiguous_exports_before_touching_destination() -> Result<()> {
        let dir = crate::test_support::TestDirectory::new()?;
        let path = dir.path().join("existente.csv");
        std::fs::write(&path, "preservar")?;
        let mut failed = document("D1.txt", &["soil"]);
        failed.error = Some("falhou".into());
        let mut other = document("D2.txt", &["soil"]);
        other.project_id = "outro".into();
        let cases = vec![
            vec![],
            vec![failed],
            vec![document("D1.txt", &[])],
            vec![document("D1.txt", &["soil, water"])],
            vec![document("D1.txt", &["a;b"])],
            vec![document("D1.txt", &["a\"b"])],
            vec![document("D1.txt", &["a\nb"])],
            vec![document("D1.txt", &[" "])],
            vec![document("D;1.txt", &["soil"])],
            vec![
                document("one/D1.txt", &["soil"]),
                document("two/D1.txt", &["water"]),
            ],
            vec![document("1.txt", &["soil"]), document("D1.txt", &["water"])],
            vec![document("D1.txt", &["soil"]), other],
        ];
        for data in cases {
            assert!(export(&path, &data).is_err());
            assert_eq!(std::fs::read_to_string(&path)?, "preservar");
        }
        assert!(
            export(
                &dir.path().join("missing/out.csv"),
                &[document("D1.txt", &["soil"])]
            )
            .is_err()
        );
        Ok(())
    }
}
