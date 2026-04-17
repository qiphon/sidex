//! CLI — extended command-line interface for `SideX`, matching VS Code's CLI.
//!
//! Handles subcommands and flags like `--diff`, `--goto`, `--wait`,
//! `--install-extension`, `--uninstall-extension`, `--locale`, `tunnel`,
//! `serve`, etc.

use std::path::PathBuf;

/// All parsed CLI arguments/subcommands.
#[derive(Debug, Clone)]
pub struct CliArgs {
    /// Files or folders to open.
    pub paths: Vec<PathBuf>,
    /// Force a new window.
    pub new_window: bool,
    /// Reuse an existing window if available.
    pub reuse_window: bool,
    /// Wait until the editor is closed (for git commit messages etc.).
    pub wait: bool,
    /// Diff two files.
    pub diff: Option<(PathBuf, PathBuf)>,
    /// Open a file at a specific location (`file:line:column`).
    pub goto: Option<GotoLocation>,
    /// Override display locale (e.g. `"de"`, `"zh-cn"`).
    pub locale: Option<String>,
    /// Install an extension by id.
    pub install_extension: Option<String>,
    /// Uninstall an extension by id.
    pub uninstall_extension: Option<String>,
    /// List installed extensions and exit.
    pub list_extensions: bool,
    /// Start with all extensions disabled.
    pub disable_extensions: bool,
    /// Enable verbose logging.
    pub verbose: bool,
    /// Subcommand mode.
    pub subcommand: Option<CliSubcommand>,
    /// Override user-data directory.
    pub user_data_dir: Option<PathBuf>,
    /// Override extensions directory.
    pub extensions_dir: Option<PathBuf>,
    /// Log to a file instead of stderr.
    pub log_file: Option<PathBuf>,
}

/// A file:line:column location parsed from `--goto`.
#[derive(Debug, Clone)]
pub struct GotoLocation {
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
}

/// Subcommands for the CLI.
#[derive(Debug, Clone)]
pub enum CliSubcommand {
    /// Start a remote tunnel server.
    Tunnel { name: Option<String> },
    /// Start a headless server (no GUI).
    Serve { port: u16 },
    /// Print version and exit.
    Version,
    /// Print status information.
    Status,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            new_window: false,
            reuse_window: false,
            wait: false,
            diff: None,
            goto: None,
            locale: None,
            install_extension: None,
            uninstall_extension: None,
            list_extensions: false,
            disable_extensions: false,
            verbose: false,
            subcommand: None,
            user_data_dir: None,
            extensions_dir: None,
            log_file: None,
        }
    }
}

/// Parse CLI arguments from raw `args` (typically `std::env::args()`).
///
/// The first element is expected to be the binary name and is skipped.
pub fn parse_cli_args<I, S>(args: I) -> Result<CliArgs, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut result = CliArgs::default();
    let args: Vec<String> = args.into_iter().map(|s| s.as_ref().to_string()).collect();
    let mut i = 1; // skip binary name

    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "--new-window" | "-n" => result.new_window = true,
            "--reuse-window" | "-r" => result.reuse_window = true,
            "--wait" | "-w" => result.wait = true,
            "--verbose" => result.verbose = true,
            "--disable-extensions" => result.disable_extensions = true,
            "--list-extensions" => result.list_extensions = true,

            "--diff" | "-d" => {
                if i + 2 >= args.len() {
                    return Err("--diff requires two file arguments".to_string());
                }
                let f1 = PathBuf::from(&args[i + 1]);
                let f2 = PathBuf::from(&args[i + 2]);
                result.diff = Some((f1, f2));
                i += 2;
            }

            "--goto" | "-g" => {
                if i + 1 >= args.len() {
                    return Err("--goto requires a file:line:column argument".to_string());
                }
                i += 1;
                result.goto = Some(parse_goto_location(&args[i])?);
            }

            "--locale" => {
                if i + 1 >= args.len() {
                    return Err("--locale requires a language code".to_string());
                }
                i += 1;
                result.locale = Some(args[i].clone());
            }

            "--install-extension" => {
                if i + 1 >= args.len() {
                    return Err("--install-extension requires an extension id".to_string());
                }
                i += 1;
                result.install_extension = Some(args[i].clone());
            }

            "--uninstall-extension" => {
                if i + 1 >= args.len() {
                    return Err("--uninstall-extension requires an extension id".to_string());
                }
                i += 1;
                result.uninstall_extension = Some(args[i].clone());
            }

            "--user-data-dir" => {
                if i + 1 >= args.len() {
                    return Err("--user-data-dir requires a path".to_string());
                }
                i += 1;
                result.user_data_dir = Some(PathBuf::from(&args[i]));
            }

            "--extensions-dir" => {
                if i + 1 >= args.len() {
                    return Err("--extensions-dir requires a path".to_string());
                }
                i += 1;
                result.extensions_dir = Some(PathBuf::from(&args[i]));
            }

            "--log" => {
                if i + 1 >= args.len() {
                    return Err("--log requires a file path".to_string());
                }
                i += 1;
                result.log_file = Some(PathBuf::from(&args[i]));
            }

            "tunnel" => {
                let mut name = None;
                if i + 2 < args.len() && args[i + 1] == "--name" {
                    name = Some(args[i + 2].clone());
                    i += 2;
                }
                result.subcommand = Some(CliSubcommand::Tunnel { name });
            }
            "serve" => {
                let mut port = 8080u16;
                if i + 1 < args.len() {
                    if args[i + 1] == "--port" && i + 2 < args.len() {
                        if let Ok(p) = args[i + 2].parse::<u16>() {
                            port = p;
                            i += 2;
                        }
                    } else if let Ok(p) = args[i + 1].parse::<u16>() {
                        port = p;
                        i += 1;
                    }
                }
                result.subcommand = Some(CliSubcommand::Serve { port });
            }
            "version" | "--version" | "-v" => {
                result.subcommand = Some(CliSubcommand::Version);
            }
            "status" => result.subcommand = Some(CliSubcommand::Status),

            other if other.starts_with('-') => {
                return Err(format!("unknown flag: {other}"));
            }

            _ => {
                if let Some(goto) = try_parse_goto(arg) {
                    result.goto = Some(goto);
                } else {
                    result.paths.push(PathBuf::from(arg));
                }
            }
        }

        i += 1;
    }

    Ok(result)
}

