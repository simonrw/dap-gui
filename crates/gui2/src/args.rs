use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser, Default)]
pub struct Args {
    /// debug rendering
    #[clap(short, long)]
    pub(crate) debug: bool,

    /// Path to the config file
    pub(crate) config_path: PathBuf,

    /// Name of the launch configuration to choose
    #[clap(short, long)]
    pub(crate) name: Option<String>,
}
