use super::{Cancellation, ProcessResult, Project, process_tree::ProcessTree};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Read};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

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
        cancelled: false,
        command: description,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
        duration: started.elapsed(),
    })
}

pub fn ensure_success(result: ProcessResult) -> Result<ProcessResult> {
    anyhow::ensure!(!result.cancelled, "Operação cancelada pelo usuário");
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

pub fn list_vocabularies(command: Command) -> Result<Vec<super::Vocabulary>> {
    parse_vocabularies(&ensure_success(run(command)?)?.stdout)
}

fn parse_vocabularies(text: &str) -> Result<Vec<super::Vocabulary>> {
    anyhow::ensure!(
        text.lines()
            .next()
            .is_some_and(|line| line.starts_with("Vocabulary ID")),
        "Formato inesperado em list-vocabs: {text}"
    );
    text.lines()
        .skip(2)
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            // Read from the right: an initialized vocabulary can have no language tags.
            let mut fields = line.trim_end().rsplitn(2, char::is_whitespace);
            let loaded = match fields.next() {
                Some("True") => true,
                Some("False") => false,
                _ => anyhow::bail!("Estado inválido em list-vocabs: {line}"),
            };
            let rest = fields.next().context("Vocabulário incompleto")?.trim_end();
            let (identity, size) = rest
                .rsplit_once(char::is_whitespace)
                .context("Tamanho ausente")?;
            let size = if size == "-" {
                None
            } else {
                Some(size.parse::<usize>()?)
            };
            let mut identity = identity.split_whitespace();
            let id = identity.next().context("ID ausente")?.to_owned();
            let languages = identity
                .next()
                .filter(|value| *value != "-")
                .map(|value| value.split(',').map(str::to_owned).collect())
                .unwrap_or_default();
            anyhow::ensure!(
                identity.next().is_none(),
                "Linha inválida em list-vocabs: {line}"
            );
            Ok(super::Vocabulary {
                id,
                languages,
                size,
                loaded,
            })
        })
        .collect()
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
    command: Command,
    sender: crossbeam_channel::Sender<String>,
    cancellation: Cancellation,
) -> Result<JoinHandle<Result<ProcessResult>>> {
    Ok(thread::Builder::new()
        .name("annif-process".into())
        .spawn(move || {
            let result = cancellable(command, sender, &cancellation)?;
            if result.cancelled {
                Ok(result)
            } else {
                ensure_success(result)
            }
        })?)
}

