use anyhow::{Context, Result};
use std::{
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Stdio},
    sync::mpsc,
    thread,
};

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

static SERVER_READY_TEXT: &str = "Listening for incoming Client connections";

pub struct PythonDebugServer {
    child: Child,
}

impl PythonDebugServer {
    #[tracing::instrument(skip(config))]
    pub fn new(config: DebugServerConfig) -> Result<Self> {
        let mut child = std::process::Command::new("python")
            .args([
                "-m",
                "debugpy.adapter",
                "--host",
                "127.0.0.1",
                "--port",
                &format!("{}", config.port),
                "--log-stderr",
            ])
            .stderr(Stdio::piped())
            .current_dir(&config.working_dir)
            .spawn()
            .context("spawning debugpy server")?;

        let stderr = child.stderr.take().unwrap();
        let reader = BufReader::new(stderr);

        let (tx, rx) = mpsc::channel();
        let mut should_send = true;
        thread::spawn(move || {
            for line in reader.lines() {
                let line = line.unwrap();
                tracing::trace!(%line, "debugger line");
                if should_send && line.contains(SERVER_READY_TEXT) {
                    let _ = tx.send(());
                    should_send = false;
                }
            }
        });
        let _ = rx.recv();

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
