//! Shell integration for enhanced terminal features.
//!
//! Tracks commands, working directories, and exit codes using ANSI escape
//! markers injected by shell init scripts. Provides command navigation,
//! decorations (success/failure indicators), and command history.

use std::path::PathBuf;
use std::time::SystemTime;

/// The type of shell running in the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Unknown,
}

/// A single command entry tracked by shell integration.
#[derive(Debug, Clone)]
pub struct CommandEntry {
    pub command: String,
    pub cwd: PathBuf,
    pub start_line: i32,
    pub end_line: Option<i32>,
    pub exit_code: Option<i32>,
    pub started_at: SystemTime,
    pub finished_at: Option<SystemTime>,
}

impl CommandEntry {
    pub fn is_finished(&self) -> bool {
        self.exit_code.is_some()
    }

    pub fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }

    pub fn duration(&self) -> Option<std::time::Duration> {
        let finished = self.finished_at?;
        finished.duration_since(self.started_at).ok()
    }
}

/// Shell integration state for a terminal instance.
pub struct ShellIntegration {
    pub shell: ShellType,
    pub command_history: Vec<CommandEntry>,
    pub current_command: Option<CommandEntry>,
    pub cwd: PathBuf,
}

impl ShellIntegration {
    pub fn new(shell: ShellType) -> Self {
        Self {
            shell,
            command_history: Vec::new(),
            current_command: None,
            cwd: PathBuf::new(),
        }
    }

    /// Records the start of a new command.
    pub fn command_started(&mut self, command: String, line: i32) {
        let entry = CommandEntry {
            command,
            cwd: self.cwd.clone(),
            start_line: line,
            end_line: None,
            exit_code: None,
            started_at: SystemTime::now(),
            finished_at: None,
        };
        self.current_command = Some(entry);
    }

    /// Records the completion of the current command.
    pub fn command_finished(&mut self, exit_code: i32, end_line: i32) {
        if let Some(mut entry) = self.current_command.take() {
            entry.exit_code = Some(exit_code);
            entry.end_line = Some(end_line);
            entry.finished_at = Some(SystemTime::now());
            self.command_history.push(entry);
        }
    }

    /// Updates the current working directory (received from shell).
    pub fn set_cwd(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }

    /// Returns the last N commands from history.
    pub fn recent_commands(&self, count: usize) -> &[CommandEntry] {
        let start = self.command_history.len().saturating_sub(count);
        &self.command_history[start..]
    }

    /// Finds the previous command start line from a given line.
    pub fn prev_command_line(&self, from_line: i32) -> Option<i32> {
        self.command_history
            .iter()
            .rev()
            .find(|e| e.start_line < from_line)
            .map(|e| e.start_line)
    }

    /// Finds the next command start line from a given line.
    pub fn next_command_line(&self, from_line: i32) -> Option<i32> {
        self.command_history
            .iter()
            .find(|e| e.start_line > from_line)
            .map(|e| e.start_line)
    }

    /// Returns the last finished command, if any.
    pub fn last_command(&self) -> Option<&CommandEntry> {
        self.command_history.last()
    }

    /// Clears all command history.
    pub fn clear_history(&mut self) {
        self.command_history.clear();
        self.current_command = None;
    }

    /// Returns whether a command is currently running.
    pub fn is_command_running(&self) -> bool {
        self.current_command.is_some()
    }
}

/// Detects the shell type from a command/path string.
pub fn detect_shell(command: &str) -> ShellType {
    let basename = std::path::Path::new(command)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(command);

    match basename.to_lowercase().as_str() {
        "bash" | "bash.exe" => ShellType::Bash,
        "zsh" => ShellType::Zsh,
        "fish" => ShellType::Fish,
        "powershell.exe" | "pwsh.exe" | "pwsh" | "powershell" => ShellType::PowerShell,
        "cmd.exe" | "cmd" => ShellType::Cmd,
        _ => ShellType::Unknown,
    }
}

