use std::sync::Mutex;
use std::thread::JoinHandle;
use std::{io::Write, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{self, Context};
use crossbeam_channel::Receiver;
use debugger::Breakpoint;
use debugger::Debugger;
use debugger::ProgramDescription;
use tracing_subscriber::filter::EnvFilter;

struct App {
    debugger: Debugger,
    program_description: Option<ProgramDescription>,
    debugger_events: Receiver<debugger::Event>,
    input_rx: Receiver<String>,

    #[allow(dead_code)]
    input_thread: JoinHandle<String>,
}

impl App {
    fn new(debugger: Debugger) -> Self {
        let debugger_events = debugger.events();

        // handle input
        let (input_tx, input_rx) = crossbeam_channel::unbounded();
        let input_thread = std::thread::spawn(move || {
            let mut buffer = String::new();
            loop {
                let n = std::io::stdin()
                    .read_line(&mut buffer)
                    .expect("reading from stdin");
                if n != 0 {
                    let input = buffer.trim().to_owned();
                    let _ = input_tx.send(input);
                    buffer.clear();
                }
            }
        });

        Self {
            debugger,
            program_description: None,
            debugger_events,
            input_thread,
            input_rx,
        }
    }

    fn loop_step(&mut self) -> eyre::Result<ShouldQuit> {
        tracing::trace!("locking stdout");
        print!("> ");
        tracing::trace!("prompt written, flushing stdout");
        std::io::stdout().flush()?;
        tracing::trace!("stdout flushed");

        crossbeam_channel::select! {
            recv(self.input_rx) -> input =>
                self.handle_input(&input.expect("recv error")).context("handling command"),
            recv(self.debugger_events) -> event => if let Ok(event) = event {
                self.handle_debugger_event(event).context("handling debugger event")
            } else {
                Ok(ShouldQuit::False)
            },
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_debugger_event(&mut self, event: debugger::Event) -> eyre::Result<ShouldQuit> {
        match event {
            debugger::Event::Uninitialised => todo!(),
            debugger::Event::Initialised => todo!(),
            debugger::Event::Paused(program_description) => {
                println!(
                    "program paused at {}:{}",
                    &program_description
                        .paused_frame
                        .frame
                        .source
                        .as_ref()
                        .unwrap()
                        .path
                        .as_ref()
                        .unwrap()
                        .display(),
                    program_description.paused_frame.frame.line
                );
                self.program_description = Some(program_description);
            }
            debugger::Event::ScopeChange(_program_description) => todo!(),
            debugger::Event::Running => {
                println!("program running");
                self.program_description = None;
            }
            debugger::Event::Ended => {
                println!("program completed");
                return Ok(ShouldQuit::True);
            }
        }
        Ok(ShouldQuit::False)
    }

    fn handle_input(&mut self, input: &str) -> eyre::Result<ShouldQuit> {
        match input {
            "q" => return Ok(ShouldQuit::True),
            "w" => {
                if let Some(description) = &self.program_description {
                    println!(
                        "{}:{}",
                        &description
                            .paused_frame
                            .frame
                            .source
                            .as_ref()
                            .unwrap()
                            .path
                            .as_ref()
                            .unwrap()
                            .display(),
                        description.paused_frame.frame.line
                    );
                } else {
                    println!("???");
                }
            }
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
    let log_file = std::fs::File::create("log.log")?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(Mutex::new(log_file))
        .init();

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
