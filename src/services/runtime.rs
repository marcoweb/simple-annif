use super::workspace::Workspace;
use crate::annif::{AnnifRuntime, EmbeddedWindowsRuntime, ProcessResult, Project, SystemRuntime};
use anyhow::{Context, Result};
use std::path::Path;

#[derive(Clone)]
pub enum ApplicationRuntime {
    System(SystemRuntime),
    Embedded(EmbeddedWindowsRuntime),
    Unavailable(String),
}

impl ApplicationRuntime {
    pub fn context_key(&self) -> String {
        match self {
            Self::Embedded(runtime) => runtime.workspace().display().to_string(),
            Self::System(runtime) => format!("{}|{}", runtime.python_cmd, runtime.annif_cmd),
            Self::Unavailable(error) => error.clone(),
        }
    }

    pub fn detect() -> Self {
        match std::env::current_exe()
            .context("Não foi possível localizar o executável da aplicação")
            .and_then(|executable| {
                let directory = executable
                    .parent()
                    .context("Não foi possível localizar o diretório da aplicação")?;
                Self::from_application_dir(directory, cfg!(debug_assertions))
            }) {
            Ok(runtime) => runtime,
            Err(error) => Self::Unavailable(format!("{error:#}")),
        }
    }

    pub fn from_application_dir(directory: &Path, allow_system: bool) -> Result<Self> {
        anyhow::ensure!(
            directory.is_absolute(),
            "O diretório da aplicação deve ser absoluto"
        );
        if directory.join("runtime").try_exists()? || !allow_system {
            let workspace_path = directory.join("workspace");
            let runtime = EmbeddedWindowsRuntime::new(directory, &workspace_path)?;
            Ok(Self::Embedded(runtime))
        } else {
            Ok(Self::System(SystemRuntime::new(
                if cfg!(windows) { "python" } else { "python3" }.into(),
                "annif".into(),
            )))
        }
    }

    pub fn configure_workspace(&mut self, root: &Path) -> Result<()> {
        let Self::Embedded(runtime) = self else {
            anyhow::bail!("Workspace portátil disponível apenas com runtime incorporado");
        };
        let candidate = runtime.with_workspace(root)?;
        let workspace = Workspace::prepare(candidate.workspace())?;
        *runtime = candidate.with_workspace(workspace.root())?;
        Ok(())
    }

    fn backend(&self) -> Result<&dyn AnnifRuntime> {
        match self {
            Self::System(runtime) => Ok(runtime),
            Self::Embedded(runtime) => {
                Workspace::prepare(runtime.workspace())?;
                Ok(runtime)
            }
            Self::Unavailable(error) => anyhow::bail!("{error}"),
        }
    }
}

impl AnnifRuntime for ApplicationRuntime {
    fn check_python(&self) -> Result<ProcessResult> {
        self.backend()?.check_python()
    }
    fn check_annif(&self) -> Result<ProcessResult> {
        self.backend()?.check_annif()
    }
    fn list_projects(&self) -> Result<Vec<Project>> {
        self.backend()?.list_projects()
    }
    fn list_vocabularies(&self) -> Result<Vec<crate::annif::Vocabulary>> {
        self.backend()?.list_vocabularies()
    }
    fn train(
        &self,
        project_id: &str,
        data_path: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        self.backend()?.train(project_id, data_path, sender)
    }
    fn suggest(&self, project_id: &str, file_path: &str) -> Result<ProcessResult> {
        self.backend()?.suggest(project_id, file_path)
    }
    fn load_vocab(
        &self,
        vocab_id: &str,
        vocab_path: &str,
        language: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        self.backend()?
            .load_vocab(vocab_id, vocab_path, language, sender)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDirectory;

    #[test]
    fn portable_mode_never_falls_back_to_system() -> Result<()> {
        let package = TestDirectory::new()?;
        assert!(matches!(
            ApplicationRuntime::from_application_dir(package.path(), true)?,
            ApplicationRuntime::System(_)
        ));
        assert!(ApplicationRuntime::from_application_dir(package.path(), false).is_err());
        std::fs::create_dir(package.path().join("runtime"))?;
        assert!(ApplicationRuntime::from_application_dir(package.path(), true).is_err());
        package.create_python("")?;
        assert!(matches!(
            ApplicationRuntime::from_application_dir(package.path(), true)?,
            ApplicationRuntime::Embedded(_)
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn relocated_bundle_runs_from_its_workspace_and_preserves_progressive_logs() -> Result<()> {
        let mut package = TestDirectory::new()?;
        package.create_python("#!/bin/sh\nprintf 'cwd=%s\\n' \"$PWD\"\nprintf 'projects=%s\\n' \"$ANNIF_PROJECTS\"\nprintf 'models=%s\\n' \"$ANNIF_DATADIR\"\nprintf 'arg=%s\\n' \"$@\"\nprintf 'log de erro\\n' >&2\n")?;
        let relocated = package.path().with_file_name(format!(
            "{} movido",
            package
                .path()
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Caminho inválido"))?
                .to_string_lossy()
        ));
        std::fs::rename(package.path(), &relocated)?;
        package.0 = relocated;
        let mut runtime = ApplicationRuntime::from_application_dir(package.path(), false)?;
        let workspace = package.path().join("workspace");
        let result = runtime.check_annif()?;
        assert_eq!(result.exit_code, Some(0));
        assert!(
            result
                .stdout
                .contains(&format!("cwd={}\n", workspace.display()))
        );
        assert!(result.stdout.contains(&format!(
            "projects={}\n",
            workspace.join("projects").display()
        )));
        assert!(
            result
                .stdout
                .contains(&format!("models={}\n", workspace.join("models").display()))
        );
        assert!(result.stdout.contains("arg=-I\n"));
        for subdirectory in [
            "projects",
            "vocabularies",
            "datasets",
            "models",
            "results",
            "logs",
        ] {
            assert!(workspace.join(subdirectory).is_dir());
        }
        let (sender, receiver) = crossbeam_channel::unbounded();
        let result = runtime
            .train("projeto", "dataset com espaços", sender)?
            .join()
            .map_err(|_| anyhow::anyhow!("Worker falhou"))??;
        assert!(result.stdout.contains("arg=dataset com espaços\n"));
        assert_eq!(result.stderr, "log de erro\n");
        let logs: Vec<_> = receiver.iter().collect();
        assert!(logs.iter().any(|line| line == "[STDERR] log de erro"));
        assert!(logs.iter().any(|line| line == "[STDOUT] arg=train"));
        let custom = package.path().join("meus experimentos");
        runtime.configure_workspace(&custom)?;
        let result = runtime.check_annif()?;
        assert!(
            result
                .stdout
                .contains(&format!("cwd={}\n", custom.display()))
        );
        // O Python removido depois da seleção também deve falhar sem fallback.
        std::fs::remove_file(package.python())?;
        assert!(runtime.check_python().is_err());
        Ok(())
    }
}