/// Generates a shell init script for integration markers.
///
/// These scripts emit special OSC sequences that the terminal can parse to
/// track command boundaries, working directories, and exit codes.
pub fn generate_shell_init(shell: ShellType) -> String {
    match shell {
        ShellType::Bash => BASH_INTEGRATION.to_string(),
        ShellType::Zsh => ZSH_INTEGRATION.to_string(),
        ShellType::Fish => FISH_INTEGRATION.to_string(),
        ShellType::PowerShell => PWSH_INTEGRATION.to_string(),
        _ => String::new(),
    }
}

const BASH_INTEGRATION: &str = r#"
__sidex_prompt_command() {
    local exit_code=$?
    printf '\e]633;D;%s\a' "$exit_code"
    printf '\e]633;A\a'
    printf '\e]633;P;Cwd=%s\a' "$PWD"
}
__sidex_preexec() {
    printf '\e]633;C\a'
    printf '\e]633;E;%s\a' "$1"
}
if [[ -z "$__sidex_installed" ]]; then
    __sidex_installed=1
    trap '__sidex_preexec "$BASH_COMMAND"' DEBUG
    PROMPT_COMMAND="__sidex_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
fi
"#;

const ZSH_INTEGRATION: &str = r#"
__sidex_preexec() {
    printf '\e]633;C\a'
    printf '\e]633;E;%s\a' "$1"
}
__sidex_precmd() {
    local exit_code=$?
    printf '\e]633;D;%s\a' "$exit_code"
    printf '\e]633;A\a'
    printf '\e]633;P;Cwd=%s\a' "$PWD"
}
if [[ -z "$__sidex_installed" ]]; then
    __sidex_installed=1
    autoload -Uz add-zsh-hook
    add-zsh-hook preexec __sidex_preexec
    add-zsh-hook precmd __sidex_precmd
fi
"#;

const FISH_INTEGRATION: &str = r#"
function __sidex_preexec --on-event fish_preexec
    printf '\e]633;C\a'
    printf '\e]633;E;%s\a' "$argv[1]"
end
function __sidex_postexec --on-event fish_postexec
    printf '\e]633;D;%s\a' "$status"
    printf '\e]633;A\a'
    printf '\e]633;P;Cwd=%s\a' "$PWD"
end
"#;

const PWSH_INTEGRATION: &str = r#"
if (-not $__sidex_installed) {
    $__sidex_installed = $true
    $__sidex_original_prompt = $function:prompt
    function prompt {
        $exitCode = $LASTEXITCODE
        [Console]::Write("`e]633;D;$exitCode`a")
        [Console]::Write("`e]633;A`a")
        [Console]::Write("`e]633;P;Cwd=$PWD`a")
        & $__sidex_original_prompt
    }
    Set-PSReadLineKeyHandler -Key Enter -ScriptBlock {
        [Console]::Write("`e]633;C`a")
        [Microsoft.PowerShell.PSConsoleReadLine]::AcceptLine()
    }
}
"#;

/// The OSC marker codes used by shell integration.
pub mod markers {
    /// Prompt start marker.
    pub const PROMPT_START: &str = "633;A";
    /// Command start marker.
    pub const COMMAND_START: &str = "633;C";
    /// Command finished marker (followed by exit code).
    pub const COMMAND_FINISHED: &str = "633;D";
    /// Command text marker.
    pub const COMMAND_TEXT: &str = "633;E";
    /// Property marker (e.g. Cwd).
    pub const PROPERTY: &str = "633;P";
}

