use std::{
    env,
    process::{Command, Stdio},
};

use clap::Parser;

#[derive(Parser)]
enum Args {
    Test,
    Doctest,
    InitVenv,
    TuiPoc,
}

macro_rules! run {
    ($command:expr) => {{
        if !Command::new($command)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output().unwrap().status.success() {
                panic!("command failed");
        }
    }};

    ($command:expr, $($arg:expr),+) => {{
        if !Command::new($command)
            .args([
                $($arg),+
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output().unwrap().status.success() {
                panic!("command failed");
        }
    }};
}

macro_rules! cargo {
    ($($arg:expr),+) => {{
        let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
        if !Command::new(cargo)
            .args([
                $($arg),+
            ])
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .output().unwrap().status.success() {
                panic!("command failed");
        }
    }};
}

fn main() {
    let args = Args::parse();

    match args {
        Args::Test => {
            cargo!(
                "nextest",
                "run",
                "--locked",
                "--all-features",
                "--all-targets"
            );
        }
        Args::Doctest => {
            cargo!("test", "--locked", "--all-features", "--doc");
        }
        Args::InitVenv => {
            run!("python", "-m", "venv", ".venv");
            run!(".venv/bin/python", "-m", "pip", "install", "debugpy");
        }
        Args::TuiPoc => {
            let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());

            if !Command::new(cargo)
                .args([
                    "run",
                    "-p",
                    "tui-poc",
                    "--",
                    "--config",
                    "launch.json",
                    "--name",
                    "Launch",
                    "--state",
                    "crates/tui-poc/state.json",
                    "--log",
                    "dap-gui.log",
                ])
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .output()
                .unwrap()
                .status
                .success()
            {
                panic!("command failed");
            }
        }
    };
}
