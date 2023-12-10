use std::{collections::HashMap, io::IsTerminal, path::PathBuf};

use anyhow::Context;
use pcaplog::extract_messages;
use tracing_subscriber::EnvFilter;

#[derive(Debug)]
pub struct Failure {
    pub file: PathBuf,
    pub expected: usize,
    pub got: usize,
}

#[test]
fn captures() -> anyhow::Result<()> {
    init_test_logger();
    let expected_counts = {
        let mut counts = HashMap::new();
        // TODO: update these counts
        // counts.insert(PathBuf::from("../captures/pycharm/startup.pcapng"), 0);
        // counts.insert(
        //     PathBuf::from("../captures/vscode/dlv-debug-session.pcapng"),
        //     0,
        // );
        // counts.insert(
        //     PathBuf::from("../captures/vscode/full-session-multiple-breakpoints.pcapng"),
        //     0,
        // );
        // counts.insert(
        //     PathBuf::from("../captures/vscode/full-session-testpy.pcapng"),
        //     0,
        // );
        // counts.insert(PathBuf::from("../captures/vscode/session2.pcapng"), 0);
        // counts.insert(PathBuf::from("../captures/vscode/stepover-go.pcapng"), 0);
        counts.insert(
            PathBuf::from("../captures/vscode/vscode-attach-connect.pcapng"),
            34,
        );
        counts
    };

    let files = glob::glob("../captures/**/*.pcap*").context("listing capture files")?;
    let mut failures = Vec::new();
    for file in files {
        let file = file.context("invalid file")?;
        if let Some(count) = expected_counts.get(&file) {
            let messages = extract_messages(&file).context("extracting messages")?;
            if messages.len() != *count {
                failures.push(Failure {
                    file: file.clone(),
                    expected: *count,
                    got: messages.len(),
                });
            }
        } else {
            tracing::warn!("file {} missing expected value", file.display());
            // TODO: raise error if capture not found
        }
    }
    assert_eq!(failures.len(), 0, "Failures: {:?}", failures);
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
