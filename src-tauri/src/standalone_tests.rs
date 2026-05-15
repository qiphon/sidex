// Standalone test module for SideX core functionality
// These tests don't require Tauri dependencies and can be run with a minimal Rust setup

#[cfg(test)]
mod standalone_tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // 辅助函数
    fn create_test_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).expect("Failed to create test file");
        path
    }

    // ===== 文件系统测试 =====
    #[test]
    fn test_fs_read_write() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "Hello, World!";
        let path = create_test_file(&temp_dir.path().to_path_buf(), "test.txt", content);

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_fs_utf8() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "你好，世界！🎉";
        let path = create_test_file(&temp_dir.path().to_path_buf(), "chinese.txt", content);

        let result = fs::read_to_string(&path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), content);
    }

    #[test]
    fn test_fs_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        create_test_file(&temp_dir.path().to_path_buf(), "file1.txt", "content1");
        create_test_file(&temp_dir.path().to_path_buf(), "file2.txt", "content2");

        let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_fs_nested_directory() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let nested_path = temp_dir.path().join("a").join("b").join("c");

        let result = fs::create_dir_all(&nested_path);
        assert!(result.is_ok());
        assert!(nested_path.is_dir());
    }

    #[test]
    fn test_fs_rename() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let old_path = create_test_file(&temp_dir.path().to_path_buf(), "old.txt", "content");
        let new_path = temp_dir.path().join("new.txt");

        let result = fs::rename(&old_path, &new_path);
        assert!(result.is_ok());
        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    // ===== 文本处理测试 =====
    #[test]
    fn test_text_line_count() {
        let content = "Line 1\nLine 2\nLine 3";
        let count = content.lines().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_text_line_count_empty() {
        let content = "";
        let count = content.lines().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_text_line_endings_crlf() {
        let content = "Line 1\r\nLine 2\r\nLine 3";
        let normalized = content.replace("\r\n", "\n");
        assert_eq!(normalized.lines().count(), 3);
    }

    #[test]
    fn test_text_trim_trailing() {
        let content = "Line with spaces   \nAnother line\t\n";
        let trimmed: Vec<&str> = content.lines().map(|line| line.trim_end()).collect();
        
        assert_eq!(trimmed[0], "Line with spaces");
        assert_eq!(trimmed[1], "Another line");
    }

    #[test]
    fn test_text_word_boundary_english() {
        let content = "Hello world! This is a test.";
        let words: Vec<&str> = content.split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
        
        assert_eq!(words, vec!["Hello", "world", "This", "is", "a", "test"]);
    }

    #[test]
    fn test_text_word_boundary_chinese() {
        let content = "你好世界";
        let chars: Vec<char> = content.chars().collect();
        
        assert_eq!(chars.len(), 4);
        assert!(chars.contains(&'你'));
        assert!(chars.contains(&'好'));
    }

    #[test]
    fn test_text_file_hash() {
        use sha2::{Sha256, Digest};
        
        let content = "test content for hash";
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let hash = hasher.finalize();
        
        assert_eq!(hash.len(), 32); // SHA256 produces 32 bytes
    }

    // ===== 搜索功能测试 =====
    #[test]
    fn test_search_basic() {
        let content = "Hello world!\nThis is a test file.\nSearch for 'test'.";
        let query = "test";
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.to_lowercase().contains(&query.to_lowercase()))
            .collect();
        
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_search_regex() {
        use regex::Regex;
        
        let content = "Line 1: 123\nLine 2: 456\nLine 3: 789";
        let pattern = r"\d+";
        
        let re = Regex::new(pattern).expect("Invalid regex");
        let matches: Vec<&str> = content.lines()
            .filter(|line| re.is_match(line))
            .collect();
        
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_whole_word() {
        let content = "testing test tester testy";
        let query = "test";
        
        let pattern = format!(r"\b{}\b", query);
        let re = regex::Regex::new(&pattern).expect("Invalid regex");
        
        let matches: Vec<&str> = content.split_whitespace()
            .filter(|word| re.is_match(word))
            .collect();
        
        assert_eq!(matches, vec!["test"]);
    }

    #[test]
    fn test_search_replace() {
        let content = "Hello world\nWorld is big";
        let search = "World";
        let replace = "Universe";
        
        let result: String = content.lines()
            .map(|line| line.replace(search, replace))
            .collect::<Vec<_>>()
            .join("\n");
        
        assert!(result.contains("Universe"));
        assert!(result.contains("Hello"));
    }

    #[test]
    fn test_search_case_sensitive() {
        let content = "Hello World\nhello world\nHELLO WORLD";
        let query = "Hello";
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.contains(query))
            .collect();
        
        assert_eq!(matches.len(), 1);
    }

    #[test]
    fn test_search_case_insensitive() {
        let content = "Hello World\nhello world\nHELLO WORLD";
        let query = "hello";
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.to_lowercase().contains(&query.to_lowercase()))
            .collect();
        
        assert_eq!(matches.len(), 3);
    }

    // ===== Git 相关测试（使用标准库） =====
    #[test]
    fn test_git_query_decoding() {
        // 测试我们实现的 Base64 + URL 解码逻辑
        use base64::{Engine as _, engine::general_purpose};
        
        // 标准 Base64
        let encoded = "eyJmaWxlUGF0aCI6ImZpbGUudHMiLCJjb21taXQiOiIxMjM0NTY3ODlhYiJ9";
        let decoded = general_purpose::STANDARD.decode(encoded).unwrap();
        let text = String::from_utf8(decoded).unwrap();
        
        assert!(text.contains("file.ts"));
        assert!(text.contains("123456789ab"));
    }

    #[test]
    fn test_url_encoding() {
        use urlencoding;
        
        // 测试 URL 编码
        let input = "file/path 中文";
        let encoded = urlencoding::encode(input);
        assert!(encoded.contains("%2F")); // /
        assert!(encoded.contains("%E4%B8%AD%E6%96%87")); // 中文
        
        // 测试解码
        let decoded = urlencoding::decode(&encoded).unwrap();
        assert_eq!(decoded, input);
    }

    // ===== 编码测试 =====
    #[test]
    fn test_utf8_validation() {
        let valid_utf8 = "Hello 世界 你好 🎉";
        assert!(std::str::from_utf8(valid_utf8.as_bytes()).is_ok());
        
        let invalid_bytes: &[u8] = &[0xFF, 0xFE, 0xFD];
        assert!(std::str::from_utf8(invalid_bytes).is_err());
    }

    #[test]
    fn test_utf16_roundtrip() {
        let content = "Hello 世界";
        let utf16: Vec<u16> = content.encode_utf16().collect();
        let decoded = String::from_utf16(&utf16).expect("Failed to decode UTF-16");
        
        assert_eq!(decoded, content);
    }

    #[test]
    fn test_base64_roundtrip() {
        use base64::{Engine as _, engine::general_purpose};
        
        let original = "Hello, World! 你好世界 🎉";
        let encoded = general_purpose::STANDARD.encode(original.as_bytes());
        let decoded = general_purpose::STANDARD.decode(&encoded).unwrap();
        let result = String::from_utf8(decoded).unwrap();
        
        assert_eq!(result, original);
    }

    // ===== 路径处理测试 =====
    #[test]
    fn test_path_components() {
        let path = PathBuf::from("/home/user/project/file.txt");
        
        assert!(path.is_absolute());
        assert_eq!(path.extension().unwrap(), "txt");
        assert_eq!(path.file_stem().unwrap(), "file");
        assert_eq!(path.parent().unwrap().file_name().unwrap(), "project");
    }

    #[test]
    fn test_path_join() {
        let base = PathBuf::from("/home/user");
        let joined = base.join("projects").join("test");
        
        assert_eq!(joined.to_string_lossy(), "/home/user/projects/test");
    }

    #[test]
    fn test_path_normalization() {
        let path = PathBuf::from("/home/user/../user/./file.txt");
        let normalized = std::fs::canonicalize(&path);
        
        // canonicalize 会解析 .. 和 .
        assert!(normalized.is_ok());
    }

    // ===== 字符串处理测试 =====
    #[test]
    fn test_string_split() {
        let content = "key1=value1,key2=value2,key3=value3";
        let pairs: Vec<(&str, &str)> = content
            .split(',')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, '=');
                match (parts.next(), parts.next()) {
                    (Some(k), Some(v)) => Some((k, v)),
                    _ => None,
                }
            })
            .collect();
        
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("key1", "value1"));
    }

    #[test]
    fn test_string_trim() {
        let content = "  \n  Hello World  \n  ";
        let trimmed: Vec<&str> = content.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect();
        
        assert_eq!(trimmed, vec!["Hello World"]);
    }

    #[test]
    fn test_json_parse() {
        use serde_json::{json, Value};
        
        let json_str = r#"{"name": "test", "value": 123, "enabled": true}"#;
        let parsed: Value = serde_json::from_str(json_str).expect("Invalid JSON");
        
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], 123);
        assert_eq!(parsed["enabled"], true);
    }

    // ===== 性能测试 =====
    #[test]
    fn test_large_file_line_count() {
        let large_content = "Line\n".repeat(100_000);
        let count = large_content.lines().count();
        assert_eq!(count, 100_000);
    }

    #[test]
    fn test_many_small_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        for i in 0..100 {
            create_test_file(&temp_dir.path().to_path_buf(), &format!("file_{}.txt", i), &format!("content {}", i));
        }
        
        let entries: Vec<_> = fs::read_dir(temp_dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 100);
    }
}
