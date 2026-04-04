use std::path::PathBuf;

use debugger::Breakpoint;
use eyre::Context;
use launch_configuration::{Debugpy, LaunchConfiguration};

/// The result of connecting to a debugger.
///
/// Contains everything both frontends need to create their session structures.
pub struct ConnectedDebugger {
    pub debugger: debugger::TcpAsyncDebugger,
    pub runtime: tokio::runtime::Runtime,
    pub server: Option<Box<dyn server::Server + Send>>,
}

/// Create a tokio runtime, connect to the debugger based on the launch
/// configuration, configure initial breakpoints, and start execution.
///
/// This is the shared session bootstrap used by both the TUI and GUI frontends.
/// The `debug_root_dir` is updated in-place if the configuration specifies a
/// working directory.
pub fn connect_debugger(
    config: &LaunchConfiguration,
    breakpoints: &[Breakpoint],
    debug_root_dir: &mut PathBuf,
) -> eyre::Result<ConnectedDebugger> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .map_err(|e| eyre::eyre!("failed to create tokio runtime: {e}"))?;

    let mut server_handle: Option<Box<dyn server::Server + Send>> = None;

    let debugger = rt.block_on(async {
        match config {
            LaunchConfiguration::Python(debugpy) | LaunchConfiguration::Debugpy(debugpy) => {
                let Debugpy {
                    request,
                    cwd,
                    connect,
                    path_mappings,
                    program,
                    ..
                } = debugpy.clone();

                if let Some(dir) = cwd {
                    *debug_root_dir =
                        std::fs::canonicalize(debugger::utils::normalise_path(&dir).as_ref())
                            .unwrap_or_else(|_| debugger::utils::normalise_path(&dir).into_owned());
                }

                match request.as_str() {
                    "attach" => {
                        let attach_args = debugger::AttachArguments {
                            working_directory: debug_root_dir.to_owned(),
                            port: connect.as_ref().map(|c| c.port),
                            host: connect.map(|c| c.host),
                            language: debugger::Language::DebugPy,
                            path_mappings,
                            just_my_code: None,
                        };

                        let port = attach_args.port.unwrap_or(server::DEFAULT_DAP_PORT);
                        let debugger = debugger::TcpAsyncDebugger::connect(
                            port,
                            debugger::Language::DebugPy,
                            debugger::SessionArgs::Attach(attach_args),
                            debugger::StartMode::Staged,
                        )
                        .await
                        .context("creating async debugger (attach)")?;

                        Ok::<_, eyre::Report>(debugger)
                    }
                    "launch" => {
                        let Some(program) = program else {
                            eyre::bail!("'program' is a required setting");
                        };

                        let program = std::fs::canonicalize(&program).unwrap_or(program);

                        let port = server::DEFAULT_DAP_PORT;
                        server_handle = Some(
                            server::for_implementation_on_port(
                                server::Implementation::Debugpy,
                                port,
                            )
                            .context("creating background server process")?,
                        );

                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                        let launch_args = debugger::LaunchArguments {
                            program: Some(program),
                            module: None,
                            args: None,
                            env: None,
                            working_directory: Some(debug_root_dir.to_owned()),
                            language: debugger::Language::DebugPy,
                            just_my_code: debugpy.just_my_code,
                            stop_on_entry: debugpy.stop_on_entry,
                        };

                        let debugger = debugger::TcpAsyncDebugger::connect(
                            port,
                            debugger::Language::DebugPy,
                            debugger::SessionArgs::Launch(launch_args),
                            debugger::StartMode::Staged,
                        )
                        .await
                        .context("creating async debugger (launch)")?;

                        Ok(debugger)
                    }
                    other => eyre::bail!("unsupported request type: {other}"),
                }
            }
            other => eyre::bail!("unsupported configuration: {other:?}"),
        }
    })?;

    rt.block_on(async {
        debugger
            .configure_breakpoints(breakpoints)
            .await
            .context("configuring breakpoints")?;

        tracing::debug!("launching debugee");
        debugger.start().await.context("launching debugee")
    })?;

    Ok(ConnectedDebugger {
        debugger,
        runtime: rt,
        server: server_handle,
    })
}
