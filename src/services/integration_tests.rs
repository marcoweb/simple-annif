//! Integração opt-in com o pacote real; nunca usa dados do workspace do usuário.
use crate::annif::AnnifRuntime;
use crate::services::{processing, projects, runtime::ApplicationRuntime, training, vocabularies};
use crate::test_support::TestDirectory;
use anyhow::{Context, Result};
use std::path::PathBuf;

#[test]
#[ignore = "requer ANNIF_TEST_PACKAGE, ANNIF_TEST_CORPUS e ANNIF_TEST_NALT_MODELS"]
fn real_nalt_corpus_with_bundled_annif() -> Result<()> {
    let package = PathBuf::from(std::env::var("ANNIF_TEST_PACKAGE")?);
    let corpus = PathBuf::from(std::env::var("ANNIF_TEST_CORPUS")?);
    let vocabulary = PathBuf::from(std::env::var("ANNIF_TEST_NALT_MODELS")?);
    let temporary = TestDirectory::new()?;
    let mut runtime = ApplicationRuntime::from_application_dir(&package, false)?;
    runtime.configure_workspace(temporary.path())?;
    projects::save_for_runtime(
        &runtime,
        &temporary.path().join("projects/nalt.toml"),
        include_str!("../../packaging/examples/projects-nalt.toml"),
    )?;
    let destination = temporary.path().join("models/vocabs/nalt");
    std::fs::create_dir_all(&destination)?;
    for name in ["subjects.csv", "subjects.ttl", "subjects.dump.gz"] {
        std::fs::copy(vocabulary.join(name), destination.join(name))?;
    }
    let vocabs = vocabularies::list(&runtime)?;
    assert_eq!(vocabs[0].size, Some(76691));
    for project in ["nalt-tfidf", "nalt-mllm"] {
        let (tx, rx) = crossbeam_channel::unbounded();
        let result = training::start(
            &runtime,
            project,
            &corpus.join("train").to_string_lossy(),
            tx,
        )?
        .join()
        .map_err(|_| anyhow::anyhow!("Worker falhou"))??;
        println!("{project}: trained in {} ms", result.duration.as_millis());
        let logs: Vec<_> = rx.iter().collect();
        assert!(!logs.is_empty());
    }
    let files = processing::input_files(&[corpus.join("test").display().to_string()])?;
    assert_eq!(files.len(), 5);
    let mut results = Vec::new();
    for project in ["nalt-tfidf", "nalt-mllm", "nalt-ensemble"] {
        for path in &files {
            let result = processing::suggest(&runtime, project, path);
            assert!(result.error.is_none(), "{:?}", result.error);
            assert!(
                !result.suggestions.is_empty(),
                "{project} {}",
                path.display()
            );
            assert!(
                result
                    .suggestions
                    .iter()
                    .all(|hit| hit.uri.starts_with("https://lod.nal.usda.gov/nalt/")
                        && hit.score.is_finite())
            );
            println!(
                "{project} {}: {} suggestions; first={}",
                path.file_name().unwrap().to_string_lossy(),
                result.suggestions.len(),
                result.suggestions[0].label
            );
            results.push(result);
        }
    }
    let export = temporary.path().join("results/nalt.json");
    processing::export(&export, &results)?;
    let parsed: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(export)?)?;
    assert_eq!(parsed.as_array().unwrap().len(), 15);
    Ok(())
}

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
    let available = vocabularies::list(&runtime)?;
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, "meu-vocab");
    assert!(!available[0].loaded);
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
    let available = vocabularies::list(&runtime)?;
    assert!(available[0].loaded);
    assert_eq!(available[0].size, Some(2));
    assert_eq!(available[0].languages, ["pt"]);
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

#[test]
#[ignore = "requer ANNIF_TEST_PACKAGE; ANNIF_TEST_RDF opcional para validar um RDF real"]
fn rdf_vocabulary_through_ui_service() -> Result<()> {
    let package =
        PathBuf::from(std::env::var("ANNIF_TEST_PACKAGE").context("Defina ANNIF_TEST_PACKAGE")?);
    let temporary = TestDirectory::new()?;
    let workspace = temporary.path().join("workspace RDF ação");
    let mut runtime = ApplicationRuntime::from_application_dir(&package, false)?;
    runtime.configure_workspace(&workspace)?;
    projects::save_for_runtime(
        &runtime,
        &workspace.join("projects/nalt.toml"),
        include_str!("../../packaging/examples/projects-nalt.toml"),
    )?;
    let source = match std::env::var_os("ANNIF_TEST_RDF") {
        Some(path) => PathBuf::from(path),
        None => {
            let file = temporary.path().join("vocabulário #100% com espaços.rdf");
            std::fs::write(
                &file,
                r#"<?xml version="1.0" encoding="utf-8"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:skos="http://www.w3.org/2004/02/skos/core#">
<skos:Concept rdf:about="https://example.org/agricultura"><skos:prefLabel xml:lang="en">Agriculture</skos:prefLabel><skos:prefLabel xml:lang="es">Agricultura</skos:prefLabel></skos:Concept>
</rdf:RDF>"#,
            )?;
            file
        }
    };
    let (tx, rx) = crossbeam_channel::unbounded();
    let result = vocabularies::start(&runtime, "nalt", &source.to_string_lossy(), "en", tx)?
        .join()
        .map_err(|_| anyhow::anyhow!("Worker falhou"))??;
    assert_eq!(result.exit_code, Some(0));
    let logs: Vec<_> = rx.iter().collect();
    assert!(
        logs.iter()
            .any(|line| line.contains("Loading vocabulary from SKOS"))
    );
    let model = workspace.join("models/vocabs/nalt");
    assert!(model.join("subjects.csv").is_file());
    assert!(model.join("subjects.ttl").is_file());
    assert!(model.join("subjects.dump.gz").is_file());
    let subjects = std::fs::read_to_string(model.join("subjects.csv"))?;
    assert!(subjects.contains("en") && subjects.contains("es"));
    println!(
        "RDF carregado pelo serviço da UI. Origem: {}. Linhas no índice CSV: {}. Código: {:?}",
        source.display(),
        subjects.lines().count(),
        result.exit_code
    );
    Ok(())
}
