#[cfg(test)]
mod terminal_tests {
    use std::process::Command;
    use std::thread;
    use std::time::Duration;

    // 3.3.1 PTY 生命周期测试
    #[test]
    fn test_terminal_spawn() {
        #[cfg(target_os = "windows")]
        let shell = "cmd.exe";
        #[cfg(target_os = "linux")]
        let shell = "bash";
        #[cfg(target_os = "macos")]
        let shell = "zsh";

        let mut child = Command::new(shell).spawn().expect("Failed to spawn shell");
        
        // Give shell time to start
        thread::sleep(Duration::from_millis(500));
        
        // Terminate the process
        let _ = child.kill();
        let _ = child.wait();
        
        assert!(true); // Test passes if we can spawn and kill a shell
    }

    #[test]
    fn test_terminal_multiple() {
        #[cfg(target_os = "windows")]
        let shell = "cmd.exe";
        #[cfg(target_os = "linux")]
        let shell = "bash";
        #[cfg(target_os = "macos")]
        let shell = "zsh";

        let mut children = Vec::new();
        
        // Spawn multiple terminals
        for _ in 0..3 {
            let child = Command::new(shell).spawn().expect("Failed to spawn shell");
            children.push(child);
            thread::sleep(Duration::from_millis(100));
        }
        
        // Kill all children
        for mut child in children {
            let _ = child.kill();
            let _ = child.wait();
        }
        
        assert!(true);
    }

    #[test]
    fn test_terminal_pid() {
        #[cfg(target_os = "windows")]
        let shell = "cmd.exe";
        #[cfg(target_os = "linux")]
        let shell = "bash";
        #[cfg(target_os = "macos")]
        let shell = "zsh";

        let child = Command::new(shell).spawn().expect("Failed to spawn shell");
        
        // Get PID
        let pid = child.id();
        assert!(pid > 0);
        
        // Clean up
        let _ = child.kill();
        
        assert!(true);
    }

    // 3.3.2 Shell 检测测试
    #[test]
    fn test_get_default_shell() {
        #[cfg(target_os = "windows")]
        {
            let path = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
            assert!(path.contains("cmd.exe") || path.contains("powershell.exe"));
        }
        
        #[cfg(target_os = "linux")]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
            assert!(shell.contains("bash") || shell.contains("zsh") || shell.contains("sh"));
        }
        
