use crate::annif::{AnnifRuntime, ProcessResult};
use std::io;
use std::sync::mpsc::{self, Receiver};
use std::thread;

pub enum DiagnosticEvent {
    Python(Result<ProcessResult, String>),
    Annif(Result<ProcessResult, String>),
}

/// A UI recebe resultados sem executar processos ou compartilhar locks.
pub fn check_environment(
    runtime: impl AnnifRuntime + 'static,
) -> io::Result<Receiver<DiagnosticEvent>> {
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name("annif-diagnostics".into())
        .spawn(move || {
            let python = runtime.check_python().map_err(|error| format!("{error:#}"));
            if sender.send(DiagnosticEvent::Python(python)).is_err() {
                return;
            }
            let annif = runtime.check_annif().map_err(|error| format!("{error:#}"));
            let _ = sender.send(DiagnosticEvent::Annif(annif));
        })?;
    Ok(receiver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::annif::SystemRuntime;
    use std::time::Duration;

    #[test]
    fn checks_annif_even_when_python_cannot_start() -> anyhow::Result<()> {
        let missing = std::env::temp_dir().join("annif-manager-missing-runtime");
        let runtime = SystemRuntime::new(
            missing
                .join("python-does-not-exist")
                .to_string_lossy()
                .into_owned(),
            missing
                .join("annif-does-not-exist")
                .to_string_lossy()
                .into_owned(),
        );
        let receiver = check_environment(runtime)?;
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5))?,
            DiagnosticEvent::Python(Err(_))
        ));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5))?,
            DiagnosticEvent::Annif(Err(_))
        ));
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
        Ok(())
    }
}
