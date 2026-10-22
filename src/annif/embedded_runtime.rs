use super::{AnnifRuntime, Cancellation, ProcessResult, Project, commands};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(any(windows, test))]
fn validate_python_paths(runtime_dir: &Path) -> Result<()> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(runtime_dir)? {
        let path = entry?.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if let Some(version) = name
            .strip_prefix("python")
            .and_then(|name| name.strip_suffix("._pth"))
            && version.chars().all(|character| character.is_ascii_digit())
        {
            files.push(path);
        }
    }
    anyhow::ensure!(
        files.len() == 1 && files[0].file_name() == Some(std::ffi::OsStr::new("python._pth")),
        "Runtime incorporado incompleto: use um único arquivo python._pth com os caminhos locais do Python (renomeie o arquivo python3XX._pth fornecido pela distribuição)"
    );
    let contents = std::fs::read_to_string(&files[0])?;
    let mut has_packages = false;
    let mut imports_site = false;
    for line in contents.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line == "import site" {
            imports_site = true;
            continue;
        }
        let relative = line.replace('\\', "/");
        anyhow::ensure!(
            !relative.starts_with('/')
                && !relative.contains(':')
                && !relative.split('/').any(|part| part == "..")
                && !relative.starts_with("import "),
            "O arquivo python*._pth deve conter apenas caminhos relativos dentro do runtime e import site"
        );
        let path = runtime_dir.join(&relative);
        anyhow::ensure!(
            path.try_exists()?,
            "Runtime incompleto: {} não foi encontrado",
            path.display()
        );
        anyhow::ensure!(
            path.canonicalize()?
                .starts_with(runtime_dir.canonicalize()?),
            "Um caminho do arquivo python*._pth aponta para fora do runtime"
        );
        if relative.trim_end_matches('/').split('/').next_back() == Some("site-packages") {
            has_packages = true;
        }
    }
    anyhow::ensure!(
        has_packages && imports_site,
        "O arquivo python*._pth deve incluir o diretório site-packages do pacote e a linha import site"
    );
    Ok(())
}

// Mesmo ponto de entrada declarado pelo Annif em project.scripts.
const ANNIF_ENTRY_POINT: &str = include_str!("cli_entry.py");

#[derive(Clone)]
pub struct EmbeddedWindowsRuntime {
    pub(crate) cancellation: Cancellation,
    runtime_dir: PathBuf,
    python: PathBuf,
    workspace: PathBuf,
}

impl EmbeddedWindowsRuntime {
    pub fn new(application_dir: &Path, workspace: &Path) -> Result<Self> {
        anyhow::ensure!(
            application_dir.is_absolute() && workspace.is_absolute(),
            "Os caminhos do pacote e do workspace devem ser absolutos"
        );
        // O layout Linux permite validar a integração durante o desenvolvimento.
        let python = if cfg!(windows) {
            application_dir.join("runtime").join("python.exe")
        } else {
            application_dir.join("runtime").join("bin").join("python3")
        };
        let runtime = Self {
            cancellation: Cancellation::default(),
            runtime_dir: application_dir.join("runtime"),
            python,
            workspace: workspace.to_path_buf(),
        };
        runtime.validate()?;
        runtime.with_workspace(workspace)
    }

    pub fn python(&self) -> &Path {
        &self.python
    }
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    pub fn with_workspace(&self, workspace: &Path) -> Result<Self> {
        anyhow::ensure!(
            workspace.is_absolute(),
            "O caminho do workspace deve ser absoluto"
        );
        // Resolve o ancestral existente para também detectar links para runtime/.
        let mut ancestor = workspace;
        let mut suffix = Vec::new();
        while !ancestor.try_exists()? {
            suffix.push(
                ancestor
                    .file_name()
                    .context("Caminho de workspace inválido")?,
            );
            ancestor = ancestor.parent().context("Caminho de workspace inválido")?;
        }
        let mut resolved = ancestor.canonicalize()?;
        for part in suffix.iter().rev() {
            resolved.push(part);
        }
        anyhow::ensure!(
            !resolved.starts_with(self.runtime_dir.canonicalize()?),
            "O workspace deve ficar fora do diretório runtime"
        );
        Ok(Self {
            cancellation: self.cancellation.clone(),
            runtime_dir: self.runtime_dir.clone(),
            python: self.python.clone(),
            workspace: resolved,
        })
    }

    fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.python.is_file(),
            "Runtime incorporado incompleto: {} não foi encontrado. Descompacte novamente o pacote completo do Annif Manager; não é necessário instalar Python",
            self.python.display()
        );
        #[cfg(windows)]
        validate_python_paths(&self.runtime_dir)?;
        Ok(())
    }

    fn python_command(&self) -> Result<Command> {
        self.validate()?;
        let mut command = Command::new(&self.python);
        command.args(["-I", "-B", "-u", "-X", "utf8"]);
        // Python -I ignora variáveis PYTHON*, diretório atual e pacotes do usuário.
        // Annif deve usar exclusivamente configuração e dados do workspace.
        for (key, _) in std::env::vars_os() {
            if key.to_string_lossy().starts_with("ANNIF_") {
                command.env_remove(key);
            }
        }
        command
            .env_remove("FLASK_APP")
            .env_remove("FLASK_DEBUG")
            .env_remove("FLASK_ENV");
        command.env("ANNIF_CONFIG", "annif.default_config.Config");
        command.env("ANNIF_PROJECTS", self.workspace.join("projects"));
        command.env("ANNIF_DATADIR", self.workspace.join("models"));
        command.env("NLTK_DATA", self.runtime_dir.join("nltk_data"));
        command.current_dir(&self.workspace);
        Ok(command)
    }

    fn annif_command(&self, args: &[&str]) -> Result<Command> {
        let mut command = self.python_command()?;
        command.args(["-c", ANNIF_ENTRY_POINT]);
        command.args(args);
        Ok(command)
    }
}

