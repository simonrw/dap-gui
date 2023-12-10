use std::{io::IsTerminal, path::PathBuf};

use anyhow::Context;
use pcaplog::extract_messages;
use rstest::rstest;
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
pub struct Failure {
    pub file: PathBuf,
    pub expected: usize,
    pub got: usize,
}

#[rstest]
#[trace]
#[case("../captures/vscode/vscode-attach-connect.pcapng", 34)]
#[trace]
#[case("../captures/vscode/dlv-debug-session.pcapng", 80)]
#[trace]
#[case("../captures/vscode/full-session-multiple-breakpoints.pcapng", 108)]
#[trace]
#[case("../captures/vscode/full-session-testpy.pcapng", 93)]
#[trace]
#[case("../captures/vscode/session1.pcapng", 74)]
#[trace]
#[case("../captures/vscode/session2.pcapng", 169)]
#[trace]
#[case("../captures/vscode/stepover-go.pcapng", 16)]
#[trace]
#[case("../captures/vscode/vscode-attach-connect.pcapng", 34)]
fn capture(#[case] path: &str, #[case] expected_count: usize) -> anyhow::Result<()> {
    init_test_logger();

    let messages = extract_messages(path).context("extracting messages")?;

    assert_eq!(messages.len(), expected_count);

    Ok(())
}

fn init_test_logger() {
    let in_ci = std::env::var("CI")
        .map(|val| val == "true")
        .unwrap_or(false);

    if std::io::stderr().is_terminal() || in_ci {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .try_init();
    }
}
