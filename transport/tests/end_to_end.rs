use std::{path::PathBuf, time::Duration};

use dap_codec::dap::{
    events,
    requests::{self, Command, PathFormat},
    types,
};
use eyre::WrapErr;
use tokio::{net::TcpListener, sync::mpsc};

use server::for_implementation_on_port;
use tracing_subscriber::EnvFilter;
use transport::{handle_messages, Client, ClientMessage};

async fn get_random_tcp_port() -> eyre::Result<u16> {
    for _ in 0..50 {
        match TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => {
                let addr = listener.local_addr().unwrap();
                let port = addr.port();
                return Ok(port);
            }
            Err(e) => {
                tracing::warn!(%e, "binding");
            }
        }
    }

    eyre::bail!("could not get free port");
}

#[tokio::test]
async fn test_loop() -> eyre::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let port = get_random_tcp_port().await.wrap_err("getting free port")?;
    let _server = for_implementation_on_port(server::Implementation::Debugpy, port)
        .wrap_err("creating server process")?;

    let stream = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
        .await
        .wrap_err("connecting to server")?;

    // events channel
    let (tx, mut events_rx) = mpsc::channel(10);

    let client = Client::new(stream, tx);

    // initialise
    let res = client
        .send(Command::Initialize(requests::InitializeArguments {
            adapter_id: "test".to_string(),
            ..Default::default()
        }))
        .await;
    assert!(res.success);

    // launch
    /*
    {
      "seq": 5,
      "type": "request",
      "command": "launch",
      "arguments": {
        "name": "Python: Current File",
        "type": "python",
        "request": "launch",
        "program": "/home/simon/dev/dap-gui/test.py",
        "console": "integratedTerminal",
        "justMyCode": false,
        "port": 5678,
        "__configurationTarget": 6,
        "workspaceFolder": "/home/simon/dev/dap-gui",
        "python": [
          "/nix/store/s6y73vxcr11h74lrvicqqwv971m48s2d-python3-3.10.12-env/bin/python"
        ],
        "debugAdapterPython": "/nix/store/s6y73vxcr11h74lrvicqqwv971m48s2d-python3-3.10.12-env/bin/python",
        "debugLauncherPython": "/nix/store/s6y73vxcr11h74lrvicqqwv971m48s2d-python3-3.10.12-env/bin/python",
        "clientOS": "unix",
        "cwd": "/home/simon/dev/dap-gui",
        "envFile": "/home/simon/dev/dap-gui/.env",
        "env": {
          "PYTHONIOENCODING": "UTF-8",
          "PYTHONUNBUFFERED": "1"
        },
        "stopOnEntry": false,
        "showReturnValue": true,
        "internalConsoleOptions": "neverOpen",
        "debugOptions": [
          "DebugStdLib",
          "ShowReturnValue"
        ],
        "__sessionId": "b524bb23-79fb-4582-ac9e-409a1105c980",
        "pythonArgs": [],
        "processName": "/home/simon/dev/dap-gui/test.py",
        "isOutputRedirected": false
      }
    }
     */
    client
        .execute(Command::Launch(requests::LaunchRequestArguments {
            additional_data: Some(serde_json::json!({
                "name": "Python: Current File",
                "type": "python",
                "request": "launch",
                "program": PathBuf::from("./test.py"),
                "justMyCode": false,
                "cwd": std::env::current_dir().unwrap().join(".."),
                "showReturnValue": true,
            })),
            ..Default::default()
        }))
        .await;

    let res = wait_for_event(&mut events_rx, Duration::from_secs(2), |e| {
        matches!(e, events::Event::Initialized { .. })
    })
    .await;
    dbg!(&res);

    let res = client
        .send(Command::SetFunctionBreakpoints(
            requests::SetFunctionBreakpointsArguments {
                breakpoints: vec![types::FunctionBreakpoint {
                    name: "main".to_string(),
                    ..Default::default()
                }],
            },
        ))
        .await;
    assert!(res.success);
    // let req = requests::RequestBody::SetFunctionBreakpoints(requests::SetFunctionBreakpoints {
    //     breakpoints: vec![requests::Breakpoint {
    //         name: "main".to_string(),
    //     }],
    // });
    // let _ = client.send(req).unwrap();

    // configuration done
    // let res = client.send(Command::ConfigurationDone).await;
    // dbg!(&res);
    // assert!(res.success);

    Ok(())
}

async fn wait_for_event<F>(
    rx: &mut mpsc::Receiver<events::Event>,
    timeout: Duration,
    pred: F,
) -> events::Event
where
    F: Fn(&events::Event) -> bool,
{
    let poll_fut = async {
        let mut n = 0;
        loop {
            tokio::select! {
                evt = rx.recv() => {
                    let evt = evt.unwrap();
                    dbg!(&evt);
                    if n >= 100 {
                        panic!("did not receive event");
                    }

                    if pred(&evt) {
                        return evt;
                    }
                    n += 1;
                }
            }
        }
    };

    tokio::time::timeout(timeout, poll_fut)
        .await
        .expect("didn't receive message")
}