impl AnnifRuntime for EmbeddedWindowsRuntime {
    fn check_python(&self) -> Result<ProcessResult> {
        let mut command = self.python_command()?;
        command.arg("--version");
        commands::run(command)
            .context("Falha ao iniciar o Python incorporado. Verifique a integridade do pacote")
    }

    fn check_annif(&self) -> Result<ProcessResult> {
        commands::run(self.annif_command(&["--version"])?)
            .context("Falha ao iniciar o Annif incorporado. Verifique a integridade do pacote")
    }

    fn list_projects(&self) -> Result<Vec<Project>> {
        commands::list_projects(|args| self.annif_command(args))
    }

    fn list_vocabularies(&self) -> Result<Vec<super::Vocabulary>> {
        commands::list_vocabularies(self.annif_command(&["list-vocabs"])?)
    }

    fn train(
        &self,
        project_id: &str,
        data_path: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        commands::progressive(
            self.annif_command(&["train", project_id, data_path])?,
            sender,
            self.cancellation.clone(),
        )
    }

    fn suggest(&self, project_id: &str, file_path: &str) -> Result<ProcessResult> {
        let (sender, receiver) = crossbeam_channel::unbounded();
        drop(receiver); // Suggest retains the complete streams in ProcessResult.
        commands::cancellable(
            self.annif_command(&["suggest", project_id, file_path])?,
            sender,
            &self.cancellation,
        )
    }

    fn load_vocab(
        &self,
        vocab_id: &str,
        vocab_path: &str,
        language: &str,
        sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>> {
        commands::progressive(
            self.annif_command(&["load-vocab", vocab_id, vocab_path, "--language", language])?,
            sender,
            self.cancellation.clone(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::TestDirectory;
    use std::ffi::OsStr;

    #[test]
    fn python_search_paths_must_stay_inside_the_bundle() -> Result<()> {
        let package = TestDirectory::new()?;
        let runtime_dir = package.path().join("runtime");
        let packages = runtime_dir.join("Lib").join("site-packages");
        std::fs::create_dir_all(&packages)?;
        assert!(validate_python_paths(&runtime_dir).is_err());
        let pth = runtime_dir.join("python._pth");
        std::fs::write(&pth, ".\nLib/site-packages\nimport site\n")?;
        validate_python_paths(&runtime_dir)?;
        for bad_path in ["../external", "/external", "C:\\external", "Lib/missing"] {
            std::fs::write(
                &pth,
                format!("{bad_path}\nLib/site-packages\nimport site\n"),
            )?;
            assert!(validate_python_paths(&runtime_dir).is_err(), "{bad_path}");
        }
        Ok(())
    }

    #[test]
    fn command_uses_bundled_python_isolation_and_workspace() -> Result<()> {
        let package = TestDirectory::new()?;
        package.create_python("")?;
        let workspace = package.path().join("workspace");
        let runtime = EmbeddedWindowsRuntime::new(package.path(), &workspace)?;
        let workspace = runtime.workspace().to_path_buf();
        let command = runtime.annif_command(&["suggest", "projeto", "texto com espaços.txt"])?;
        assert_eq!(command.get_program(), package.python());
        let args: Vec<_> = command.get_args().collect();
        assert_eq!(
            args,
            [
                "-I",
                "-B",
                "-u",
                "-X",
                "utf8",
                "-c",
                ANNIF_ENTRY_POINT,
                "suggest",
                "projeto",
                "texto com espaços.txt"
            ]
            .map(OsStr::new)
        );
        assert_eq!(command.get_current_dir(), Some(workspace.as_path()));
        let env: std::collections::HashMap<_, _> = command.get_envs().collect();
        assert_eq!(
            env.get(OsStr::new("ANNIF_PROJECTS")),
            Some(&Some(workspace.join("projects").as_os_str()))
        );
        assert_eq!(
            env.get(OsStr::new("ANNIF_DATADIR")),
            Some(&Some(workspace.join("models").as_os_str()))
        );
        assert_eq!(
            env.get(OsStr::new("ANNIF_CONFIG")),
            Some(&Some(OsStr::new("annif.default_config.Config")))
        );
        assert_eq!(
            env.get(OsStr::new("NLTK_DATA")),
            Some(&Some(
                package.path().join("runtime").join("nltk_data").as_os_str()
            ))
        );
        Ok(())
    }

    #[test]
    fn rejects_workspace_inside_runtime_including_symlinks() -> Result<()> {
        let package = TestDirectory::new()?;
        package.create_python("")?;
        let runtime =
            EmbeddedWindowsRuntime::new(package.path(), &package.path().join("workspace"))?;
        assert!(
            runtime
                .with_workspace(&package.path().join("runtime").join("dados"))
                .is_err()
        );
        assert!(
            runtime
                .with_workspace(Path::new("relative-workspace"))
                .is_err()
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                package.path().join("runtime"),
                package.path().join("link"),
            )?;
            assert!(
                runtime
                    .with_workspace(&package.path().join("link").join("dados"))
                    .is_err()
            );
        }
        Ok(())
    }
}
