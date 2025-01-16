use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;

use color_eyre::eyre::{self, Context};

struct App {
    stdin: BufReader<std::io::Stdin>,
    stdout: std::io::Stdout,
    input_buffer: String,
}

impl App {
    fn new() -> Self {
        Self {
            stdin: BufReader::new(std::io::stdin()),
            stdout: std::io::stdout(),
            input_buffer: String::new(),
        }
    }

    fn loop_step(&mut self) -> eyre::Result<ShouldQuit> {
        let mut stdout = self.stdout.lock();
        write!(&mut stdout, "> ")?;
        stdout.flush()?;

        let _n = self.stdin.read_line(&mut self.input_buffer)?;
        let input = self.input_buffer.trim().to_owned();

        let res = self.handle_input(&input).context("handling command");
        self.input_buffer.clear();
        res
    }

    fn handle_input(&mut self, input: &str) -> eyre::Result<ShouldQuit> {
        match input {
            "q" => return Ok(ShouldQuit::True),
            other => writeln!(self.stdout, "Unhandled commmand: '{}'", other)?,
        }
        Ok(ShouldQuit::False)
    }
}

fn main() -> eyre::Result<()> {
    color_eyre::install().context("installing color_eyre")?;
    tracing_subscriber::fmt::init();

    let mut app = App::new();
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
