//! Integração opt-in com o pacote real; nunca usa dados do workspace do usuário.
use crate::annif::AnnifRuntime;
use crate::services::{processing, projects, runtime::ApplicationRuntime, training, vocabularies};
use crate::test_support::TestDirectory;
use anyhow::{Context, Result};
use std::path::PathBuf;

#[test]
#[ignore = "requer ANNIF_TEST_PACKAGE apontando para o pacote Windows extraído"]
fn existing_workflows_with_bundled_annif() -> Result<()> {
    let package =
        PathBuf::from(std::env::var("ANNIF_TEST_PACKAGE").context("Defina ANNIF_TEST_PACKAGE")?);
    let temporary = TestDirectory::new()?;
    let workspace = temporary.path().join("workspace de validação");
    let mut runtime = ApplicationRuntime::from_application_dir(&package, false)?;
    runtime.configure_workspace(&workspace)?;
    projects::save_for_runtime(
        &runtime,
        &workspace.join("projects/projetos ação.toml"),
        projects::EXAMPLE,
    )?;
    let listed = runtime.list_projects()?;
    assert_eq!(listed.len(), 3);
    assert!(
        listed
            .iter()
            .any(|p| p.name == "Meu projeto TF-IDF" && p.backend == "tfidf")
    );
    assert!(listed.iter().any(|p| p.backend == "mllm"));
    assert!(listed.iter().any(|p| p.backend == "ensemble"));
    let vocab = workspace.join("vocabularies/vocabulário.tsv");
    std::fs::write(
        &vocab,
        "https://example.org/computacao\tComputação\nhttps://example.org/jardinagem\tJardinagem\n",
    )?;
    let (tx, rx) = crossbeam_channel::unbounded();
    let loaded = vocabularies::start(&runtime, "meu-vocab", &vocab.to_string_lossy(), "pt", tx)?
        .join()
        .map_err(|_| anyhow::anyhow!("Worker falhou"))??;
    assert_eq!(loaded.exit_code, Some(0));
    assert!(rx.iter().any(|line| line.contains("Loading vocabulary")));
    let corpus = workspace.join("datasets/corpus português");
    std::fs::create_dir(&corpus)?;
    for (subject, text) in [
        (
            "computacao",
            "Computação programação computadores software sistemas tecnologia. Referência secundária: jardinagem.",
        ),
        (
            "jardinagem",
            "Jardinagem plantas flores jardins árvores cultivo natureza. Referência secundária: computação.",
        ),
    ] {
        for n in 0..10 {
            let file = corpus.join(format!("{subject} {n}.txt"));
            std::fs::write(&file, text)?;
            std::fs::write(
                file.with_extension("key"),
                format!("<https://example.org/{subject}>\n"),
            )?;
        }
    }
    for project in ["meu-tfidf", "meu-mllm"] {
        let (tx, rx) = crossbeam_channel::unbounded();
        let trained = training::start(&runtime, project, &corpus.to_string_lossy(), tx)?
            .join()
            .map_err(|_| anyhow::anyhow!("Worker falhou"))??;
        assert_eq!(trained.exit_code, Some(0));
        let _: Vec<_> = rx.iter().collect();
    }
    let inputs = workspace.join("datasets/entradas");
    std::fs::create_dir(&inputs)?;
    let doc = inputs.join("texto para sugestão.txt");
    std::fs::write(
        &doc,
        "Computação programação computadores software sistemas.",
    )?;
    let files = processing::input_files(&[inputs.display().to_string()])?;
    assert_eq!(files.len(), 1);
    let mut results = Vec::new();
    for project in ["meu-tfidf", "meu-mllm", "meu-ensemble"] {
        let result = processing::suggest(&runtime, project, &files[0]);
        assert!(result.error.is_none(), "{:?}", result.error);
        assert!(
            result
                .suggestions
                .iter()
                .any(|s| s.label == "Computação" && s.score > 0.0)
        );
        results.push(result);
    }
    let failed = processing::suggest(&runtime, "projeto-inexistente", &doc);
    assert!(failed.error.is_some());
    results.push(failed);
    let exported = workspace.join("results/sugestões.json");
    processing::export(&exported, &results)?;
    let json: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(exported)?)?;
    assert_eq!(json.as_array().unwrap().len(), 4);
    assert!(json[0]["suggestions"][0]["score"].is_number());
    assert!(json[3]["error"].is_string());
    runtime.configure_workspace(&temporary.path().join("outro workspace"))?;
    assert!(runtime.list_projects()?.is_empty());
    assert!(
        runtime
            .configure_workspace(&package.join("runtime"))
            .is_err()
    );
    Ok(())
}
