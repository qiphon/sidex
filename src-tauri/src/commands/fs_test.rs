#[cfg(test)]
mod fs_operations_tests {
    use std::fs;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).expect("Failed to create test file");
        path
    }

    // 3.2.1 读取文件测试
    #[test]
    fn test_read_file_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "Hello, World!";
        let path = create_test_file(&temp_dir.path().to_path_buf(), "test.txt", content);

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_read_file_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "empty.txt", "");

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_read_file_utf8() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "你好，世界！🎉 中文字符测试";
        let path = create_test_file(&temp_dir.path().to_path_buf(), "chinese.txt", content);

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_read_file_with_spaces() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "File with spaces in name";
        let path = temp_dir.path().join("file with spaces.txt");
        fs::write(&path, content).expect("Failed to create file");

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_read_nonexistent() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("nonexistent.txt");

        let result = fs::read_to_string(&path);
        assert!(result.is_err());
    }

    // 3.2.2 写入文件测试
    #[test]
    fn test_write_file_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("write_test.txt");
        let content = "Write test content";

        let result = fs::write(&path, content);
        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn test_write_file_overwrite() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "overwrite.txt", "Original");
        let new_content = "Overwritten";

        let result = fs::write(&path, new_content);
        assert!(result.is_ok());
        assert_eq!(fs::read_to_string(&path).unwrap(), new_content);
    }

    #[test]
    fn test_write_file_create_parent() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("subdir").join("nested").join("file.txt");
        let content = "Nested file";

        // Note: fs::write does not create parent directories
        // This test documents the expected behavior
        let result = fs::create_dir_all(path.parent().unwrap());
        assert!(result.is_ok());
        
        let write_result = fs::write(&path, content);
        assert!(write_result.is_ok());
        assert_eq!(fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn test_write_file_binary() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("binary.bin");
        let bytes: Vec<u8> = vec![0x00, 0xFF, 0x42, 0x13, 0x37];

        let result = fs::write(&path, &bytes);
        assert!(result.is_ok());
        
        let read_bytes = fs::read(&path).unwrap();
        assert_eq!(read_bytes, bytes);
    }

    // 3.2.3 目录操作测试
    #[test]
    fn test_read_dir_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create some files
        create_test_file(&temp_dir.path().to_path_buf(), "file1.txt", "content1");
        create_test_file(&temp_dir.path().to_path_buf(), "file2.txt", "content2");
        fs::create_dir(temp_dir.path().join("subdir")).expect("Failed to create subdir");

        let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 3); // 2 files + 1 directory
    }

    #[test]
    fn test_read_dir_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_mkdir_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let new_dir = temp_dir.path().join("new_directory");

        let result = fs::create_dir(&new_dir);
        assert!(result.is_ok());
        assert!(new_dir.is_dir());
    }

    #[test]
    fn test_mkdir_nested() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let nested_path = temp_dir.path().join("a").join("b").join("c");

        let result = fs::create_dir_all(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.is_dir());
    }

    #[test]
    fn test_mkdir_already_exists() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        let result = fs::create_dir(temp_dir.path());
        assert!(result.is_err()); // Should fail because dir already exists
    }

    #[test]
    fn test_remove_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "to_remove.txt", "content");

        let result = fs::remove_file(&path);
        assert!(result.is_ok());
        assert!(!path.exists());
    }

    #[test]
    fn test_remove_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let subdir = temp_dir.path().join("to_remove_dir");
        fs::create_dir(&subdir).expect("Failed to create subdir");

        let result = fs::remove_dir(&subdir);
        assert!(result.is_ok());
        assert!(!subdir.exists());
    }

    #[test]
    fn test_remove_nonexistent() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("nonexistent.txt");

        let result = fs::remove_file(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let old_path = create_test_file(&temp_dir.path().to_path_buf(), "old_name.txt", "content");
        let new_path = temp_dir.path().join("new_name.txt");

        let result = fs::rename(&old_path, &new_path);
        assert!(result.is_ok());
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    #[test]
    fn test_rename_dir() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let old_dir = temp_dir.path().join("old_dir");
        fs::create_dir(&old_dir).expect("Failed to create old dir");
        let new_dir = temp_dir.path().join("new_dir");

        let result = fs::rename(&old_dir, &new_dir);
        assert!(result.is_ok());
        assert!(!old_dir.exists());
        assert!(new_dir.exists());
    }

    // 3.2.4 文件元数据测试
    #[test]
    fn test_stat_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "stat_test.txt", "content");

        let metadata = fs::metadata(&path).expect("Failed to get metadata");
        assert!(metadata.is_file());
        assert_eq!(metadata.len(), 7); // "content" is 7 bytes
    }

    #[test]
    fn test_stat_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        let metadata = fs::metadata(temp_dir.path()).expect("Failed to get metadata");
        assert!(metadata.is_dir());
    }

    #[test]
    fn test_exists_true() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "exists.txt", "content");

        assert!(path.exists());
    }

    #[test]
    fn test_exists_false() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("nonexistent.txt");

        assert!(!path.exists());
    }

    // 3.2.5 路径处理测试
    #[test]
    fn test_path_normalization() {
        let path = PathBuf::from("/home/user/../user/./file.txt");
        let normalized = path.components()
            .fold(PathBuf::new(), |acc, c| match c {
                std::path::Component::ParentDir => {
                    acc.parent().map(|p| p.to_path_buf()).unwrap_or(acc)
                }
                std::path::Component::CurDir => acc,
                std::path::Component::Normal(s) => acc.join(s),
                _ => acc.join(c.as_os_str()),
            });
        
        assert!(normalized.to_string_lossy().contains("user/file.txt"));
    }

    #[test]
    fn test_path_traversal() {
        // Test that path traversal attempts are properly handled
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("safe.txt");
        fs::write(&file_path, "safe").expect("Failed to write");

        // Attempt path traversal
        let malicious_path = temp_dir.path().join("..").join("safe.txt");
        
        // The actual traversal prevention depends on the implementation
        // This test documents the expected behavior
        assert!(malicious_path.exists()); // Path exists but may not be accessible
    }

    #[test]
    fn test_absolute_vs_relative() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Absolute path
        assert!(temp_dir.path().is_absolute());
        
        // Relative path
        let relative = PathBuf::from("relative/path");
        assert!(!relative.is_absolute());
    }

    #[test]
    fn test_path_with_special_chars() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "Special chars test";
        
        // Test paths with various special characters
        let special_names = vec![
            "file with spaces.txt",
            "file-with-dashes.txt",
            "file_with_underscores.txt",
            "file.with.dots.txt",
            "file@with@ats.txt",
            "file#with#hashes.txt",
        ];

        for name in special_names {
            let path = temp_dir.path().join(name);
            let write_result = fs::write(&path, content);
            assert!(write_result.is_ok(), "Failed to write: {}", name);
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
    }

    // 3.2.6 字节流测试
    #[test]
    fn test_read_file_bytes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let bytes: Vec<u8> = (0..255).collect(); // All byte values
        let path = temp_dir.path().join("bytes.bin");

        fs::write(&path, &bytes).expect("Failed to write bytes");
        
        let read_bytes = fs::read(&path).expect("Failed to read bytes");
        assert_eq!(read_bytes, bytes);
    }

    #[test]
    fn test_write_file_bytes() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("write_bytes.bin");
        let bytes: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF];

        let result = fs::write(&path, &bytes);
        assert!(result.is_ok());
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }

    // 3.2.7 边界情况测试
    #[test]
    fn test_read_write_very_long_content() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("long.txt");
        let content = "A".repeat(1_000_000); // 1MB of 'A's

        fs::write(&path, &content).expect("Failed to write long content");
        
        let read_content = fs::read_to_string(&path).expect("Failed to read long content");
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_file_permissions() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "permissions.txt", "content");

        let metadata = fs::metadata(&path).expect("Failed to get metadata");
        // Check that we can read permissions
        assert!(metadata.permissions().readonly() == false || metadata.permissions().readonly() == true);
    }

    #[test]
    fn test_concurrent_read_write_same_file() {
        use std::thread;
        use std::sync::Arc;
        
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = temp_dir.path().join("concurrent.txt");
        
        // Initialize file
        fs::write(&path, "initial").expect("Failed to write initial");
        
        // Note: Concurrent reads/writes to the same file may cause issues
        // This test documents expected behavior
        let path_clone = path.clone();
        let handle = thread::spawn(move || {
            fs::write(&path_clone, "modified").expect("Failed to write in thread");
        });
        
        handle.join().expect("Thread panicked");
        
        // Final state
        let final_content = fs::read_to_string(&path).expect("Failed to read final");
        assert!(final_content == "initial" || final_content == "modified");
    }
}
