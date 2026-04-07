use std::collections::HashMap;

use launch_configuration::{ChosenLaunchConfiguration, LaunchConfiguration, PathMapping};

#[ctor::ctor]
fn init() {
    let _ = color_eyre::install();
}

// ---------------------------------------------------------------------------
// Helper: resolve the testdata directory to an absolute path
// ---------------------------------------------------------------------------

fn testdata_dir() -> std::path::PathBuf {
    std::fs::canonicalize("./testdata/vscode").expect("testdata/vscode should exist")
}

// ===========================================================================
// Existing parsing tests (reader-based — no variable resolution)
// ===========================================================================

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

// ===========================================================================
// Path-based loading — variable resolution
// ===========================================================================

#[test]
fn test_read_example_resolves_workspace_folder() {
    let path = "./testdata/vscode/localstack-ext.json";
    let workspace_root = testdata_dir().parent().unwrap().to_path_buf();

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Python: Remote Attach".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Python: Remote Attach");
    assert_eq!(config.request, "attach");
    assert_eq!(
        config.path_mappings,
        Some(vec![PathMapping {
            local_root: format!("{}/localstack_ext", workspace_root.display()),
            remote_root: "/opt/code/localstack/.venv/lib/python3.11/site-packages/localstack_ext"
                .to_string(),
        }])
    );
}

#[test]
fn test_workspace_named_folder_resolution_in_path_mappings() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let workspace_dir = testdata_dir();

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Remote Attach (ext)".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Remote Attach (ext)");
    assert_eq!(config.request, "attach");
    assert_eq!(
        config.path_mappings,
        Some(vec![
            PathMapping {
                local_root: format!(
                    "{}/localstack-ext/localstack-pro-core/localstack/pro",
                    workspace_dir.display()
                ),
                remote_root:
                    "/opt/code/localstack/.venv/lib/python3.11/site-packages/localstack/pro"
                        .to_string(),
            },
            PathMapping {
                local_root: format!(
                    "{}/localstack/localstack-core/localstack",
                    workspace_dir.display()
                ),
                remote_root: "/opt/code/localstack/.venv/lib/python3.11/site-packages/localstack"
                    .to_string()
            }
        ])
    );
    assert!(!config.just_my_code.unwrap());
}

#[test]
fn test_workspace_cwd_resolved() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let workspace_dir = testdata_dir();

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Run LocalStack (host mode)".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Run LocalStack (host mode)");
    assert_eq!(config.request, "launch");
    assert_eq!(
        config.cwd,
        Some(workspace_dir.join("localstack")),
        "cwd should be resolved to absolute path"
    );
}

#[test]
fn test_workspace_env_values_resolved() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let workspace_dir = testdata_dir();

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Run LocalStack (host mode)".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    let env = config.env.unwrap();
    assert_eq!(env.get("CONFIG_PROFILE"), Some(&"dev,test".to_string()));
    assert_eq!(
        env.get("PYTHONPATH"),
        Some(&format!("{}/localstack", workspace_dir.display())),
        "env PYTHONPATH should be resolved"
    );
}

#[test]
fn test_workspace_env_file_resolved() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let workspace_dir = testdata_dir();

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Run community test".to_string()), path)
            .unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(config.name, "Run community test");
    assert_eq!(
        config.env_file,
        Some(workspace_dir.join("localstack").join(".env")),
        "envFile should be resolved to absolute path"
    );
    assert_eq!(
        config.cwd,
        Some(workspace_dir.join("localstack")),
        "cwd should be resolved to absolute path"
    );
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

    assert_eq!(config.request, "launch");
    assert!(config.program.is_none());
    assert_eq!(config.module, Some("localstack.cli.main".to_string()));
    assert_eq!(
        config.args,
        Some(vec!["start".to_string(), "--host".to_string()])
    );
    assert_eq!(config.just_my_code, Some(false));
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

// ===========================================================================
// launch.json ${workspaceFolder} resolution
// ===========================================================================

#[test]
fn test_launch_json_workspace_folder_in_program() {
    let path = "./testdata/vscode/sample-project/.vscode/launch.json";
    let workspace_root =
        std::fs::canonicalize("./testdata/vscode/sample-project").expect("testdata should exist");

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"Launch App".to_string()), path).unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    assert_eq!(
        config.program,
        Some(workspace_root.join("main.py")),
        "program should be resolved"
    );
    assert_eq!(
        config.cwd,
        Some(workspace_root.clone()),
        "cwd should be resolved"
    );
    let env = config.env.unwrap();
    assert_eq!(
        env.get("PYTHONPATH"),
        Some(&format!("{}/src", workspace_root.display())),
        "env PYTHONPATH should be resolved"
    );
}

// ===========================================================================
// ${env:VARNAME} resolution
// ===========================================================================

#[test]
fn test_env_var_resolution() {
    // SAFETY: this test is not run in parallel with others that use this var
    unsafe {
        std::env::set_var("DAP_TEST_CUSTOM_VAR", "hello_from_env");
    }

    let path = "./testdata/vscode/sample-project/.vscode/launch.json";

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load_from_path(Some(&"With Env Var".to_string()), path).unwrap()
    else {
        panic!("specified launch configuration not found");
    };

    let env = config.env.unwrap();
    assert_eq!(
        env.get("CUSTOM"),
        Some(&"hello_from_env".to_string()),
        "${{env:DAP_TEST_CUSTOM_VAR}} should be resolved"
    );

    // Clean up
    unsafe {
        std::env::remove_var("DAP_TEST_CUSTOM_VAR");
    }
}

// ===========================================================================
// Edge cases
// ===========================================================================

#[test]
fn test_command_variable_left_as_is() {
    let input = br#"{
        "version": "0.2.0",
        "configurations": [
            {
                "name": "With command",
                "type": "debugpy",
                "request": "launch",
                "program": "${command:python.interpreterPath}"
            }
        ]
    }"# as &[u8];

    let ChosenLaunchConfiguration::Specific(LaunchConfiguration::Debugpy(config)) =
        launch_configuration::load(Some(&"With command".to_string()), input).unwrap()
    else {
        panic!("expected Debugpy");
    };

    assert_eq!(
        config.program,
        Some(std::path::PathBuf::from(
            "${command:python.interpreterPath}"
        )),
        "unsupported variables should be left as-is"
    );
}

#[test]
fn test_load_all_from_workspace() {
    let path = "./testdata/vscode/localstack.code-workspace";
    let configs = launch_configuration::load_all_from_path(path).unwrap();
    assert_eq!(configs.len(), 6);

    // All configs should have resolved paths (no raw ${workspaceFolder:...})
    for config in &configs {
        if let LaunchConfiguration::Debugpy(d) = config
            && let Some(ref cwd) = d.cwd
        {
            assert!(
                !cwd.display().to_string().contains("${workspaceFolder"),
                "cwd should be resolved: {}",
                cwd.display()
            );
        }
    }
}

#[test]
fn test_load_all_from_launch_json() {
    let path = "./testdata/vscode/sample-project/.vscode/launch.json";
    let configs = launch_configuration::load_all_from_path(path).unwrap();
    assert_eq!(configs.len(), 2);

    for config in &configs {
        if let LaunchConfiguration::Debugpy(d) = config
            && let Some(ref cwd) = d.cwd
        {
            assert!(
                !cwd.display().to_string().contains("${workspaceFolder}"),
                "cwd should be resolved: {}",
                cwd.display()
            );
        }
    }
}