/// Parse a `file:line:column` string.
fn parse_goto_location(s: &str) -> Result<GotoLocation, String> {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    match parts.len() {
        3 => {
            let column = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("invalid column: {}", parts[0]))?;
            let line = parts[1]
                .parse::<u32>()
                .map_err(|_| format!("invalid line: {}", parts[1]))?;
            Ok(GotoLocation {
                path: PathBuf::from(parts[2]),
                line,
                column,
            })
        }
        2 => {
            let line = parts[0]
                .parse::<u32>()
                .map_err(|_| format!("invalid line: {}", parts[0]))?;
            Ok(GotoLocation {
                path: PathBuf::from(parts[1]),
                line,
                column: 1,
            })
        }
        _ => Err(format!(
            "invalid goto format: {s} (expected file:line or file:line:column)"
        )),
    }
}

/// Try to parse `file:line:column` from a positional argument. Returns `None`
/// if the string doesn't look like a goto location.
fn try_parse_goto(s: &str) -> Option<GotoLocation> {
    let parts: Vec<&str> = s.rsplitn(3, ':').collect();
    if parts.len() >= 2 && parts[0].chars().all(|c| c.is_ascii_digit()) {
        parse_goto_location(s).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<CliArgs, String> {
        parse_cli_args(args)
    }

    #[test]
    fn open_current_dir() {
        let args = parse(&["sidex", "."]).unwrap();
        assert_eq!(args.paths, vec![PathBuf::from(".")]);
    }

    #[test]
    fn open_file() {
        let args = parse(&["sidex", "file.rs"]).unwrap();
        assert_eq!(args.paths, vec![PathBuf::from("file.rs")]);
    }

    #[test]
    fn new_window_flag() {
        let args = parse(&["sidex", "--new-window"]).unwrap();
        assert!(args.new_window);
    }

    #[test]
    fn reuse_window_flag() {
        let args = parse(&["sidex", "--reuse-window"]).unwrap();
        assert!(args.reuse_window);
    }

    #[test]
    fn wait_flag() {
        let args = parse(&["sidex", "--wait"]).unwrap();
        assert!(args.wait);
    }

    #[test]
    fn diff_two_files() {
        let args = parse(&["sidex", "--diff", "a.rs", "b.rs"]).unwrap();
        let (f1, f2) = args.diff.unwrap();
        assert_eq!(f1, PathBuf::from("a.rs"));
        assert_eq!(f2, PathBuf::from("b.rs"));
    }

    #[test]
    fn goto_file_line_col() {
        let args = parse(&["sidex", "--goto", "main.rs:10:5"]).unwrap();
        let goto = args.goto.unwrap();
        assert_eq!(goto.path, PathBuf::from("main.rs"));
        assert_eq!(goto.line, 10);
        assert_eq!(goto.column, 5);
    }

    #[test]
    fn goto_file_line() {
        let args = parse(&["sidex", "--goto", "main.rs:42"]).unwrap();
        let goto = args.goto.unwrap();
        assert_eq!(goto.line, 42);
        assert_eq!(goto.column, 1);
    }

    #[test]
    fn locale_flag() {
        let args = parse(&["sidex", "--locale", "de"]).unwrap();
        assert_eq!(args.locale.as_deref(), Some("de"));
    }

    #[test]
    fn install_extension() {
        let args = parse(&["sidex", "--install-extension", "rust-lang.rust-analyzer"]).unwrap();
        assert_eq!(
            args.install_extension.as_deref(),
            Some("rust-lang.rust-analyzer")
        );
    }

    #[test]
    fn uninstall_extension() {
        let args = parse(&["sidex", "--uninstall-extension", "some.ext"]).unwrap();
        assert_eq!(args.uninstall_extension.as_deref(), Some("some.ext"));
    }

    #[test]
    fn list_extensions() {
        let args = parse(&["sidex", "--list-extensions"]).unwrap();
        assert!(args.list_extensions);
    }

    #[test]
    fn disable_extensions() {
        let args = parse(&["sidex", "--disable-extensions"]).unwrap();
        assert!(args.disable_extensions);
    }

    #[test]
    fn verbose() {
        let args = parse(&["sidex", "--verbose"]).unwrap();
        assert!(args.verbose);
    }

    #[test]
    fn tunnel_subcommand() {
        let args = parse(&["sidex", "tunnel"]).unwrap();
        assert!(matches!(args.subcommand, Some(CliSubcommand::Tunnel { name: None })));
    }

    #[test]
    fn tunnel_with_name() {
        let args = parse(&["sidex", "tunnel", "--name", "my-machine"]).unwrap();
        match args.subcommand {
            Some(CliSubcommand::Tunnel { name }) => assert_eq!(name.as_deref(), Some("my-machine")),
            other => panic!("expected Tunnel, got {other:?}"),
        }
    }

    #[test]
    fn serve_subcommand() {
        let args = parse(&["sidex", "serve", "3000"]).unwrap();
        match args.subcommand {
            Some(CliSubcommand::Serve { port }) => assert_eq!(port, 3000),
            other => panic!("expected Serve, got {other:?}"),
        }
    }

    #[test]
    fn serve_with_port_flag() {
        let args = parse(&["sidex", "serve", "--port", "9090"]).unwrap();
        match args.subcommand {
            Some(CliSubcommand::Serve { port }) => assert_eq!(port, 9090),
            other => panic!("expected Serve, got {other:?}"),
        }
    }

    #[test]
    fn serve_default_port() {
        let args = parse(&["sidex", "serve"]).unwrap();
        match args.subcommand {
            Some(CliSubcommand::Serve { port }) => assert_eq!(port, 8080),
            other => panic!("expected Serve, got {other:?}"),
        }
    }

    #[test]
    fn version_subcommand() {
        let args = parse(&["sidex", "--version"]).unwrap();
        assert!(matches!(args.subcommand, Some(CliSubcommand::Version)));
    }

    #[test]
    fn diff_missing_args() {
        assert!(parse(&["sidex", "--diff", "a.rs"]).is_err());
    }

    #[test]
    fn unknown_flag() {
        assert!(parse(&["sidex", "--unknown-flag"]).is_err());
    }

    #[test]
    fn positional_goto() {
        let args = parse(&["sidex", "src/main.rs:15:3"]).unwrap();
        let goto = args.goto.unwrap();
        assert_eq!(goto.path, PathBuf::from("src/main.rs"));
        assert_eq!(goto.line, 15);
        assert_eq!(goto.column, 3);
    }

    #[test]
    fn empty_args() {
        let args = parse(&["sidex"]).unwrap();
        assert!(args.paths.is_empty());
        assert!(args.subcommand.is_none());
    }

    #[test]
    fn extensions_dir_flag() {
        let args = parse(&["sidex", "--extensions-dir", "/tmp/exts"]).unwrap();
        assert_eq!(args.extensions_dir, Some(PathBuf::from("/tmp/exts")));
    }

    #[test]
    fn log_flag() {
        let args = parse(&["sidex", "--log", "/tmp/sidex.log"]).unwrap();
        assert_eq!(args.log_file, Some(PathBuf::from("/tmp/sidex.log")));
    }

    #[test]
    fn multiple_paths() {
        let args = parse(&["sidex", "a.rs", "b.rs", "src/"]).unwrap();
        assert_eq!(args.paths.len(), 3);
    }

    #[test]
    fn combined_flags() {
        let args = parse(&[
            "sidex",
            "--new-window",
            "--verbose",
            "--disable-extensions",
            "--locale",
            "fr",
            "project/",
        ])
        .unwrap();
        assert!(args.new_window);
        assert!(args.verbose);
        assert!(args.disable_extensions);
        assert_eq!(args.locale.as_deref(), Some("fr"));
        assert_eq!(args.paths, vec![PathBuf::from("project/")]);
    }
}
