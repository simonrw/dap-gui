use launch_configuration::LaunchConfiguration;

#[test]
fn test_read_example() {
    let path = "./testdata/vscode/localstack-ext.json";
    let LaunchConfiguration::Debugpy(config) =
        launch_configuration::load_from_path("Python: Remote Attach", path)
            .unwrap()
            .unwrap();

    assert_eq!(config.name, "Python: Remote Attach");
    assert_eq!(config.request, "attach");
}

#[ctor::ctor]
fn init() {
    // let in_ci = std::env::var("CI")
    //     .map(|val| val == "true")
    //     .unwrap_or(false);

    // if std::io::stderr().is_terminal() || in_ci {
    //     let _ = tracing_subscriber::fmt()
    //         .with_env_filter(EnvFilter::from_default_env())
    //         .try_init();
    // } else {
    //     let _ = tracing_subscriber::fmt()
    //         .with_env_filter(EnvFilter::from_default_env())
    //         .json()
    //         .try_init();
    // }

    let _ = color_eyre::install();
}
