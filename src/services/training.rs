use crate::annif::{AnnifRuntime, ProcessResult};
use anyhow::{Context, Result};
use std::{path::Path, thread::JoinHandle};

pub fn start(
    runtime: &impl AnnifRuntime,
    project: &str,
    path: &str,
    logs: crossbeam_channel::Sender<String>,
) -> Result<JoinHandle<Result<ProcessResult>>> {
    let path = Path::new(path.trim());
    anyhow::ensure!(
        path.is_file() || path.is_dir(),
        "Selecione um arquivo ou diretório de treinamento existente"
    );
    let path = path
        .canonicalize()
        .context("Não foi possível acessar o corpus")?;
    runtime.train(project, &path.to_string_lossy(), logs)
}
