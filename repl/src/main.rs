use std::sync::Mutex;
use std::thread::JoinHandle;
use std::{io::Write, path::PathBuf};

use clap::Parser;
use color_eyre::eyre::{self, Context};
use crossbeam_channel::Receiver;
use debugger::{Breakpoint, Debugger, ProgramState};
use tracing_subscriber::filter::EnvFilter;
use transport::types::Variable;

// TODO: this would be better async
fn print_var(debugger: &mut Debugger, v: Variable) -> eyre::Result<()> {
    let span = tracing::debug_span!("print_var", name = %v.name);
    let _guard = span.enter();

    // TODO: presentation hint
    if v.variables_reference == 0 {
        tracing::debug!(name = ?v.name, "got leaf variable");
        println!(". {} = {}", v.name, v.value);
    } else {
        tracing::debug!(vref = %v.variables_reference, "recursing into variable");
        let vs = debugger.variables(v.variables_reference)?;
        for vv in vs {
            tracing::debug!(?vv, "recursing");
            print_var(debugger, vv.clone())?;
        }
    }
    Ok(())
}

struct App {
    debugger: Debugger,
    program_description: Option<ProgramState>,
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
                    for frame in description.stack.iter() {
                        println!(
                            "{}:{}",
                            frame
                                .source
                                .as_ref()
                                .unwrap()
                                .path
                                .as_ref()
                                .unwrap()
                                .display(),
                            frame.line
                        );
                    }
                } else {
                    println!("???");
                }
            }
            "c" => {
                tracing::debug!("executing continue command");
                self.debugger.r#continue().context("resuming execution")?;
            }
            "v" => {
                if let Some(ProgramState { paused_frame, .. }) = &self.program_description {
                    tracing::debug!("printing variable names in scope");
                    for var in &paused_frame.variables {
                        tracing::debug!(?var, "printing variable recursively");
                        print_var(&mut self.debugger, var.clone()).context("printing variable")?;
                    }
                } else {
                    println!("???");
                }
            }
            "o" => {
                tracing::debug!("stepping out");
                self.debugger.step_out().context("stepping out")?;
            }
            "i" => {
                tracing::debug!("stepping in");
                self.debugger.step_in().context("stepping in")?;
            }
            input if input.starts_with("p ") => {
                let var_name = input.trim_start_matches("p ").trim();
                tracing::debug!("printing variable {}", var_name);
                if let Some(ProgramState { paused_frame, .. }) = &self.program_description {
                    if let Some(var) = paused_frame.variables.iter().find(|v| v.name == var_name) {
                        println!(". {} = {}", var.name, var.value);
                    } else {
                        println!("Variable '{}' not found in current scope", var_name);
                    }
                } else {
                    println!("???");
                }
            }
            "?" => {
                println!(". Commands:");
                println!(". q - quit");
                println!(". w - where");
                println!(". c - continue");
                println!(". v - variables");
                println!(". o - step out");
                println!(". i - step in");
                println!(". p <name> - print variable value");
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
    println!(". enter '?' for help");
    loop {
        match app.loop_step() {
            Ok(ShouldQuit::True) => break,
            Ok(ShouldQuit::False) => {}
            Err(e) => eyre::bail!("Error running command: {e:?}"),
        }
    }

    Ok(())
}

enum ShouldQuit {
    True,
    False,
}
