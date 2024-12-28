use launch_configuration::{ChosenLaunchConfiguration, LaunchConfiguration};

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

#[test]
fn test_read_example() {
    let path = "./testdata/vscode/localstack-ext.json";
    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Python: Remote Attach".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Python: Remote Attach");
    assert_eq!(config.request, "attach");
}

#[test]
fn test_read_code_workspace() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Remote Attach (ext)".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Python: Remote Attach");
    assert_eq!(config.request, "attach");
    // TODO: config.connect
    // assert_eq!(config.path_mappings, vec![]);
    //assert!(!config.just_my_code.unwrap());
}