/// Parses an OSC 633 shell integration sequence.
pub fn parse_shell_integration_osc(payload: &str) -> Option<ShellIntegrationEvent> {
    if let Some(rest) = payload.strip_prefix("633;") {
        if rest.starts_with('A') {
            return Some(ShellIntegrationEvent::PromptStart);
        }
        if rest.starts_with('C') {
            return Some(ShellIntegrationEvent::CommandStart);
        }
        if let Some(code_str) = rest.strip_prefix("D;") {
            let code = code_str.trim().parse::<i32>().unwrap_or(-1);
            return Some(ShellIntegrationEvent::CommandFinished(code));
        }
        if let Some(cmd) = rest.strip_prefix("E;") {
            return Some(ShellIntegrationEvent::CommandText(cmd.to_string()));
        }
        if let Some(prop) = rest.strip_prefix("P;") {
            if let Some(cwd) = prop.strip_prefix("Cwd=") {
                return Some(ShellIntegrationEvent::SetCwd(PathBuf::from(cwd)));
            }
            return Some(ShellIntegrationEvent::Property(prop.to_string()));
        }
    }
    None
}

/// Events parsed from shell integration OSC sequences.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellIntegrationEvent {
    PromptStart,
    CommandStart,
    CommandFinished(i32),
    CommandText(String),
    SetCwd(PathBuf),
    Property(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_shell_types() {
        assert_eq!(detect_shell("/bin/bash"), ShellType::Bash);
        assert_eq!(detect_shell("/bin/zsh"), ShellType::Zsh);
        assert_eq!(detect_shell("/usr/bin/fish"), ShellType::Fish);
        assert_eq!(detect_shell("pwsh.exe"), ShellType::PowerShell);
        assert_eq!(detect_shell("cmd.exe"), ShellType::Cmd);
        assert_eq!(detect_shell("/bin/dash"), ShellType::Unknown);
    }

    #[test]
    fn generate_init_non_empty() {
        assert!(!generate_shell_init(ShellType::Bash).is_empty());
        assert!(!generate_shell_init(ShellType::Zsh).is_empty());
        assert!(!generate_shell_init(ShellType::Fish).is_empty());
        assert!(!generate_shell_init(ShellType::PowerShell).is_empty());
        assert!(generate_shell_init(ShellType::Unknown).is_empty());
    }

    #[test]
    fn command_lifecycle() {
        let mut si = ShellIntegration::new(ShellType::Bash);
        si.set_cwd(PathBuf::from("/home/user"));
        assert!(!si.is_command_running());

        si.command_started("ls -la".to_string(), 5);
        assert!(si.is_command_running());

        si.command_finished(0, 10);
        assert!(!si.is_command_running());
        assert_eq!(si.command_history.len(), 1);
        assert!(si.last_command().unwrap().succeeded());
    }

    #[test]
    fn command_navigation() {
        let mut si = ShellIntegration::new(ShellType::Zsh);
        si.set_cwd(PathBuf::from("/tmp"));

        si.command_started("cmd1".to_string(), 1);
        si.command_finished(0, 3);
        si.command_started("cmd2".to_string(), 5);
        si.command_finished(1, 8);
        si.command_started("cmd3".to_string(), 10);
        si.command_finished(0, 12);

        assert_eq!(si.prev_command_line(10), Some(5));
        assert_eq!(si.next_command_line(1), Some(5));
        assert_eq!(si.next_command_line(10), None);
    }

    #[test]
    fn parse_osc_events() {
        assert_eq!(
            parse_shell_integration_osc("633;A"),
            Some(ShellIntegrationEvent::PromptStart)
        );
        assert_eq!(
            parse_shell_integration_osc("633;C"),
            Some(ShellIntegrationEvent::CommandStart)
        );
        assert_eq!(
            parse_shell_integration_osc("633;D;0"),
            Some(ShellIntegrationEvent::CommandFinished(0))
        );
        assert_eq!(
            parse_shell_integration_osc("633;E;git status"),
            Some(ShellIntegrationEvent::CommandText("git status".to_string()))
        );
        assert_eq!(
            parse_shell_integration_osc("633;P;Cwd=/home/user"),
            Some(ShellIntegrationEvent::SetCwd(PathBuf::from("/home/user")))
        );
    }
}
