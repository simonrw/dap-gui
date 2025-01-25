use std::{io::Write, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{self, Context};
use debugger::Breakpoint;
use debugger::Debugger;

struct App {
    debugger: Debugger,
    input_buffer: String,
}

impl App {
    fn new(debugger: Debugger) -> Self {
        Self {
            debugger,
            input_buffer: String::new(),
        }
    }

    fn loop_step(&mut self) -> eyre::Result<ShouldQuit> {
        tracing::trace!("locking stdout");
        print!("> ");
        tracing::trace!("prompt written, flushing stdout");
        std::io::stdout().flush()?;
        tracing::trace!("stdout flushed");

        tracing::trace!("reading from stdin");
        let n = std::io::stdin().read_line(&mut self.input_buffer)?;
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
            other => println!("Unhandled commmand: '{}'", other),
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
    debugger.wait_for_event(|event| matches!(event, debugger::Event::Initialised));
    for breakpoint in args.breakpoints {
        tracing::debug!(?breakpoint, "adding breakpoint");
        debugger
            .add_breakpoint(&breakpoint)
            .context("adding breakpoint")?;
    }
    tracing::debug!("breakpoints added");
    debugger.start().context("starting debugger")?;
    tracing::debug!("debugger started");

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
