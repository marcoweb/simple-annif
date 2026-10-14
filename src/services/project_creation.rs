use super::{runtime::ApplicationRuntime, workspace::Workspace};
use anyhow::{Context, Result, ensure};
use std::{io::Write, path::PathBuf};

#[derive(Clone, Debug)]
pub struct ConfiguredProject {
    pub id: String,
    pub name: String,
    pub backend: String,
    pub language: String,
    pub vocab: String,
}

#[derive(Clone)]
pub struct Draft {
    pub name: String,
    pub id: String,
    pub language: String,
    pub vocab: String,
    pub backend: String,
    pub sources: Vec<String>,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            name: String::new(),
            id: String::new(),
            language: "pt".into(),
            vocab: String::new(),
            backend: "tfidf".into(),
            sources: Vec::new(),
        }
    }
}

pub fn suggested_id(name: &str) -> String {
    let mut result = String::new();
    for c in name.to_lowercase().chars() {
        let c = match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' => 'a',
            'é' | 'ê' | 'è' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ô' | 'õ' | 'ò' | 'ö' => 'o',
            'ú' | 'ü' | 'ù' | 'û' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            result.push(c);
        } else if !result.ends_with('-') && !result.is_empty() {
            result.push('-');
        }
    }
    result.trim_end_matches('-').chars().take(64).collect()
}

fn directory(runtime: &ApplicationRuntime) -> Result<PathBuf> {
    let ApplicationRuntime::Embedded(embedded) = runtime else {
        anyhow::bail!(
            "O formulário requer um workspace portátil. No modo de desenvolvimento, use o editor TOML."
        );
    };
    Workspace::prepare(embedded.workspace())?;
    Ok(embedded.workspace().join("projects"))
}

