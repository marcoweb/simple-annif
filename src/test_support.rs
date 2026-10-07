use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct TestDirectory(pub PathBuf);

impl TestDirectory {
    pub fn new() -> Result<Self> {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "annif pacote portátil {} {unique}",
            std::process::id()
        ));
        std::fs::create_dir(&directory)?;
        Ok(Self(directory))
    }

    pub fn python(&self) -> PathBuf {
        if cfg!(windows) {
            self.0.join("runtime").join("python.exe")
        } else {
            self.0.join("runtime").join("bin").join("python3")
        }
    }

    pub fn create_python(&self, contents: &str) -> Result<()> {
        let python = self.python();
        std::fs::create_dir_all(
            python
                .parent()
                .ok_or_else(|| anyhow::anyhow!("Caminho de teste inválido"))?,
        )?;
        std::fs::write(&python, contents)?;
        #[cfg(windows)]
        {
            std::fs::create_dir_all(self.0.join("runtime").join("Lib").join("site-packages"))?;
            std::fs::write(
                self.0.join("runtime").join("python._pth"),
                ".\nLib/site-packages\nimport site\n",
            )?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&python, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
