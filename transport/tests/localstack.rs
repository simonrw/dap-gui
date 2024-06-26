/*
use std::{io::IsTerminal, net::TcpStream, path::PathBuf, process::Stdio, thread, time::Duration};

use eyre::{WrapErr, Result};
use tracing_subscriber::EnvFilter;

use transport::{
    bindings::get_random_tcp_port,
    requests::{self, Attach, ConnectInfo, Initialize, PathFormat, PathMapping},
    responses, Received,
};

// test suite "constructor"
#[ctor::ctor]
fn init() {
    let in_ci = std::env::var("CI")
        .map(|val| val == "true")
        .unwrap_or(false);

    if std::io::stderr().is_terminal() || in_ci {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .try_init();
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .json()
            .try_init();
    }

    // error traces
    let _ = color_eyre::install();
}

#[test]
#[ignore]
fn localstack() -> Result<()> {
    init_test_logger();

    let (tx, rx) = spmc::channel();
    with_launch_localstack(|edge_port, debug_port| {
        let span = tracing::debug_span!("with_launch_localstack", %edge_port, %debug_port);
        let _guard = span.enter();

        let stream = TcpStream::connect(format!("127.0.0.1:{debug_port}")).unwrap();
        let client = transport::Client::new(stream, tx).unwrap();

        // initialize
        let req = requests::RequestBody::Initialize(Initialize {
            adapter_id: "dap gui".to_string(),
            lines_start_at_one: false,
            path_format: PathFormat::Path,
            supports_start_debugging_request: true,
            supports_variable_type: true,
            supports_variable_paging: true,
            supports_progress_reporting: true,
            supports_memory_event: true,
        });
        client.send(req).unwrap();

        let _ = wait_for_response(&rx, |r| matches!(r, responses::ResponseBody::Initialize(_)))
            .context("waiting for initialize response")?;

        // attach
        let req = requests::RequestBody::Attach(Attach {
            connect: ConnectInfo {
                host: "localhost".to_string(),
                port: 5678,
            },
            path_mappings: vec![PathMapping {
                local_root: "/home/simon/work/localstack/localstack-ext/localstack_ext".to_string(),
                remote_root:
                    "/opt/code/localstack/.venv/lib/python3.10/site-packages/localstack_ext"
                        .to_string(),
            }],
            just_my_code: false,
            workspace_folder: PathBuf::from("/home/simon/work/localstack/localstack-ext"),
        });
        client.send(req).unwrap();

        /*
        // wait for initialized event
        let _waiting_for_server_event = wait_for_event(&rx, |e| {
            tracing::debug!(event = ?e, "got event");
            if let events::Event::Unknown(s) = e {
                if s == "debugpyWaitingForServer" {
                    return true;
                }
            }
            return false;
        });
        */

        Ok(())
    })
}

type ContainerId = String;

struct DockerClient {}

impl DockerClient {
    fn create(&self, edge_port: u16, debug_port: u16) -> Result<ContainerId> {
        let output = std::process::Command::new("docker")
            .args([
                "create",
                "-p",
                &format!("127.0.0.1:{edge_port}:4566"),
                "-p",
                &format!("127.0.0.1:{debug_port}:5678"),
                "-v",
                "/var/run/docker.sock:/var/run/docker.sock",
                "-e",
                "DEVELOP=1",
                "-e",
                "WAIT_FOR_DEBUGGER=1",
                "localstack/localstack",
            ])
            .output()
            .context("waiting for docker create command to finish")?;

        if !output.status.success() {
            eyre::bail!("bad exit code from docker command");
        }

        let output = String::from_utf8(output.stdout).context("invalid utf8 output")?;
        Ok(output.trim().to_string())
    }
    fn start(&self, id: &ContainerId) -> Result<()> {
        let exit_code = std::process::Command::new("docker")
            .args(["start", id])
            .stdout(Stdio::piped())
            .spawn()
            .context("spawning docker command")?
            .wait()
            .context("waiting for docker start command")?;
        if !exit_code.success() {
            eyre::bail!("bad exit code from docker command");
        }

        Ok(())
    }
    fn stop(&self, id: ContainerId) -> Result<()> {
        let exit_code = std::process::Command::new("docker")
            .args(["rm", "-f", &id])
            .stdout(Stdio::piped())
            .spawn()
            .context("spawning docker command")?
            .wait()
            .context("waiting for docker start command")?;
        if !exit_code.success() {
            eyre::bail!("bad exit code from docker command");
        }
        Ok(())
    }

    fn logs(&self, container_id: &str) -> Result<Vec<String>> {
        let output = std::process::Command::new("docker")
            .args(["logs", container_id])
            .output()
            .context("waiting for docker create command to finish")?;

        if !output.status.success() {
            eyre::bail!("bad exit code from docker command");
        }

        let output = String::from_utf8(output.stdout).context("invalid utf8 output")?;
        Ok(output.split('\n').map(ToString::to_string).collect())
    }
}

fn with_launch_localstack<F>(f: F) -> Result<()>
where
    F: FnOnce(u16, u16) -> Result<()>,
{
    let edge_port = get_random_tcp_port().context("finding random tcp port")?;
    let debug_port = get_random_tcp_port().context("finding random tcp port")?;

    let client = DockerClient {};
    let container_id = client
        .create(edge_port, debug_port)
        .context("creating localstack container")?;
    client
        .start(&container_id)
        .context("starting LocalStack container")?;

    'outer: loop {
        match client.logs(&container_id) {
            Ok(logs) => {
                for line in dbg!(logs) {
                    if line.contains("Starting debug server") {
                        break 'outer;
                    }
                }
            }
            Err(e) => {
                client
                    .stop(container_id)
                    .context("stopping LocalStack container")?;
                return Err(e);
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    let res = f(edge_port, debug_port);

    client
        .stop(container_id)
        .context("stopping LocalStack container")?;

    res
}

fn wait_for_response<F>(rx: &spmc::Receiver<Received>, pred: F) -> Result<responses::ResponseBody>
where
    F: Fn(&responses::ResponseBody) -> bool,
{
    tracing::debug!("waiting for response");
    let mut n = 0;
    loop {
        let msg = rx.recv().unwrap();
        if n >= 100 {
            eyre::bail!("did not receive message");
        }
        if let Received::Response(_, response) = msg {
            assert!(response.success);
            if let Some(body) = response.body {
                if pred(&body) {
                    tracing::debug!(response = ?body, "received expected response");
                    return Ok(body);
                }
            }
        }
        n += 1;
    }
}

fn wait_for_event<F>(rx: &mpsc::Receiver<Received>, pred: F) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    tracing::debug!("waiting for event");
    let mut n = 0;
    for msg in rx {
        if n >= 10 {
            panic!("did not receive event");
        }

        if let Received::Event(evt) = msg {
            if pred(&evt) {
                tracing::debug!(event = ?evt, "received expected event");
                return evt;
            }
        } else {
            tracing::warn!(message = ?msg, "unhandled message");
        }

        n += 1;
    }

    unreachable!()
}
*/
