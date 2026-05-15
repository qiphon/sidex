#[cfg(test)]
mod git_operations_tests {
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    // 辅助函数：创建测试仓库
    fn create_test_repo() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Initialize git repo
        Command::new("git")
            .arg("init")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to initialize git repo");
        
        // Configure git
        Command::new("git")
            .arg("config")
            .arg("user.name")
            .arg("Test User")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to configure git user");
        
        Command::new("git")
            .arg("config")
            .arg("user.email")
            .arg("test@example.com")
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to configure git email");
        
        temp_dir
    }

    // 辅助函数：创建测试文件并提交
    fn create_and_commit_file(repo_dir: &TempDir, name: &str, content: &str, message: &str) {
        let path = repo_dir.path().join(name);
        fs::write(&path, content).expect("Failed to create file");
        
        Command::new("git")
            .arg("add")
            .arg(name)
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to add file");
        
        Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg(message)
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to commit");
    }

    // 3.1.1 Git 状态测试
    #[test]
    fn test_git_status_clean() {
        let repo_dir = create_test_repo();
        
        let output = Command::new("git")
            .arg("status")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git status");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("clean"));
    }

    #[test]
    fn test_git_status_with_changes() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "initial.txt", "initial content", "Initial commit");
        
        // Modify file
        fs::write(repo_dir.path().join("initial.txt"), "modified content").unwrap();
        
        let output = Command::new("git")
            .arg("status")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git status");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("modified"));
        assert!(stdout.contains("initial.txt"));
    }

    #[test]
    fn test_git_status_untracked_files() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "initial.txt", "initial", "Initial");
        
        // Create untracked file
        fs::write(repo_dir.path().join("untracked.txt"), "untracked content").unwrap();
        
        let output = Command::new("git")
            .arg("status")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git status");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("untracked"));
        assert!(stdout.contains("untracked.txt"));
    }

    #[test]
    fn test_git_status_ignored_files() {
        let repo_dir = create_test_repo();
        
        // Create .gitignore
        fs::write(repo_dir.path().join(".gitignore"), "node_modules/\n*.log").unwrap();
        
        // Create ignored file
        fs::create_dir_all(repo_dir.path().join("node_modules")).unwrap();
        fs::write(repo_dir.path().join("node_modules", "package.json"), "{}").unwrap();
        fs::write(repo_dir.path().join("debug.log"), "log content").unwrap();
        
        Command::new("git")
            .arg("add")
            .arg(".gitignore")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add gitignore")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("status")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git status");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("clean")); // Ignored files shouldn't appear
    }

    // 3.1.2 Git Diff 测试
    #[test]
    fn test_git_diff_staged() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "original", "Initial");
        
        // Modify and stage
        fs::write(repo_dir.path().join("file.txt"), "modified").unwrap();
        Command::new("git")
            .arg("add")
            .arg("file.txt")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("diff")
            .arg("--cached")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git diff");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("+modified"));
        assert!(stdout.contains("-original"));
    }

    #[test]
    fn test_git_diff_unstaged() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "original", "Initial");
        
        // Modify but don't stage
        fs::write(repo_dir.path().join("file.txt"), "modified").unwrap();
        
        let output = Command::new("git")
            .arg("diff")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git diff");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("+modified"));
        assert!(stdout.contains("-original"));
    }

    #[test]
    fn test_git_diff_binary_file() {
        let repo_dir = create_test_repo();
        
        // Create binary file (PNG header)
        let binary_content = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        fs::write(repo_dir.path().join("image.png"), &binary_content).unwrap();
        
        Command::new("git")
            .arg("add")
            .arg("image.png")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add image")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        // Modify binary file
        let modified_content = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0B];
        fs::write(repo_dir.path().join("image.png"), &modified_content).unwrap();
        
        let output = Command::new("git")
            .arg("diff")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git diff");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("Binary"));
    }

    // 3.1.3 Git 提交测试
    #[test]
    fn test_git_commit_single_file() {
        let repo_dir = create_test_repo();
        
        fs::write(repo_dir.path().join("single.txt"), "content").unwrap();
        
        Command::new("git")
            .arg("add")
            .arg("single.txt")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add single file")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to commit");
        
        assert!(output.status.success());
        
        // Verify commit
        let log = Command::new("git")
            .arg("log")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        assert!(String::from_utf8_lossy(&log.stdout).contains("Add single file"));
    }

    #[test]
    fn test_git_commit_multiple_files() {
        let repo_dir = create_test_repo();
        
        fs::write(repo_dir.path().join("file1.txt"), "content1").unwrap();
        fs::write(repo_dir.path().join("file2.txt"), "content2").unwrap();
        fs::write(repo_dir.path().join("file3.txt"), "content3").unwrap();
        
        Command::new("git")
            .arg("add")
            .arg(".")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("Add multiple files")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to commit");
        
        assert!(output.status.success());
    }

    #[test]
    fn test_git_commit_empty_message() {
        let repo_dir = create_test_repo();
        fs::write(repo_dir.path().join("file.txt"), "content").unwrap();
        
        Command::new("git")
            .arg("add")
            .arg("file.txt")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("commit")
            .arg("--allow-empty-message")
            .arg("-m")
            .arg("")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to commit");
        
        // With --allow-empty-message, it should succeed
        assert!(output.status.success());
    }

    // 3.1.4 Git 分支测试
    #[test]
    fn test_git_branch_list() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        let output = Command::new("git")
            .arg("branch")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git branch");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("* main") || stdout.contains("* master"));
    }

    #[test]
    fn test_git_branch_create() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        let output = Command::new("git")
            .arg("branch")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to create branch");
        
        assert!(output.status.success());
        
        let list = Command::new("git")
            .arg("branch")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        assert!(String::from_utf8_lossy(&list.stdout).contains("feature"));
    }

    #[test]
    fn test_git_checkout_branch() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        Command::new("git")
            .arg("branch")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("checkout")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to checkout branch");
        
        assert!(output.status.success());
        
        let current = Command::new("git")
            .arg("branch")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        assert!(String::from_utf8_lossy(&current.stdout).contains("* feature"));
    }

    #[test]
    fn test_git_checkout_file() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "original", "Initial");
        
        // Modify file
        fs::write(repo_dir.path().join("file.txt"), "modified").unwrap();
        
        let output = Command::new("git")
            .arg("checkout")
            .arg("--")
            .arg("file.txt")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to checkout file");
        
        assert!(output.status.success());
        
        let content = fs::read_to_string(repo_dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "original");
    }

    // 3.1.5 Git 日志测试
    #[test]
    fn test_git_log_basic() {
        let repo_dir = create_test_repo();
        
        create_and_commit_file(&repo_dir, "file1.txt", "content1", "First commit");
        create_and_commit_file(&repo_dir, "file2.txt", "content2", "Second commit");
        create_and_commit_file(&repo_dir, "file3.txt", "content3", "Third commit");
        
        let output = Command::new("git")
            .arg("log")
            .arg("--oneline")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git log");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        assert!(stdout.contains("First commit"));
        assert!(stdout.contains("Second commit"));
        assert!(stdout.contains("Third commit"));
    }

    #[test]
    fn test_git_log_limit() {
        let repo_dir = create_test_repo();
        
        create_and_commit_file(&repo_dir, "file1.txt", "content1", "Commit 1");
        create_and_commit_file(&repo_dir, "file2.txt", "content2", "Commit 2");
        create_and_commit_file(&repo_dir, "file3.txt", "content3", "Commit 3");
        
        let output = Command::new("git")
            .arg("log")
            .arg("--oneline")
            .arg("-2")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git log");
        
        assert!(output.status.success());
        let lines: Vec<&str> = String::from_utf8_lossy(&output.stdout).lines().collect();
        
        assert_eq!(lines.len(), 2);
    }

    // 3.1.6 Git Stash 测试
    #[test]
    fn test_git_stash_save() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "original", "Initial");
        
        // Modify file
        fs::write(repo_dir.path().join("file.txt"), "modified").unwrap();
        
        let output = Command::new("git")
            .arg("stash")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to stash");
        
        assert!(output.status.success());
        
        // File should be reverted
        let content = fs::read_to_string(repo_dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn test_git_stash_pop() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "original", "Initial");
        
        // Modify and stash
        fs::write(repo_dir.path().join("file.txt"), "modified").unwrap();
        Command::new("git")
            .arg("stash")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        // Pop stash
        let output = Command::new("git")
            .arg("stash")
            .arg("pop")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to pop stash");
        
        assert!(output.status.success());
        
        // File should be restored
        let content = fs::read_to_string(repo_dir.path().join("file.txt")).unwrap();
        assert_eq!(content, "modified");
    }

    // 3.1.7 Git Blame 测试
    #[test]
    fn test_git_blame_basic() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "line1\nline2\nline3", "Initial");
        
        let output = Command::new("git")
            .arg("blame")
            .arg("file.txt")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to run git blame");
        
        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        assert!(stdout.contains("Test User"));
        assert!(stdout.contains("line1"));
        assert!(stdout.contains("line2"));
        assert!(stdout.contains("line3"));
    }

    // 3.1.8 Git 合并测试
    #[test]
    fn test_git_merge_fast_forward() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "base.txt", "base", "Base commit");
        
        // Create feature branch
        Command::new("git")
            .arg("branch")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        Command::new("git")
            .arg("checkout")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        create_and_commit_file(&repo_dir, "feature.txt", "feature", "Feature commit");
        
        // Switch back and merge
        Command::new("git")
            .arg("checkout")
            .arg("main")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        let output = Command::new("git")
            .arg("merge")
            .arg("feature")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to merge");
        
        assert!(output.status.success());
    }

    // 3.1.9 Git 标签测试
    #[test]
    fn test_git_tag_create() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        let output = Command::new("git")
            .arg("tag")
            .arg("v1.0.0")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to create tag");
        
        assert!(output.status.success());
        
        let list = Command::new("git")
            .arg("tag")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        assert!(String::from_utf8_lossy(&list.stdout).contains("v1.0.0"));
    }

    #[test]
    fn test_git_tag_annotated() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        let output = Command::new("git")
            .arg("tag")
            .arg("-a")
            .arg("v1.0.0")
            .arg("-m")
            .arg("Version 1.0.0")
            .current_dir(repo_dir.path())
            .output()
            .expect("Failed to create annotated tag");
        
        assert!(output.status.success());
        
        let show = Command::new("git")
            .arg("show")
            .arg("v1.0.0")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        assert!(String::from_utf8_lossy(&show.stdout).contains("Version 1.0.0"));
    }

    // 3.1.10 错误处理测试
    #[test]
    fn test_git_not_a_repo() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        let output = Command::new("git")
            .arg("status")
            .current_dir(temp_dir.path())
            .output()
            .expect("Expected git to fail");
        
        assert!(!output.status.success());
    }

    #[test]
    fn test_git_invalid_path() {
        let repo_dir = create_test_repo();
        create_and_commit_file(&repo_dir, "file.txt", "content", "Initial");
        
        let output = Command::new("git")
            .arg("log")
            .arg("--")
            .arg("nonexistent.txt")
            .current_dir(repo_dir.path())
            .output()
            .unwrap();
        
        // Should succeed (just no output for non-existent file)
        assert!(output.status.success());
    }
}
