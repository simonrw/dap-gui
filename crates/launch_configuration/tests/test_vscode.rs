use std::collections::HashMap;

use launch_configuration::{ChosenLaunchConfiguration, LaunchConfiguration, PathMapping};

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

    assert_eq!(config.name, "Remote Attach (ext)");
    assert_eq!(config.request, "attach");
    // TODO: config.connect
    assert_eq!(
        config.path_mappings,
        Some(vec![
            PathMapping {
                local_root: "${workspaceFolder:localstack-ext}/localstack-pro-core/localstack/pro"
                    .to_string(),
                remote_root:
                    "/opt/code/localstack/.venv/lib/python3.11/site-packages/localstack/pro"
                        .to_string(),
            },
            PathMapping {
                local_root: "${workspaceFolder:localstack}/localstack-core/localstack".to_string(),
                remote_root: "/opt/code/localstack/.venv/lib/python3.11/site-packages/localstack"
                    .to_string()
            }
        ])
    );
    assert!(!config.just_my_code.unwrap());
}

#[test]
fn test_malformed_json() {
    let input = b"not valid json {{{" as &[u8];
    let result = launch_configuration::load(None, input);
    assert!(result.is_err());
}

#[test]
fn test_empty_input() {
    let input = b"" as &[u8];
    let result = launch_configuration::load(None, input);
    assert!(result.is_err());
}

#[test]
fn test_missing_configurations_field() {
    let input = br#"{"version": "0.2.0"}"# as &[u8];
    let result = launch_configuration::load(None, input);
    assert!(result.is_err());
}

#[test]
fn test_empty_configurations_array() {
    let input = br#"{"version": "0.2.0", "configurations": []}"# as &[u8];
    let result = launch_configuration::load(None, input);
    let config = result.unwrap();
    assert!(matches!(
        config,
        ChosenLaunchConfiguration::ToBeChosen(names) if names.is_empty()
    ));
}

#[test]
fn test_config_not_found() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Python: Current File",
                "type": "debugpy",
                "request": "launch",
                "program": "${file}"
            }
        ]
    }"# as &[u8];
    let result = launch_configuration::load(Some(&"Nonexistent Config".to_string()), input);
    let config = result.unwrap();
    assert!(matches!(config, ChosenLaunchConfiguration::NotFound));
}

#[test]
fn test_to_be_chosen_lists_names() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Config A",
                "type": "debugpy",
                "request": "launch",
                "program": "${file}"
            },
            {
                "name": "Config B",
                "type": "python",
                "request": "attach"
            }
        ]
    }"# as &[u8];
    let result = launch_configuration::load(None, input);
    let config = result.unwrap();
    match config {
        ChosenLaunchConfiguration::ToBeChosen(names) => {
            assert_eq!(names, vec!["Config A", "Config B"]);
        }
        _ => panic!("expected ToBeChosen"),
    }
}

#[test]
fn test_specific_config_by_name() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Config A",
                "type": "debugpy",
                "request": "launch",
                "program": "${file}"
            },
            {
                "name": "Config B",
                "type": "debugpy",
                "request": "attach"
            }
        ]
    }"# as &[u8];
    let result = launch_configuration::load(Some(&"Config B".to_string()), input);
    let config = result.unwrap();
    match config {
        ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(debugpy)) => {
            assert_eq!(debugpy.name, "Config B");
            assert_eq!(debugpy.request, "attach");
        }
        _ => panic!("expected Specific Debugpy"),
    }
}

#[test]
fn test_jsonc_with_comments() {
    let input = br#"{
        // This is a comment
        "version": "0.2.0",
        "configurations": [
            {
                "name": "With Comments",
                "type": "debugpy",
                "request": "launch",
                "program": "${file}" // trailing comment
            }
        ]
    }"# as &[u8];
    let result = launch_configuration::load(Some(&"With Comments".to_string()), input);
    let config = result.unwrap();
    assert!(matches!(
        config,
        ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(_))
    ));
}

