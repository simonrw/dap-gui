use anyhow::{Context, Result};
use std::{path::PathBuf, process::Child, thread, time::Duration};

pub struct DebugServerConfig {
    pub working_dir: PathBuf,
    pub filename: PathBuf,
    pub port: u16,
}

pub trait DebugServer {
    fn start(&mut self, config: DebugServerConfig) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
}

pub enum Language {
    Python,
}

pub struct PythonDebugServer {
    child: Child,
}

impl PythonDebugServer {
    pub fn new(config: DebugServerConfig) -> Result<Self> {
        let child = std::process::Command::new("python")
            .args([
                "-m",
                "debugpy",
                "--log-to-stderr",
                "--wait-for-client",
                "--listen",
                &format!("127.0.0.1:{}", config.port),
                &config.filename.display().to_string(),
            ])
            .current_dir(&config.working_dir)
            .spawn()
            .context("spawning debugpy server")?;

        // TODO: wait for ready
        thread::sleep(Duration::from_secs(3));

        Ok(Self { child })
    }

    pub fn stop(&mut self) -> Result<()> {
        self.child.kill().context("killing debug server")?;
        Ok(())
    }
}

impl Drop for PythonDebugServer {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            tracing::error!(error = %e, "debug server still running after program exit");
        }
    }
}
