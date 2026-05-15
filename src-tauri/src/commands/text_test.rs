#[cfg(test)]
mod text_processing_tests {
    use std::fs;
    use tempfile::TempDir;

    // 辅助函数：创建测试文件
    fn create_test_file(dir: &std::path::PathBuf, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).expect("Failed to create test file");
        path
    }

    // 3.4.1 行数统计测试
    #[test]
    fn test_count_lines_empty() {
        let content = "";
        let expected = 0;
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    #[test]
    fn test_count_lines_single() {
        let content = "Single line";
        let expected = 1;
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    #[test]
    fn test_count_lines_multiple() {
        let content = "Line 1\nLine 2\nLine 3";
        let expected = 3;
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    #[test]
    fn test_count_lines_with_trailing_newline() {
        let content = "Line 1\nLine 2\nLine 3\n";
        let expected = 3; // Trailing newline doesn't count as a line
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    #[test]
    fn test_count_lines_without_newline() {
        let content = "No trailing newline";
        let expected = 1;
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    #[test]
    fn test_count_lines_with_empty_lines() {
        let content = "Line 1\n\nLine 3\n\n\nLine 6";
        let expected = 6; // Empty lines count as lines
        
        let count = content.lines().count();
        assert_eq!(count, expected);
    }

    // 3.4.2 行尾处理测试
    #[test]
    fn test_normalize_line_endings_lf() {
        let content = "Line 1\nLine 2\nLine 3";
        let expected = content;
        
        // LF should remain unchanged
        let normalized = content.replace("\r\n", "\n");
        assert_eq!(normalized, expected);
    }

    #[test]
    fn test_normalize_line_endings_crlf() {
        let content = "Line 1\r\nLine 2\r\nLine 3";
        let expected = "Line 1\nLine 2\nLine 3";
        
        let normalized = content.replace("\r\n", "\n");
        assert_eq!(normalized, expected);
    }

    #[test]
    fn test_normalize_line_endings_mixed() {
        let content = "Line 1\r\nLine 2\nLine 3\r\nLine 4\n";
        let expected = "Line 1\nLine 2\nLine 3\nLine 4\n";
        
        let normalized = content.replace("\r\n", "\n");
        assert_eq!(normalized, expected);
    }

    #[test]
    fn test_to_crlf() {
        let content = "Line 1\nLine 2\nLine 3";
        let expected = "Line 1\r\nLine 2\r\nLine 3";
        
        let converted = content.replace("\n", "\r\n");
        assert_eq!(converted, expected);
    }

    #[test]
    fn test_trim_trailing_whitespace() {
        let content = "Line with trailing spaces  \nAnother line\t\nLast line   ";
        let expected = "Line with trailing spaces\nAnother line\nLast line";
        
        let trimmed: Vec<&str> = content.lines().map(|line| line.trim_end()).collect();
        let result = trimmed.join("\n");
        assert_eq!(result, expected);
    }

    // 3.4.3 单词边界测试
    #[test]
    fn test_word_boundaries_english() {
        let content = "Hello world! This is a test.";
        let words: Vec<&str> = content.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
        
        assert_eq!(words, vec!["Hello", "world", "This", "is", "a", "test"]);
    }

    #[test]
    fn test_word_boundaries_chinese() {
        let content = "你好世界！这是一个测试。";
        let chars: Vec<char> = content.chars().filter(|c| !"！。".contains(*c)).collect();
        
        assert!(chars.len() > 0);
        assert!(chars.contains(&'你'));
        assert!(chars.contains(&'好'));
        assert!(chars.contains(&'世'));
        assert!(chars.contains(&'界'));
    }

    #[test]
    fn test_word_boundaries_mixed() {
        let content = "Hello 世界！This is 中文 test.";
        let parts: Vec<&str> = content.split(|c: char| c.is_whitespace() || "！.".contains(c)).filter(|s| !s.is_empty()).collect();
        
        assert!(parts.contains(&"Hello"));
        assert!(parts.contains(&"世界"));
        assert!(parts.contains(&"This"));
        assert!(parts.contains(&"中文"));
        assert!(parts.contains(&"test"));
    }

    #[test]
    fn test_word_boundaries_punctuation() {
        let content = "Hello, world! How are you?";
        let words: Vec<&str> = content.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
        
        assert_eq!(words, vec!["Hello", "world", "How", "are", "you"]);
    }

    // 3.4.4 文件摘要测试
    #[test]
    fn test_file_summary_basic() {
        let content = "This is a simple file content with some text.";
        let summary = if content.len() <= 50 {
            content.to_string()
        } else {
            content[..50].to_string() + "..."
        };
        
        assert_eq!(summary, "This is a simple file content with some text.");
    }

    #[test]
    fn test_file_summary_truncated() {
        let content = "This is a very long file content that definitely exceeds the 50 character limit by a significant amount.";
        let summary = if content.len() <= 50 {
            content.to_string()
        } else {
            content[..50].to_string() + "..."
        };
        
        assert_eq!(summary.len(), 53); // 50 chars + "..."
        assert!(summary.starts_with("This is a very long file content that"));
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn test_file_summary_empty() {
        let content = "";
        let summary = if content.is_empty() {
            "(empty)".to_string()
        } else if content.len() <= 50 {
            content.to_string()
        } else {
            content[..50].to_string() + "..."
        };
        
        assert_eq!(summary, "(empty)");
    }

    // 3.4.5 文件比较测试
    #[test]
    fn test_files_equal_identical() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path1 = create_test_file(&temp_dir.path().to_path_buf(), "file1.txt", "same content");
        let path2 = create_test_file(&temp_dir.path().to_path_buf(), "file2.txt", "same content");
        
        let content1 = fs::read_to_string(&path1).unwrap();
        let content2 = fs::read_to_string(&path2).unwrap();
        
        assert_eq!(content1, content2);
    }

    #[test]
    fn test_files_equal_different() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path1 = create_test_file(&temp_dir.path().to_path_buf(), "file1.txt", "content 1");
        let path2 = create_test_file(&temp_dir.path().to_path_buf(), "file2.txt", "content 2");
        
        let content1 = fs::read_to_string(&path1).unwrap();
        let content2 = fs::read_to_string(&path2).unwrap();
        
        assert_ne!(content1, content2);
    }

    #[test]
    fn test_file_hash() {
        use sha2::{Sha256, Digest};
        
        let content = "test content for hash";
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();
        
        // Verify hash length
        assert_eq!(hash.len(), 32); // SHA256 produces 32 bytes
        
        // Verify same content produces same hash
        let mut hasher2 = Sha256::new();
        hasher2.update(content.as_bytes());
        let hash2 = hasher2.finalize();
        
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_simple_diff_additions() {
        let old_content = "Line 1\nLine 2\nLine 3";
        let new_content = "Line 1\nLine 2\nAdded line\nLine 3";
        
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        
        // Find added lines
        let added_lines: Vec<&&str> = new_lines.iter().filter(|line| !old_lines.contains(line)).collect();
        
        assert_eq!(added_lines, vec![&"Added line"]);
    }

    #[test]
    fn test_simple_diff_deletions() {
        let old_content = "Line 1\nLine 2\nLine 3";
        let new_content = "Line 1\nLine 3";
        
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        
        // Find deleted lines
        let deleted_lines: Vec<&&str> = old_lines.iter().filter(|line| !new_lines.contains(line)).collect();
        
        assert_eq!(deleted_lines, vec![&"Line 2"]);
    }

    #[test]
    fn test_simple_diff_modifications() {
        let old_content = "Line 1\nOriginal line\nLine 3";
        let new_content = "Line 1\nModified line\nLine 3";
        
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        
        // Find modified lines (same position, different content)
        let modifications: Vec<(&str, &str)> = old_lines
            .iter()
            .enumerate()
            .filter(|(_, old_line)| {
                new_lines.get(0).map(|new_line| new_line != *old_line).unwrap_or(false)
            })
            .map(|(i, old_line)| (*old_line, new_lines[i]))
            .collect();
        
        assert!(!modifications.is_empty());
    }

    #[test]
    fn test_simple_diff_empty() {
        let old_content = "Line 1\nLine 2\nLine 3";
        let new_content = "Line 1\nLine 2\nLine 3";
        
        let old_lines: Vec<&str> = old_content.lines().collect();
        let new_lines: Vec<&str> = new_content.lines().collect();
        
        let added_lines: Vec<&&str> = new_lines.iter().filter(|line| !old_lines.contains(line)).collect();
        let deleted_lines: Vec<&&str> = old_lines.iter().filter(|line| !new_lines.contains(line)).collect();
        
        assert!(added_lines.is_empty());
        assert!(deleted_lines.is_empty());
    }

    // 3.4.6 编码检测测试
    #[test]
    fn test_detect_utf8() {
        let content = "Hello 世界 你好";
        let is_valid_utf8 = std::str::from_utf8(content.as_bytes()).is_ok();
        
        assert!(is_valid_utf8);
    }

    #[test]
    fn test_detect_invalid_utf8() {
        let invalid_bytes: &[u8] = &[0xFF, 0xFE, 0xFD]; // Invalid UTF-8 sequence
        let result = std::str::from_utf8(invalid_bytes);
        
        assert!(result.is_err());
    }

    #[test]
    fn test_utf16_conversion() {
        let content = "Hello 世界";
        let utf16: Vec<u16> = content.encode_utf16().collect();
        
        // Convert back to UTF-8
        let decoded = String::from_utf16(&utf16).expect("Failed to decode UTF-16");
        
        assert_eq!(decoded, content);
    }

    // 3.4.7 大文件处理测试
    #[test]
    fn test_large_file_line_count() {
        let large_content = "Line\n".repeat(100_000); // 100,000 lines
        
        let line_count = large_content.lines().count();
        assert_eq!(line_count, 100_000);
    }

    #[test]
    fn test_memory_efficiency() {
        use std::io::{BufRead, BufReader};
        use std::io::Cursor;
        
        let content = "Line\n".repeat(10_000);
        let cursor = Cursor::new(content.as_bytes());
        let reader = BufReader::new(cursor);
        
        let mut line_count = 0;
        for _ in reader.lines() {
            line_count += 1;
        }
        
        assert_eq!(line_count, 10_000);
    }

    // 3.4.8 特殊字符处理测试
    #[test]
    fn test_special_characters() {
        let content = "Normal text\nWith\ttabs\nWith\r\nCRLF\nUnicode: 😊🎉";
        
        let lines: Vec<&str> = content.split('\n').collect();
        assert_eq!(lines.len(), 4); // Split by LF only
        
        // Check tabs are preserved
        assert!(lines[1].contains('\t'));
        
        // Check Unicode is preserved
        assert!(lines[3].contains('😊'));
        assert!(lines[3].contains('🎉'));
    }

    #[test]
    fn test_null_byte_handling() {
        let content = "Hello\x00World";
        
        // In Rust, strings with null bytes are handled, but may cause issues in some contexts
        assert_eq!(content.len(), 11); // 11 bytes including null
        assert!(content.contains('\x00'));
    }
}
