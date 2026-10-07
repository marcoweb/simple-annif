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

pub fn list_projects(command: Command) -> Result<Vec<Project>> {
    let result = run(command)?;
    if result.exit_code != Some(0) {
        anyhow::bail!("Falha ao listar projetos: {}", result.stderr);
    }
    // Preserva o parser existente; validar o formato com a versão empacotada.
    let mut projects = Vec::new();
    for line in result.stdout.lines().skip(2) {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() >= 3 {
            projects.push(Project {
                project_id: parts[0].into(),
                name: parts[1..parts.len() - 1].join(" "),
                backend: parts[parts.len() - 1].into(),
            });
        }
    }
    Ok(projects)
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
