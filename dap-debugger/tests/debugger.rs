use anyhow::Result;
use dap_debugger::Debugger;
use std::{
    net::TcpListener,
    process::{Child, Command},
    thread,
};

// run a background server on a random port, and return the port used
struct Server {
    port: u16,
    child: Child,
}

impl Server {
    fn kill(mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
    }
}

fn get_random_tcp_port() -> Result<u16> {
    for _ in 0..10 {
        if let Ok(port) = TcpListener::bind("127.0.0.1:0")
            .and_then(|s| s.local_addr())
            .map(|a| a.port())
        {
            return Ok(port);
        }
    }
    Err(anyhow::anyhow!("could not bind a free port"))
}

fn run_server() -> Result<Server> {
    let port = get_random_tcp_port()?;
    let child = Command::new("make")
        .env("PORT", format!("{port}"))
        .args(["-C", "..", "run-server"])
        .spawn()?;
    Ok(Server { port, child })
}

#[test]
fn end_to_end() {
    // Test setting up a debugger, setting up a launch configuration, starting execution, hitting a
    // breakpoint, and continuing.
    // launch the background server
    let server = run_server().expect("running background server");

    thread::scope(|s| {
        let mut messages = Vec::new();
        let callback = move |reply| {
            messages.push(reply);
        };

        let _debugger = Debugger::new(s, format!("127.0.0.1:{}", server.port), callback);
    });

    server.kill().unwrap();
}