#[test]
fn test_jsonc_with_trailing_commas() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Trailing Commas",
                "type": "debugpy",
                "request": "launch",
                "program": "${file}",
            },
        ],
    }"# as &[u8];
    let result = launch_configuration::load(Some(&"Trailing Commas".to_string()), input);
    let config = result.unwrap();
    assert!(matches!(
        config,
        ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(_))
    ));
}

#[test]
fn test_workspace_config_not_found() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let result =
        launch_configuration::load_from_path(Some(&"Nonexistent Config".to_string()), path)
            .unwrap();
    assert!(matches!(result, ChosenLaunchConfiguration::NotFound));
}

#[test]
fn test_workspace_to_be_chosen() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let result = launch_configuration::load_from_path(None, path).unwrap();
    match result {
        ChosenLaunchConfiguration::ToBeChosen(names) => {
            assert!(!names.is_empty());
        }
        _ => panic!("expected ToBeChosen for workspace without specified name"),
    }
}

#[test]
fn test_module_args_env_fields() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "Run with module",
                "type": "debugpy",
                "request": "launch",
                "module": "pytest",
                "args": ["tests/", "-v", "--tb=short"],
                "cwd": "/home/user/project",
                "env": {
                    "PYTHONPATH": "/home/user/project",
                    "DEBUG": "1"
                },
                "justMyCode": false,
                "stopOnEntry": true
            }
        ]
    }"# as &[u8];

    let result = launch_configuration::load(Some(&"Run with module".to_string()), input);
    let config = result.unwrap();
    match config {
        ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(debugpy)) => {
            assert_eq!(debugpy.name, "Run with module");
            assert_eq!(debugpy.request, "launch");
            assert!(debugpy.program.is_none());
            assert_eq!(debugpy.module, Some("pytest".to_string()));
            assert_eq!(
                debugpy.args,
                Some(vec![
                    "tests/".to_string(),
                    "-v".to_string(),
                    "--tb=short".to_string()
                ])
            );
            let expected_env: HashMap<String, String> = [
                ("PYTHONPATH".to_string(), "/home/user/project".to_string()),
                ("DEBUG".to_string(), "1".to_string()),
            ]
            .into_iter()
            .collect();
            assert_eq!(debugpy.env, Some(expected_env));
            assert_eq!(debugpy.just_my_code, Some(false));
            assert_eq!(debugpy.stop_on_entry, Some(true));
        }
        _ => panic!("expected Specific Debugpy"),
    }
}

#[test]
fn test_workspace_module_launch_config() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Run LocalStack (host mode)".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Run LocalStack (host mode)");
    assert_eq!(config.request, "launch");
    assert!(config.program.is_none());
    assert_eq!(config.module, Some("localstack.cli.main".to_string()));
    assert_eq!(
        config.args,
        Some(vec!["start".to_string(), "--host".to_string()])
    );
    assert!(config.env.is_some());
    let env = config.env.unwrap();
    assert_eq!(env.get("CONFIG_PROFILE"), Some(&"dev,test".to_string()));
    assert_eq!(config.just_my_code, Some(false));
}

#[test]
fn test_workspace_module_with_env_file() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Run community test".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Run community test");
    assert_eq!(config.module, Some("pytest".to_string()));
    assert!(config.env_file.is_some());
    assert!(config.args.is_some());
}

#[test]
fn test_env_file_parsing() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "With env file",
                "type": "debugpy",
                "request": "launch",
                "module": "myapp",
                "envFile": "/path/to/.env"
            }
        ]
    }"# as &[u8];

    let result = launch_configuration::load(Some(&"With env file".to_string()), input);
    let config = result.unwrap();
    match config {
        ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(debugpy)) => {
            assert_eq!(
                debugpy.env_file,
                Some(std::path::PathBuf::from("/path/to/.env"))
            );
        }
        _ => panic!("expected Specific Debugpy"),
    }
}
