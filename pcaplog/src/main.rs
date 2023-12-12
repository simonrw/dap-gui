use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use pcaplog::extract_messages;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    file: PathBuf,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args = Args::parse();
    tracing::debug!(?args, "parsed command line arguments");

    let messages = extract_messages(&args.file).context("extracting messages")?;
    for message in messages {
        println!(
            "{}",
            serde_json::to_string(&message).context("serialising messages")?
        );
    }

    Ok(())
}
