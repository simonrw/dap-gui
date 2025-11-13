use std::path::PathBuf;

use clap::Parser;
use eyre::WrapErr;
use pcaplog::extract_messages;
use serde::Serialize;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    file: PathBuf,

    #[clap(short, long, default_value_t = 5678)]
    port: u16,
}

#[derive(Serialize)]
struct Messages(Vec<transport::Message>);

fn main() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let args = Args::parse();
    tracing::debug!(?args, "parsed command line arguments");

    let messages =
        Messages(extract_messages(&args.file, args.port).context("extracting messages")?);
    println!(
        "{}",
        serde_json::to_string_pretty(&messages).context("serializing messages")?
    );

    Ok(())
}
