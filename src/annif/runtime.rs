use crate::annif::project::Project;
use anyhow::Result;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub cancelled: bool,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: Duration,
}

pub trait AnnifRuntime: Send + Sync {
    fn check_python(&self) -> Result<ProcessResult>;
    fn check_annif(&self) -> Result<ProcessResult>;

    // MVP 1
    fn list_projects(&self) -> Result<Vec<Project>>;
    fn list_vocabularies(&self) -> Result<Vec<super::Vocabulary>>;

    /// Executes a command and returns the progressive output via channels,
    /// along with a handle to wait for the final result.
    /// Returns (receiver for stdout/stderr lines, handle to wait for completion)
    fn train(
        &self,
        project_id: &str,
        data_path: &str,
        log_sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>>;

    fn suggest(&self, project_id: &str, file_path: &str) -> Result<ProcessResult>;

    fn load_vocab(
        &self,
        vocab_id: &str,
        vocab_path: &str,
        language: &str,
        log_sender: crossbeam_channel::Sender<String>,
    ) -> Result<std::thread::JoinHandle<Result<ProcessResult>>>;
}