pub fn catalog(runtime: &ApplicationRuntime) -> Result<Vec<ConfiguredProject>> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(directory(runtime)?)? {
        let path = entry?.path();
        ensure!(
            !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("cfg")),
            "Este workspace contém configurações antigas .cfg. Converta-as para TOML antes de usar o formulário, ou continue com a configuração avançada."
        );
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("Não foi possível ler {}", path.display()))?;
        let config: toml::Table = toml::from_str(&text).with_context(|| {
            format!(
                "Corrija o TOML em {} antes de criar outro projeto",
                path.display()
            )
        })?;
        for (id, value) in config {
            let fields = value
                .as_table()
                .context("Cada projeto deve ser uma seção TOML")?;
            let field = |key: &str| {
                fields
                    .get(key)
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            result.push(ConfiguredProject {
                name: field("name"),
                backend: field("backend"),
                language: field("language"),
                vocab: field("vocab"),
                id,
            });
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

pub fn validate(draft: &Draft) -> Result<()> {
    ensure!(
        !draft.name.trim().is_empty(),
        "Informe um nome para o projeto."
    );
    ensure!(
        !draft.id.is_empty()
            && draft.id.len() <= 64
            && draft
                .id
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_'),
        "Use até 64 letras minúsculas, números, hífen ou sublinhado no identificador."
    );
    ensure!(
        ["pt", "en", "es"].contains(&draft.language.as_str()),
        "Selecione o idioma."
    );
    ensure!(
        !draft.vocab.trim().is_empty()
            && draft
                .vocab
                .trim()
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "Informe o ID do vocabulário usando letras, números, hífen ou sublinhado."
    );
    ensure!(
        ["tfidf", "mllm", "ensemble"].contains(&draft.backend.as_str()),
        "Selecione um método."
    );
    if draft.backend == "ensemble" {
        ensure!(
            draft.sources.len() >= 2,
            "Selecione pelo menos dois projetos para combinar."
        );
    }
    Ok(())
}

pub fn create(runtime: &ApplicationRuntime, draft: &Draft) -> Result<PathBuf> {
    validate(draft)?;
    let existing = catalog(runtime)?;
    ensure!(
        !existing
            .iter()
            .any(|p| p.id.eq_ignore_ascii_case(&draft.id)),
        "Já existe um projeto com esse identificador. Escolha outro."
    );
    let mut fields = toml::Table::new();
    for (key, value) in [
        ("name", draft.name.trim()),
        ("backend", &draft.backend),
        ("language", &draft.language),
        ("vocab", draft.vocab.trim()),
    ] {
        fields.insert(key.into(), toml::Value::String(value.into()));
    }
    if draft.backend == "ensemble" {
        let mut sources = draft.sources.clone();
        sources.sort();
        sources.dedup();
        ensure!(sources.len() >= 2, "Selecione dois projetos diferentes.");
        for id in &sources {
            ensure!(
                existing.iter().any(|p| &p.id == id
                    && p.language == draft.language
                    && p.vocab == draft.vocab.trim()
                    && ["tfidf", "mllm"].contains(&p.backend.as_str())),
                "O projeto {id} deve usar o mesmo idioma e vocabulário e ser TF-IDF ou MLLM."
            );
        }
        fields.insert(
            "sources".into(),
            toml::Value::String(
                sources
                    .iter()
                    .map(|s| format!("{s}:1.0"))
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        );
    } else {
        let language = match draft.language.as_str() {
            "en" => "english",
            "es" => "spanish",
            _ => "portuguese",
        };
        fields.insert(
            "analyzer".into(),
            toml::Value::String(format!("snowball({language})")),
        );
    }
    let mut config = toml::Table::new();
    config.insert(draft.id.clone(), toml::Value::Table(fields));
    let text = toml::to_string_pretty(&config)?;
    let path = directory(runtime)?.join(format!("project-{}.toml", draft.id));
    // Each project gets a new file; existing configurations are never overwritten.
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path)
        .context("Não foi possível criar o arquivo. Verifique se o destino já existe ou se a pasta permite gravação.")?;
    file.write_all(text.as_bytes())?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDirectory;

    fn setup() -> Result<(TestDirectory, ApplicationRuntime)> {
        let dir = TestDirectory::new()?;
        dir.create_python("")?;
        let runtime = ApplicationRuntime::from_application_dir(dir.path(), false)?;
        Ok((dir, runtime))
    }

    #[test]
    fn creation_escapes_names_and_preserves_existing_configuration() -> Result<()> {
        let (dir, runtime) = setup()?;
        let draft = Draft {
            name: "Ciência \"agrícola\"".into(),
            id: suggested_id("Ciência agrícola"),
            vocab: "nalt".into(),
            ..Default::default()
        };
        assert_eq!(draft.id, "ciencia-agricola");
        let path = create(&runtime, &draft)?;
        let original = std::fs::read_to_string(&path)?;
        let parsed: toml::Table = toml::from_str(&original)?;
        assert_eq!(
            parsed[&draft.id]["name"].as_str(),
            Some(draft.name.as_str())
        );
        assert!(create(&runtime, &draft).is_err());
        assert_eq!(std::fs::read_to_string(&path)?, original);
        let traversal = Draft {
            id: "../fora".into(),
            ..draft.clone()
        };
        assert!(create(&runtime, &traversal).is_err());
        std::fs::write(
            dir.path().join("workspace/projects/legacy.cfg"),
            "[legacy]\n",
        )?;
        assert!(
            create(
                &runtime,
                &Draft {
                    id: "outro".into(),
                    ..draft
                }
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn ensemble_requires_distinct_compatible_sources() -> Result<()> {
        let (_dir, runtime) = setup()?;
        let base = Draft {
            name: "Agricultura".into(),
            id: "fonte-a".into(),
            vocab: "nalt".into(),
            language: "en".into(),
            ..Default::default()
        };
        create(&runtime, &base)?;
        create(
            &runtime,
            &Draft {
                id: "fonte-b".into(),
                backend: "mllm".into(),
                ..base.clone()
            },
        )?;
        let ensemble = Draft {
            id: "combinado".into(),
            backend: "ensemble".into(),
            sources: vec!["fonte-a".into(), "fonte-b".into()],
            ..base
        };
        assert!(
            create(
                &runtime,
                &Draft {
                    language: "pt".into(),
                    ..ensemble.clone()
                }
            )
            .is_err()
        );
        assert!(
            create(
                &runtime,
                &Draft {
                    sources: vec!["fonte-a".into(), "fonte-a".into()],
                    ..ensemble.clone()
                }
            )
            .is_err()
        );
        let path = create(&runtime, &ensemble)?;
        let parsed: toml::Table = toml::from_str(&std::fs::read_to_string(path)?)?;
        assert_eq!(
            parsed["combinado"]["sources"].as_str(),
            Some("fonte-a:1.0,fonte-b:1.0")
        );
        Ok(())
    }
}
