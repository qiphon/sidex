//! Launch configuration parsing (`.vscode/launch.json` compatible).

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A debug launch configuration, matching the shape in `.vscode/launch.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub debug_type: String,
    /// Either "launch" or "attach".
    pub request: String,
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub console: Option<String>,
    #[serde(default, rename = "preLaunchTask")]
    pub pre_launch_task: Option<String>,
    #[serde(default, rename = "postDebugTask")]
    pub post_debug_task: Option<String>,
    /// Extra fields not covered above.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Configuration for attaching to a running process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachConfig {
    #[serde(default)]
    pub process_id: Option<i64>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Console type for launch configurations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleType {
    #[default]
    InternalConsole,
    IntegratedTerminal,
    ExternalTerminal,
}

/// A reusable launch configuration template for a specific debug type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchConfigTemplate {
    pub type_name: String,
    pub request: String,
    pub name: String,
    #[serde(default)]
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub console: ConsoleType,
    #[serde(default, rename = "preLaunchTask")]
    pub pre_launch_task: Option<String>,
    #[serde(default, rename = "postDebugTask")]
    pub post_debug_task: Option<String>,
}

/// A compound launch configuration — runs multiple debug sessions together.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompoundLaunchConfig {
    pub name: String,
    pub configurations: Vec<String>,
    #[serde(default)]
    pub stop_all: bool,
    #[serde(default, rename = "preLaunchTask")]
    pub pre_launch_task: Option<String>,
}

/// Returns built-in launch config templates for common debug types.
#[must_use]
pub fn builtin_templates() -> Vec<LaunchConfigTemplate> {
    vec![
        LaunchConfigTemplate {
            type_name: "node".into(),
            request: "launch".into(),
            name: "Launch Node.js".into(),
            program: "${workspaceFolder}/index.js".into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::IntegratedTerminal,
            pre_launch_task: None,
            post_debug_task: None,
        },
        LaunchConfigTemplate {
            type_name: "python".into(),
            request: "launch".into(),
            name: "Launch Python".into(),
            program: "${workspaceFolder}/main.py".into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::IntegratedTerminal,
            pre_launch_task: None,
            post_debug_task: None,
        },
        LaunchConfigTemplate {
            type_name: "lldb".into(),
            request: "launch".into(),
            name: "Launch Rust (LLDB)".into(),
            program: "${workspaceFolder}/target/debug/${workspaceFolderBasename}".into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::InternalConsole,
            pre_launch_task: Some("cargo build".into()),
            post_debug_task: None,
        },
        LaunchConfigTemplate {
            type_name: "go".into(),
            request: "launch".into(),
            name: "Launch Go".into(),
            program: "${workspaceFolder}".into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::IntegratedTerminal,
            pre_launch_task: None,
            post_debug_task: None,
        },
        LaunchConfigTemplate {
            type_name: "cppdbg".into(),
            request: "launch".into(),
            name: "Launch C/C++".into(),
            program: "${workspaceFolder}/build/a.out".into(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::InternalConsole,
            pre_launch_task: Some("cmake build".into()),
            post_debug_task: None,
        },
        LaunchConfigTemplate {
            type_name: "java".into(),
            request: "launch".into(),
            name: "Launch Java".into(),
            program: String::new(),
            args: Vec::new(),
            env: HashMap::new(),
            cwd: Some("${workspaceFolder}".into()),
            console: ConsoleType::IntegratedTerminal,
            pre_launch_task: None,
            post_debug_task: None,
        },
    ]
}

/// The top-level shape of a `.vscode/launch.json` file.
#[derive(Debug, Deserialize)]
struct LaunchJsonFile {
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    configurations: Vec<Value>,
    #[serde(default)]
    compounds: Vec<Value>,
}

/// Parses a `.vscode/launch.json` file and returns all launch configurations.
///
/// Handles JSONC (JSON with comments) by stripping single-line `//` comments
/// and trailing commas before parsing.
pub fn parse_launch_json(path: &Path) -> Result<(Vec<LaunchConfig>, Vec<CompoundLaunchConfig>)> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let cleaned = strip_jsonc_comments(&raw);

    let file: LaunchJsonFile = serde_json::from_str(&cleaned)
        .with_context(|| format!("failed to parse {}", path.display()))?;

    let _ = file.version;

    let mut configs = Vec::new();
    for val in file.configurations {
        match serde_json::from_value::<LaunchConfig>(val) {
            Ok(cfg) => configs.push(cfg),
            Err(e) => {
                log::warn!("skipping invalid launch configuration: {e}");
            }
        }
    }

    let mut compounds = Vec::new();
    for val in file.compounds {
        match serde_json::from_value::<CompoundLaunchConfig>(val) {
            Ok(c) => compounds.push(c),
            Err(e) => {
                log::warn!("skipping invalid compound configuration: {e}");
            }
        }
    }

    Ok((configs, compounds))
}