pub fn cancellable(
    mut command: Command,
    sender: crossbeam_channel::Sender<String>,
    cancellation: &Cancellation,
) -> Result<ProcessResult> {
    prepare(&mut command);
    let description = description(&command);
    let started = Instant::now();
    if cancellation.is_requested() {
        return Ok(ProcessResult {
            cancelled: true,
            command: description,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            duration: started.elapsed(),
        });
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut tree = ProcessTree::spawn(&mut command).with_context(|| {
        format!("Não foi possível executar {description}. Verifique a integridade do runtime")
    })?;
    let stdout = tree
        .child
        .stdout
        .take()
        .context("Não foi possível capturar stdout")?;
    let stderr = tree
        .child
        .stderr
        .take()
        .context("Não foi possível capturar stderr")?;
    let output_sender = sender.clone();
    let out = thread::spawn(move || read_logs(stdout, output_sender, "STDOUT"));
    let err = thread::spawn(move || read_logs(stderr, sender, "STDERR"));
    let mut cancelled = false;
    let status = (|| -> Result<std::process::ExitStatus> {
        loop {
            // A result already completed wins over a late cancellation request.
            if let Some(status) = tree.child.try_wait()? {
                return Ok(status);
            }
            if cancellation.is_requested() {
                tree.terminate()
                    .context("Não foi possível encerrar o processo do Annif")?;
                cancelled = true;
                return Ok(tree.child.wait()?);
            }
            thread::sleep(Duration::from_millis(50));
        }
    })();
    // Ensure child helpers cannot keep the output readers alive after CLI exit.
    drop(tree);
    let stdout = out
        .join()
        .map_err(|_| anyhow::anyhow!("Falha na leitura de stdout"))?;
    let stderr = err
        .join()
        .map_err(|_| anyhow::anyhow!("Falha na leitura de stderr"))?;
    let stdout = stdout?;
    let stderr = stderr?;
    let status = status?;
    Ok(ProcessResult {
        cancelled,
        command: description,
        stdout,
        stderr,
        exit_code: status.code(),
        duration: started.elapsed(),
    })
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
        let result = progressive(command, sender, Cancellation::default())?
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
    fn vocabulary_table_handles_unloaded_multilingual_and_untagged() -> Result<()> {
        let items = parse_vocabularies(
            "Vocabulary ID   Languages   Size   Loaded\n------------------------------------------------\nnalt            en,es       76691  True\npendente        -           -      False\nsem-idioma                  1      True\n",
        )?;
        assert_eq!(items[0].languages, ["en", "es"]);
        assert_eq!(items[0].size, Some(76691));
        assert!(!items[1].loaded);
        assert_eq!(items[1].size, None);
        assert!(items[2].languages.is_empty());
        assert!(parse_vocabularies("erro").is_err());
        assert!(parse_vocabularies("Vocabulary ID\n---\nincompleto\n").is_err());
        assert!(parse_vocabularies("Vocabulary ID   Languages   Size   Loaded\n---\n")?.is_empty());
        Ok(())
    }
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

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use std::io::Write;

    fn fixture(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "annif::commands::cancellation_tests::process_fixture",
                "--nocapture",
            ])
            .env("ANNIF_MANAGER_PROCESS_FIXTURE", mode);
        prepare(&mut command);
        command
    }

    #[test]
    fn process_fixture() {
        let Ok(mode) = std::env::var("ANNIF_MANAGER_PROCESS_FIXTURE") else {
            return;
        };
        if mode == "tree" {
            let mut child = fixture("sleep").spawn().unwrap();
            println!("descendant={}", child.id());
            std::io::stdout().flush().unwrap();
            let _ = child.wait();
        } else if mode == "sleep" {
            println!("ready={}", std::process::id());
            eprintln!("aviso com acentuação");
            std::io::stdout().flush().unwrap();
            thread::sleep(Duration::from_secs(30));
        } else {
            println!("conclusão");
            eprintln!("diagnóstico");
            if mode == "fail" {
                std::process::exit(9);
            }
        }
    }

    #[test]
    fn cancellation_ends_descendants_and_retains_both_streams() -> Result<()> {
        let cancellation = Cancellation::default();
        let (tx, rx) = crossbeam_channel::unbounded();
        let handle = progressive(fixture("tree"), tx, cancellation.clone())?;
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut ready = false;
        let mut stderr = false;
        let mut pid = None;
        while !(ready && stderr && pid.is_some()) {
            let line = rx.recv_timeout(deadline.saturating_duration_since(Instant::now()))?;
            ready |= line.contains("ready=");
            stderr |= line.contains("aviso com acentuação");
            if let Some(value) = line.strip_prefix("[STDOUT] descendant=") {
                pid = Some(value.parse::<u32>()?);
            }
        }
        #[cfg(windows)]
        let descendant = {
            use std::os::windows::io::{FromRawHandle, OwnedHandle};
            use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
            // SAFETY: hold the precise descendant's handle to avoid PID reuse.
            let raw = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid.unwrap()) };
            anyhow::ensure!(
                !raw.is_null(),
                "Descendant must be alive before cancellation"
            );
            unsafe { OwnedHandle::from_raw_handle(raw) }
        };
        let started = Instant::now();
        cancellation.cancel();
        cancellation.cancel(); // repeated clicks must be harmless
        let (done, completion) = crossbeam_channel::bounded(1);
        thread::spawn(move || {
            let _ = done.send(handle.join().unwrap());
        });
        let result = completion.recv_timeout(Duration::from_secs(5))??;
        assert!(result.cancelled);
        assert!(started.elapsed() < Duration::from_secs(5));
        assert!(result.stdout.contains("ready="));
        assert!(result.stderr.contains("aviso com acentuação"));
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{
                Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
            };
            // SAFETY: valid process handle owned above, bounded wait.
            assert_eq!(
                unsafe { WaitForSingleObject(descendant.as_raw_handle(), 1000) },
                WAIT_OBJECT_0
            );
        }
        // A fresh operation is independent of the cancelled token.
        let (tx, _rx) = crossbeam_channel::unbounded();
        let next = progressive(fixture("success"), tx, Cancellation::default())?
            .join()
            .unwrap()?;
        assert!(!next.cancelled);
        assert_eq!(next.exit_code, Some(0));
        Ok(())
    }

    #[test]
    fn pre_cancel_does_not_spawn_and_late_cancel_does_not_change_success() -> Result<()> {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let (tx, _rx) = crossbeam_channel::unbounded();
        let result = cancellable(
            Command::new("annif-manager-must-not-exist"),
            tx,
            &cancellation,
        )?;
        assert!(result.cancelled);
        assert_eq!(result.exit_code, None);
        let cancellation = Cancellation::default();
        let (tx, _rx) = crossbeam_channel::unbounded();
        let result = cancellable(fixture("success"), tx, &cancellation)?;
        cancellation.cancel();
        assert!(!result.cancelled);
        assert!(result.stdout.contains("conclusão"));
        let (tx, _rx) = crossbeam_channel::unbounded();
        let failure = progressive(fixture("fail"), tx, Cancellation::default())?
            .join()
            .unwrap()
            .unwrap_err();
        assert!(failure.to_string().contains('9'));
        assert!(failure.to_string().contains("diagnóstico"));
        Ok(())
    }
}
