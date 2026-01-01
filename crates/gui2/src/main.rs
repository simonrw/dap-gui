use color_eyre::eyre;
use gui2::debugger_app;

#[cfg(feature = "sentry")]
macro_rules! setup_sentry {
    () => {
        tracing::info!("setting up sentry for crash reporting");
        let _guard = sentry::init((
            "https://f08b65bc9944ecbb855f1ebb2cadcb92@o366030.ingest.sentry.io/4505663159926784",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ));
    };
}

#[cfg(not(feature = "sentry"))]
macro_rules! setup_sentry {
    () => {};
}

fn main() -> eyre::Result<()> {
    setup_sentry!();
    let _ = tracing_subscriber::fmt::try_init();
    let _ = color_eyre::install();

    // iced::run(app.update, app.view)?;
    iced::application(debugger_app::boot, debugger_app::update, debugger_app::view)
        .subscription(debugger_app::subscription)
        .title("DAP GUI")
        .run()?;
    Ok(())
}
