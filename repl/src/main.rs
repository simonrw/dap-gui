use color_eyre::eyre::{self, Context};

fn main() -> eyre::Result<()> {
    color_eyre::install().context("installing color_eyre")?;
    tracing_subscriber::fmt::init();
    Ok(())
}
