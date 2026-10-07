use super::{AnnifRuntime, ProcessResult, Project, commands};
use anyhow::Result;
use std::process::Command;

#[derive(Clone)]
pub struct SystemRuntime {
    pub python_cmd: String,
    pub annif_cmd: String,
}

impl SystemRuntime {
    pub fn new(python_cmd: String, annif_cmd: String) -> Self {
        Self {
            python_cmd,
            annif_cmd,
        }
    }

    fn annif_command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(&self.annif_cmd);
        command.args(args);
        command
    }
}

impl AnnifRuntime for SystemRuntime {
    fn check_python(&self) -> Result<ProcessResult> {
        let mut command = Command::new(&self.python_cmd);
        command.arg("--version");
        commands::run(command)
    }

    fn check_annif(&self) -> Result<ProcessResult> {
        commands::run(self.annif_command(&["--version"]))
    }

    fn list_projects(&self) -> Result<Vec<Project>> {
        commands::list_projects(self.annif_command(&["list-projects"]))
    }

    fn train(
        &self,
        project_id: &str,
        data_path: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        commands::progressive(
            self.annif_command(&["train", project_id, data_path]),
            sender,
        )
    }

    fn suggest(&self, project_id: &str, file_path: &str) -> Result<ProcessResult> {
        commands::run(self.annif_command(&["suggest", project_id, file_path]))
    }

    fn load_vocab(
        &self,
        vocab_id: &str,
        vocab_path: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        commands::progressive(
            self.annif_command(&["load-vocab", vocab_id, vocab_path]),
            sender,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_executable_has_actionable_error() {
        let missing = std::env::temp_dir()
            .join("annif-manager-missing-runtime")
            .join("python-does-not-exist");
        let runtime = SystemRuntime::new(missing.to_string_lossy().into_owned(), String::new());
        let error = runtime
            .check_python()
            .expect_err("the executable must not exist");
        assert!(error.to_string().contains("Verifique o caminho"));
    }

    #[cfg(unix)]
    #[test]
    fn captures_both_streams_and_exit_status_with_unicode_path() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{SystemTime, UNIX_EPOCH};

        struct TestDirectory(std::path::PathBuf);
        impl Drop for TestDirectory {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = TestDirectory(
            std::env::temp_dir().join(format!("annif diagnóstico {} {unique}", std::process::id())),
        );
        std::fs::create_dir(&directory.0)?;
        let executable = directory.0.join("annif simulado");
        std::fs::write(
            &executable,
            "#!/bin/sh\n[ \"$#\" = 1 ] && [ \"$1\" = --version ] || exit 99\nprintf 'Annif teste\\n'\nprintf 'aviso teste\\n' >&2\nexit 7\n",
        )?;
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700))?;
        let runtime = SystemRuntime::new(String::new(), executable.to_string_lossy().into_owned());
        let result = runtime.check_annif()?;
        assert_eq!(result.stdout, "Annif teste\n");
        assert_eq!(result.stderr, "aviso teste\n");
        assert_eq!(result.exit_code, Some(7));
        assert!(result.command.contains("annif simulado"));
        // stderr alone does not mean the command failed.
        std::fs::write(&executable, "#!/bin/sh\nprintf 'Python teste\\n' >&2\n")?;
        let result = runtime.check_annif()?;
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stderr, "Python teste\n");
        Ok(())
    }
}