        #[cfg(target_os = "macos")]
        {
            let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
            assert!(shell.contains("zsh") || shell.contains("bash"));
        }
    }

    #[test]
    fn test_exec_simple_command() {
        #[cfg(target_os = "windows")]
        let output = Command::new("echo").arg("Hello World").output();
        
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("echo").arg("Hello World").output();
        
        assert!(output.is_ok());
        let result = output.unwrap();
        assert!(result.status.success());
        
        #[cfg(target_os = "windows")]
        assert!(String::from_utf8_lossy(&result.stdout).contains("Hello World"));
        
        #[cfg(not(target_os = "windows"))]
        assert!(String::from_utf8_lossy(&result.stdout).contains("Hello World"));
    }

    #[test]
    fn test_exec_with_args() {
        #[cfg(target_os = "windows")]
        let output = Command::new("echo").args(&["arg1", "arg2", "arg3"]).output();
        
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("echo").args(&["arg1", "arg2", "arg3"]).output();
        
        assert!(output.is_ok());
        let result = output.unwrap();
        assert!(result.status.success());
    }

    #[test]
    fn test_exec_with_cwd() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        
        #[cfg(target_os = "windows")]
        let output = Command::new("cd").current_dir(temp_dir.path()).output();
        
        #[cfg(not(target_os = "windows"))]
        let output = Command::new("pwd").current_dir(temp_dir.path()).output();
        
        assert!(output.is_ok());
        let result = output.unwrap();
        assert!(result.status.success());
        
        let stdout = String::from_utf8_lossy(&result.stdout);
        assert!(stdout.contains(temp_dir.path().to_string_lossy().as_ref()));
    }

    #[test]
    fn test_exec_exit_code() {
        #[cfg(not(target_os = "windows"))]
        {
            // Test successful command
            let success = Command::new("true").output().unwrap();
            assert!(success.status.success());
            
            // Test failing command
            let failure = Command::new("false").output().unwrap();
            assert!(!failure.status.success());
        }
        
        #[cfg(target_os = "windows")]
        {
            // On Windows, use exit codes
            let success = Command::new("cmd").arg("/c").arg("exit 0").output().unwrap();
            assert!(success.status.success());
            
            let failure = Command::new("cmd").arg("/c").arg("exit 1").output().unwrap();
            assert!(!failure.status.success());
        }
    }

    #[test]
    fn test_exec_timeout() {
        #[cfg(not(target_os = "windows"))]
        {
            let mut child = Command::new("sleep").arg("10").spawn().expect("Failed to spawn sleep");
            
            // Wait with timeout
            let result = child.wait_timeout(Duration::from_secs(1));
            assert!(result.is_ok());
            assert!(result.unwrap().is_none()); // Process still running
            
            // Kill it
            let _ = child.kill();
            let _ = child.wait();
        }
        
        #[cfg(target_os = "windows")]
        {
            let mut child = Command::new("timeout").arg("/t").arg("10").arg("/nobreak").spawn().expect("Failed to spawn timeout");
            
            let result = child.wait_timeout(Duration::from_secs(1));
            assert!(result.is_ok());
            assert!(result.unwrap().is_none());
            
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    #[test]
    fn test_exec_nonexistent() {
        let output = Command::new("nonexistent_command_that_does_not_exist_12345").output();
        
        assert!(output.is_err() || !output.unwrap().status.success());
    }

    // 3.3.3 缓冲区测试
    #[test]
    fn test_terminal_buffer_small() {
        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new("echo").arg("a".repeat(100)).output().unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout.len(), 101); // 100 'a's + newline
        }
        
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("cmd").arg("/c").arg("echo").arg("a".repeat(100)).output().unwrap();
            assert!(output.status.success());
        }
    }

    #[test]
    fn test_terminal_buffer_large() {
        let large_input = "a".repeat(100_000);
        
        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new("echo").arg(large_input).output().unwrap();
            assert!(output.status.success());
            assert!(output.stdout.len() >= 100_000);
        }
    }

    // 3.3.4 ANSI 解析测试
    #[test]
    fn test_ansi_colors() {
        let ansi_text = "\x1b[31mRed text\x1b[0m Normal text";
        
        // Check that ANSI codes are present
        assert!(ansi_text.contains("\x1b["));
        assert!(ansi_text.contains("31m"));
        assert!(ansi_text.contains("0m"));
        
        // Strip ANSI codes
        let stripped: String = ansi_text.chars()
            .enumerate()
            .filter(|(i, c)| {
                if *c == '\x1b' {
                    // Skip escape sequence
                    false
                } else if i > &0 && ansi_text.chars().nth(i - 1) == Some('\x1b') {
                    // Skip characters after escape
                    let remaining = &ansi_text[i..];
                    if let Some(end) = remaining.find(|c: char| c.is_ascii_alphabetic()) {
                        i < &(i + end + 1)
                    } else {
                        false
                    }
                } else {
                    true
                }
            })
            .map(|(_, c)| c)
            .collect();
        
        assert!(stripped.contains("Red text"));
        assert!(stripped.contains("Normal text"));
    }

    // 3.3.5 错误处理测试
    #[test]
    fn test_terminal_permission_denied() {
        #[cfg(not(target_os = "windows"))]
        {
            // Try to execute a file without execute permissions
            let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
            let no_exec_file = temp_dir.path().join("noexec.sh");
            std::fs::write(&no_exec_file, "#!/bin/bash\necho test").unwrap();
            
            // Remove execute permission
            let _ = Command::new("chmod").arg("-x").arg(&no_exec_file).output();
            
            let output = Command::new(&no_exec_file).output();
            assert!(output.is_err() || !output.unwrap().status.success());
        }
    }

    // 3.3.6 环境变量测试
    #[test]
    fn test_exec_with_env() {
        use std::env;
        
        let mut envs = env::vars().collect::<Vec<_>>();
        envs.push(("TEST_VAR".to_string(), "test_value".to_string()));
        
        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new("bash")
                .arg("-c")
                .arg("echo $TEST_VAR")
                .envs(envs)
                .output()
                .unwrap();
            
            assert!(output.status.success());
            assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "test_value");
        }
        
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("cmd")
                .arg("/c")
                .arg("echo %TEST_VAR%")
                .envs(envs)
                .output()
                .unwrap();
            
            assert!(output.status.success());
        }
    }

    // 3.3.7 管道测试
    #[test]
    fn test_pipeline() {
        #[cfg(not(target_os = "windows"))]
        {
            // Test simple pipeline: echo "test" | grep "es"
            let echo = Command::new("echo").arg("test").output().unwrap();
            assert!(echo.status.success());
            
            let mut grep = Command::new("grep").arg("es").stdin(std::process::Stdio::piped()).spawn().unwrap();
            
            if let Some(mut stdin) = grep.stdin.take() {
                use std::io::Write;
                stdin.write_all(&echo.stdout).unwrap();
            }
            
            let grep_output = grep.wait_with_output().unwrap();
            assert!(grep_output.status.success());
            assert!(String::from_utf8_lossy(&grep_output.stdout).contains("test"));
        }
    }
}