/// Strips single-line `//` comments and trailing commas from JSONC text.
fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if escape {
            out.push(ch);
            escape = false;
            continue;
        }

        if in_string {
            if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(ch);
            continue;
        }

        if ch == '"' {
            in_string = true;
            out.push(ch);
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            // consume until end of line
            for c in chars.by_ref() {
                if c == '\n' {
                    out.push('\n');
                    break;
                }
            }
            continue;
        }

        out.push(ch);
    }

    // Remove trailing commas before } or ]
    let mut result = String::with_capacity(out.len());
    let bytes = out.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b',' {
            let mut j = i + 1;
            while j < len
                && (bytes[j] == b' ' || bytes[j] == b'\t' || bytes[j] == b'\n' || bytes[j] == b'\r')
            {
                j += 1;
            }
            if j < len && (bytes[j] == b'}' || bytes[j] == b']') {
                i += 1;
                continue;
            }
        }
        result.push(bytes[i] as char);
        i += 1;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parse_basic_launch_json() {
        let json = r#"{
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Run",
                    "type": "node",
                    "request": "launch",
                    "program": "${workspaceFolder}/index.js",
                    "args": ["--verbose"],
                    "cwd": "${workspaceFolder}"
                },
                {
                    "name": "Attach",
                    "type": "node",
                    "request": "attach",
                    "port": 9229
                }
            ]
        }"#;

        let tmp = tempfile(json);
        let (configs, _) = parse_launch_json(tmp.path()).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].name, "Run");
        assert_eq!(configs[0].request, "launch");
        assert_eq!(
            configs[0].program.as_deref(),
            Some("${workspaceFolder}/index.js")
        );
        assert_eq!(configs[0].args, vec!["--verbose"]);
        assert_eq!(configs[1].name, "Attach");
        assert_eq!(configs[1].request, "attach");
    }

    #[test]
    fn parse_jsonc_with_comments() {
        let jsonc = r#"{
            // This is a comment
            "version": "0.2.0",
            "configurations": [
                {
                    "name": "Debug", // inline comment
                    "type": "python",
                    "request": "launch",
                    "program": "main.py",
                }
            ]
        }"#;

        let tmp = tempfile(jsonc);
        let (configs, _) = parse_launch_json(tmp.path()).unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].name, "Debug");
        assert_eq!(configs[0].debug_type, "python");
    }

    #[test]
    fn parse_with_env() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Env Test",
                    "type": "node",
                    "request": "launch",
                    "program": "app.js",
                    "env": {
                        "NODE_ENV": "development",
                        "PORT": "3000"
                    }
                }
            ]
        }"#;

        let tmp = tempfile(json);
        let (configs, _) = parse_launch_json(tmp.path()).unwrap();
        assert_eq!(configs[0].env.get("NODE_ENV").unwrap(), "development");
        assert_eq!(configs[0].env.get("PORT").unwrap(), "3000");
    }

    #[test]
    fn parse_with_tasks() {
        let json = r#"{
            "configurations": [
                {
                    "name": "Build & Run",
                    "type": "cppdbg",
                    "request": "launch",
                    "program": "./build/app",
                    "preLaunchTask": "cmake build",
                    "postDebugTask": "cleanup"
                }
            ]
        }"#;

        let tmp = tempfile(json);
        let (configs, _) = parse_launch_json(tmp.path()).unwrap();
        assert_eq!(configs[0].pre_launch_task.as_deref(), Some("cmake build"));
        assert_eq!(configs[0].post_debug_task.as_deref(), Some("cleanup"));
    }

    #[test]
    fn empty_configurations() {
        let json = r#"{"configurations": []}"#;
        let tmp = tempfile(json);
        let (configs, _) = parse_launch_json(tmp.path()).unwrap();
        assert!(configs.is_empty());
    }

    #[test]
    fn strip_comments_preserves_strings() {
        let input = r#"{"key": "value // not a comment"}"#;
        let output = strip_jsonc_comments(input);
        assert_eq!(output, input);
    }

    #[test]
    fn attach_config_roundtrip() {
        let config = AttachConfig {
            process_id: Some(1234),
            port: Some(9229),
            host: Some("localhost".to_owned()),
            extra: HashMap::new(),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AttachConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.process_id, Some(1234));
        assert_eq!(back.port, Some(9229));
    }

    struct TempFile {
        path: std::path::PathBuf,
    }

    impl TempFile {
        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    fn tempfile(contents: &str) -> TempFile {
        let path = std::env::temp_dir().join(format!(
            "sidex_dap_test_{}.json",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        TempFile { path }
    }

    #[test]
    fn console_type_default() {
        assert_eq!(ConsoleType::default(), ConsoleType::InternalConsole);
    }

    #[test]
    fn console_type_roundtrip() {
        for ct in [
            ConsoleType::InternalConsole,
            ConsoleType::IntegratedTerminal,
            ConsoleType::ExternalTerminal,
        ] {
            let json = serde_json::to_string(&ct).unwrap();
            let back: ConsoleType = serde_json::from_str(&json).unwrap();
            assert_eq!(back, ct);
        }
    }

    #[test]
    fn launch_config_template_roundtrip() {
        let tpl = LaunchConfigTemplate {
            type_name: "node".into(),
            request: "launch".into(),
            name: "Test".into(),
            program: "index.js".into(),
            args: vec!["--port".into(), "3000".into()],
            env: HashMap::from([("NODE_ENV".into(), "dev".into())]),
            cwd: Some("/app".into()),
            console: ConsoleType::IntegratedTerminal,
            pre_launch_task: Some("build".into()),
            post_debug_task: None,
        };
        let json = serde_json::to_string(&tpl).unwrap();
        let back: LaunchConfigTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(back.type_name, "node");
        assert_eq!(back.console, ConsoleType::IntegratedTerminal);
        assert_eq!(back.args.len(), 2);
    }

    #[test]
    fn builtin_templates_cover_all_types() {
        let templates = builtin_templates();
        assert!(templates.len() >= 6);
        let types: Vec<&str> = templates.iter().map(|t| t.type_name.as_str()).collect();
        assert!(types.contains(&"node"));
        assert!(types.contains(&"python"));
        assert!(types.contains(&"lldb"));
        assert!(types.contains(&"go"));
        assert!(types.contains(&"cppdbg"));
        assert!(types.contains(&"java"));
    }

    #[test]
    fn compound_config_roundtrip() {
        let c = CompoundLaunchConfig {
            name: "Full Stack".into(),
            configurations: vec!["Server".into(), "Client".into()],
            stop_all: true,
            pre_launch_task: Some("build all".into()),
        };
        let json = serde_json::to_string(&c).unwrap();
        let back: CompoundLaunchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.configurations.len(), 2);
        assert!(back.stop_all);
    }

    #[test]
    fn parse_compound_launch_json() {
        let json = r#"{
            "version": "0.2.0",
            "configurations": [
                { "name": "Server", "type": "node", "request": "launch", "program": "server.js" },
                { "name": "Client", "type": "node", "request": "launch", "program": "client.js" }
            ],
            "compounds": [
                { "name": "Full Stack", "configurations": ["Server", "Client"], "stopAll": true }
            ]
        }"#;
        let tmp = tempfile(json);
        let (configs, compounds) = parse_launch_json(tmp.path()).unwrap();
        assert_eq!(configs.len(), 2);
        assert_eq!(compounds.len(), 1);
        assert_eq!(compounds[0].name, "Full Stack");
        assert!(compounds[0].stop_all);
    }
}
