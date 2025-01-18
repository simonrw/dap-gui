use std::{io::Write, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{self, Context};
use debugger::Breakpoint;
use debugger::Debugger;

struct App {
    debugger: Debugger,
    stdin: std::io::Stdin,
    stdout: std::io::Stdout,
    input_buffer: String,
}

impl App {
    fn new(debugger: Debugger) -> Self {
        Self {
            debugger,
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
            input_buffer: String::new(),
        }
    }

    fn loop_step(&mut self) -> eyre::Result<ShouldQuit> {
        tracing::trace!("locking stdout");
        let mut stdout = self.stdout.lock();
        tracing::trace!("writing prompt to stdout");
        write!(&mut stdout, "> ")?;
        tracing::trace!("prompt written, flushing stdout");
        stdout.flush()?;
        tracing::trace!("stdout flushed");

        tracing::trace!("reading from stdin");
        let n = self.stdin.read_line(&mut self.input_buffer)?;
        tracing::trace!(%n, "read bytes from stdin");
        let input = self.input_buffer.trim().to_owned();
        tracing::trace!(%input, "parsed command");

        let res = self.handle_input(&input).context("handling command");
        self.input_buffer.clear();
        res
    }

    fn handle_input(&mut self, input: &str) -> eyre::Result<ShouldQuit> {
        match input {
            "q" => return Ok(ShouldQuit::True),
            "c" => {
                tracing::debug!("executing continue command");
                self.debugger.r#continue().context("resuming execution")?;
            }
            "" => return Ok(ShouldQuit::False),
            other => writeln!(self.stdout, "Unhandled commmand: '{}'", other)?,
        }
        Ok(ShouldQuit::False)
    }
}

#[derive(Debug, Parser)]
struct Args {
    launch_configuration: PathBuf,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    breakpoints: Vec<Breakpoint>,
}

fn main() -> eyre::Result<()> {
    color_eyre::install().context("installing color_eyre")?;
    tracing_subscriber::fmt::init();

    let args = Args::parse();
    let debugger = Debugger::from_launch_configuration(args.launch_configuration, args.name)
        .context("creating debugger")?;
    for breakpoint in args.breakpoints {
        tracing::debug!(?breakpoint, "adding breakpoint");
        debugger
            .add_breakpoint(&breakpoint)
            .context("adding breakpoint")?;
    }
    tracing::debug!("breakpoints added");

    let mut app = App::new(debugger);
    tracing::debug!("debugger set up");
    loop {
        match app.loop_step() {
            Ok(ShouldQuit::True) => break,
            Ok(ShouldQuit::False) => {}
            Err(e) => eyre::bail!("Error running command: {e}"),
        }
    }

    Ok(())
}

enum ShouldQuit {
    True,
    False,
}
