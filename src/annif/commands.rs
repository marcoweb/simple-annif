use super::{ProcessResult, Project};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::Instant;

fn description(command: &Command) -> String {
    std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|part| format!("{part:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn prepare(command: &mut Command) {
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
}

pub fn run(mut command: Command) -> Result<ProcessResult> {
    prepare(&mut command);
    let description = description(&command);
    let started = Instant::now();
    let output = command.output().with_context(|| {
        format!("Não foi possível executar {description}. Verifique o caminho do executável, as permissões e a integridade do runtime")
    })?;
    Ok(ProcessResult {
        command: description,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
        duration: started.elapsed(),
    })
}

pub fn ensure_success(result: ProcessResult) -> Result<ProcessResult> {
    anyhow::ensure!(
        result.exit_code == Some(0),
        "Comando {} falhou (código {:?}):\n{}\n{}",
        result.command,
        result.exit_code,
        result.stdout,
        result.stderr
    );
    Ok(result)
}

pub fn list_projects(mut command: impl FnMut(&[&str]) -> Result<Command>) -> Result<Vec<Project>> {
    let result = ensure_success(run(command(&["list-projects"])?)?)?;
    anyhow::ensure!(
        result
            .stdout
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("Project ID")),
        "Formato inesperado em list-projects: {}",
        result.stdout
    );
    result
        .stdout
        .lines()
        .skip(2)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let id = line.split_whitespace().next().context("Projeto sem ID")?;
            // A tabela não contém backend; show-project fornece os campos nomeados.
            let details = ensure_success(run(command(&["show-project", id])?)?)?;
            parse_project(&details.stdout)
        })
        .collect()
}

fn parse_project(text: &str) -> Result<Project> {
    let field = |key: &str| -> Result<String> {
        text.lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .with_context(|| format!("Campo {key} ausente em show-project: {text}"))
    };
    Ok(Project {
        project_id: field("Project ID:")?,
        name: field("Project Name:")?,
        backend: field("Backend:")?,
    })
}

fn read_logs(
    stream: impl Read,
    sender: crossbeam_channel::Sender<String>,
    label: &str,
) -> Result<String> {
    let mut reader = BufReader::new(stream);
    let mut bytes = Vec::new();
    let mut output = Vec::new();
    loop {
        bytes.clear();
        if reader.read_until(b'\n', &mut bytes)? == 0 {
            break;
        }
        output.extend_from_slice(&bytes);
        let line = String::from_utf8_lossy(&bytes);
        let _ = sender.send(format!("[{label}] {}", line.trim_end_matches(['\r', '\n'])));
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

pub fn progressive(
    mut command: Command,
    sender: crossbeam_channel::Sender<String>,
) -> Result<JoinHandle<Result<ProcessResult>>> {
    prepare(&mut command);
    let description = description(&command);
    let started = Instant::now();
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!("Não foi possível executar {description}. Verifique a integridade do runtime")
        })?;
    let stdout = child
        .stdout
        .take()
        .context("Não foi possível capturar stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("Não foi possível capturar stderr")?;
    let output_sender = sender.clone();
    let out = thread::spawn(move || read_logs(stdout, output_sender, "STDOUT"));
    let err = thread::spawn(move || read_logs(stderr, sender, "STDERR"));
    Ok(thread::spawn(move || {
        let status = child.wait();
        let stdout = out
            .join()
            .map_err(|_| anyhow::anyhow!("Falha na leitura de stdout"))?;
        let stderr = err
            .join()
            .map_err(|_| anyhow::anyhow!("Falha na leitura de stderr"))?;
        let stdout = stdout?;
        let stderr = stderr?;
        let status = status?;
        if !status.success() {
            anyhow::bail!(
                "Comando {description} falhou (código {:?}): {stderr}",
                status.code()
            );
        }
        Ok(ProcessResult {
            command: description,
            stdout,
            stderr,
            exit_code: status.code(),
            duration: started.elapsed(),
        })
    }))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn progressive_failure_reports_exit_code_and_stderr() -> Result<()> {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "printf 'progresso\\n'; printf 'falha técnica\\n' >&2; exit 9",
        ]);
        let (sender, receiver) = crossbeam_channel::unbounded();
        let result = progressive(command, sender)?
            .join()
            .map_err(|_| anyhow::anyhow!("Worker falhou"))?;
        let error = result.expect_err("nonzero exit must fail").to_string();
        assert!(error.contains("9"));
        assert!(error.contains("falha técnica"));
        let logs: Vec<_> = receiver.iter().collect();
        assert!(logs.iter().any(|line| line == "[STDOUT] progresso"));
        assert!(logs.iter().any(|line| line == "[STDERR] falha técnica"));
        Ok(())
    }
}

#[cfg(test)]
mod parsing_tests {
    use super::*;
    #[test]
    fn project_details_keep_names_and_real_backend() -> Result<()> {
        let project = parse_project(
            "Project ID:        assunto-pt\nProject Name:      Ciência e computação\nVocabulary:        vocab\nBackend:           tfidf\nTrained:           True\nModification time: 2026-10-07 12:00:00\n",
        )?;
        assert_eq!(project.project_id, "assunto-pt");
        assert_eq!(project.name, "Ciência e computação");
        assert_eq!(project.backend, "tfidf");
        assert!(parse_project("Project ID: x\n").is_err());
        Ok(())
    }
}
