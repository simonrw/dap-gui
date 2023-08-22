use anyhow::Result;
use dap_debugger::{types, Debugger};
use std::{
    net::TcpListener,
    process::{Child, Command},
    thread, time::Duration,
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
    let port_env = format!("PORT={port}");
    let child = Command::new("make")
        .args(["-C", "..", "run-server", &port_env])
        .spawn()?;
    // wait for server to start
    thread::sleep(Duration::from_secs(1));
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

        let mut debugger = Debugger::new(s, format!("127.0.0.1:{}", server.port), callback).unwrap();
        debugger
            .set_function_breakpoint(types::FunctionBreakpoint {
                name: "main".to_string(),
            })
            .unwrap();
        debugger.launch().unwrap();
        // blocks until breakpoint hit
        debugger.continue_execution().unwrap();
    });

    server.kill().unwrap();
}
