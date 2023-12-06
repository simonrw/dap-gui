use anyhow::Context;
use transport::DEFAULT_DAP_PORT;

pub mod debugpy;

pub enum Implementation {
    Debugpy,
}

pub trait Server {
    fn on_port(port: impl Into<u16>) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn new() -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        Self::on_port(DEFAULT_DAP_PORT)
    }
}

pub fn for_implementation(implementation: Implementation) -> anyhow::Result<Box<dyn Server>> {
    for_implementation_on_port(implementation, DEFAULT_DAP_PORT)
}

pub fn for_implementation_on_port(
    implementation: Implementation,
    port: impl Into<u16>,
) -> anyhow::Result<Box<dyn Server>> {
    match implementation {
        Implementation::Debugpy => {
            let server = crate::debugpy::DebugpyServer::on_port(port).context("creating server")?;
            Ok(Box::new(server))
        }
    }
}
