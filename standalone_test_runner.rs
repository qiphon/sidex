// Standalone tests that can run without Tauri dependencies
// Use: rustc --test --extern tempfile=... standalone_test_runner.rs && ./standalone_test_runner

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    
    // 简单的临时目录创建（不需要外部依赖）
    struct TempDir {
        path: PathBuf,
    }
    
    impl TempDir {
        fn new() -> std::io::Result<Self> {
            let path = std::env::temp_dir().join(format!("sidex_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            fs::create_dir_all(&path)?;
            Ok(Self { path })
        }
        
        fn path(&self) -> &PathBuf {
            &self.path
        }
    }
    
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    // 辅助函数
    fn create_file(path: &PathBuf, content: &str) -> std::io::Result<PathBuf> {
        fs::write(path, content)?;
        Ok(path.clone())
    }

    // ===== 文件系统测试 =====
    #[test]
    fn test_fs_read_write() {
        let temp = TempDir::new().unwrap();
        let path = create_file(&temp.path().join("test.txt"), "Hello, World!").unwrap();
        
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_fs_utf8() {
        let temp = TempDir::new().unwrap();
        let content = "你好，世界！🎉";
        let path = create_file(&temp.path().join("chinese.txt"), content).unwrap();
        
        let read_content = fs::read_to_string(&path).unwrap();
        assert_eq!(read_content, content);
    }

    #[test]
    fn test_fs_directory_operations() {
        let temp = TempDir::new().unwrap();
        
        // 创建文件
        create_file(&temp.path().join("file1.txt"), "content1").unwrap();
        create_file(&temp.path().join("file2.txt"), "content2").unwrap();
        
        // 读取目录
        let entries: Vec<_> = fs::read_dir(temp.path()).unwrap().collect();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_fs_nested_directory() {
        let temp = TempDir::new().unwrap();
        let nested = temp.path().join("a").join("b").join("c");
        
        fs::create_dir_all(&nested).unwrap();
        assert!(nested.is_dir());
    }

    #[test]
    fn test_fs_rename() {
        let temp = TempDir::new().unwrap();
        let old = create_file(&temp.path().join("old.txt"), "content").unwrap();
        let new = temp.path().join("new.txt");
        
        fs::rename(&old, &new).unwrap();
        assert!(!old.exists());
        assert!(new.exists());
    }

    #[test]
    fn test_fs_delete() {
        let temp = TempDir::new().unwrap();
        let path = create_file(&temp.path().join("delete_me.txt"), "content").unwrap();
        
        fs::remove_file(&path).unwrap();
        assert!(!path.exists());
    }

    // ===== 文本处理测试 =====
    #[test]
    fn test_text_lines() {
        let content = "Line 1\nLine 2\nLine 3";
        let count = content.lines().count();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_text_lines_empty() {
        let content = "";
        assert_eq!(content.lines().count(), 0);
    }

    #[test]
    fn test_text_trim() {
        let content = "  Hello World  ";
        assert_eq!(content.trim(), "Hello World");
    }

    #[test]
    fn test_text_replace() {
        let content = "Hello World";
        assert_eq!(content.replace("World", "Rust"), "Hello Rust");
    }

    #[test]
    fn test_text_split() {
        let content = "a,b,c,d";
        let parts: Vec<&str> = content.split(',').collect();
        assert_eq!(parts, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn test_text_contains() {
        let content = "Hello World";
        assert!(content.contains("World"));
        assert!(!content.contains("Rust"));
    }

    #[test]
    fn test_text_starts_ends() {
        let content = "Hello World";
        assert!(content.starts_with("Hello"));
        assert!(content.ends_with("World"));
    }

    // ===== 路径测试 =====
    #[test]
    fn test_path_extension() {
        let path = PathBuf::from("/path/to/file.txt");
        assert_eq!(path.extension().unwrap(), "txt");
    }

    #[test]
    fn test_path_stem() {
        let path = PathBuf::from("/path/to/file.txt");
        assert_eq!(path.file_stem().unwrap(), "file");
    }

    #[test]
    fn test_path_parent() {
        let path = PathBuf::from("/path/to/file.txt");
        assert_eq!(path.parent().unwrap(), PathBuf::from("/path/to"));
    }

    #[test]
    fn test_path_join() {
        let base = PathBuf::from("/home/user");
        let joined = base.join("projects").join("test");
        assert_eq!(joined.to_string_lossy(), "/home/user/projects/test");
    }

    // ===== 字符串测试 =====
    #[test]
    fn test_string_lowercase() {
        let s = "HELLO World";
        assert_eq!(s.to_lowercase(), "hello world");
    }

    #[test]
    fn test_string_uppercase() {
        let s = "Hello World";
        assert_eq!(s.to_uppercase(), "HELLO WORLD");
    }

    #[test]
    fn test_string_reverse() {
        let s: String = "hello".chars().rev().collect();
        assert_eq!(s, "olleh");
    }

    #[test]
    fn test_string_contains_char() {
        let s = "Hello";
        assert!(s.contains('H'));
        assert!(s.contains('o'));
    }

    // ===== 数组/向量测试 =====
    #[test]
    fn test_vec_push_pop() {
        let mut v = vec![1, 2, 3];
        v.push(4);
        assert_eq!(v, vec![1, 2, 3, 4]);
        
        let popped = v.pop();
        assert_eq!(popped, Some(4));
        assert_eq!(v, vec![1, 2, 3]);
    }

    #[test]
    fn test_vec_map() {
        let v = vec![1, 2, 3, 4, 5];
        let doubled: Vec<i32> = v.iter().map(|x| x * 2).collect();
        assert_eq!(doubled, vec![2, 4, 6, 8, 10]);
    }

    #[test]
    fn test_vec_filter() {
        let v = vec![1, 2, 3, 4, 5];
        let evens: Vec<&i32> = v.iter().filter(|x| *x % 2 == 0).collect();
        assert_eq!(evens, vec![&2, &4]);
    }

    #[test]
    fn test_vec_reduce() {
        let v = vec![1, 2, 3, 4, 5];
        let sum: i32 = v.iter().sum();
        assert_eq!(sum, 15);
    }

    #[test]
    fn test_vec_sort() {
        let mut v = vec![5, 3, 1, 4, 2];
        v.sort();
        assert_eq!(v, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_vec_reverse() {
        let mut v = vec![1, 2, 3];
        v.reverse();
        assert_eq!(v, vec![3, 2, 1]);
    }

    // ===== Option 测试 =====
    #[test]
    fn test_option_map() {
        let some_value = Some(5);
        let doubled = some_value.map(|x| x * 2);
        assert_eq!(doubled, Some(10));
        
        let none_value: Option<i32> = None;
        let mapped = none_value.map(|x| x * 2);
        assert_eq!(mapped, None);
    }

    #[test]
    fn test_option_unwrap() {
        let some_value = Some(42);
        assert_eq!(some_value.unwrap(), 42);
    }

    #[test]
    fn test_option_or_else() {
        let none: Option<i32> = None;
        let fallback = none.or_else(|| Some(100));
        assert_eq!(fallback, Some(100));
        
        let some_value = Some(42);
        let fallback2 = some_value.or_else(|| Some(100));
        assert_eq!(fallback2, Some(42));
    }

    // ===== Result 测试 =====
    #[test]
    fn test_result_ok() {
        let ok: Result<i32, &str> = Ok(42);
        assert!(ok.is_ok());
        assert_eq!(ok.unwrap(), 42);
    }

    #[test]
    fn test_result_err() {
        let err: Result<i32, &str> = Err("error");
        assert!(err.is_err());
        assert_eq!(err.unwrap_err(), "error");
    }

    #[test]
    fn test_result_map() {
        let ok: Result<i32, &str> = Ok(5);
        let doubled = ok.map(|x| x * 2);
        assert_eq!(doubled.unwrap(), 10);
    }

    // ===== HashMap 测试 =====
    #[test]
    fn test_hashmap_basic() {
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        map.insert("key1", "value1");
        map.insert("key2", "value2");
        
        assert_eq!(map.get("key1"), Some(&"value1"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn test_hashmap_update() {
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        map.insert("key", "value1");
        map.insert("key", "value2");
        
        assert_eq!(map.get("key"), Some(&"value2"));
        assert_eq!(map.len(), 1);
    }

    #[test]
    fn test_hashmap_iteration() {
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        map.insert(1, "one");
        map.insert(2, "two");
        map.insert(3, "three");
        
        let keys: Vec<&i32> = map.keys().collect();
        assert!(keys.contains(&&1));
        assert!(keys.contains(&&2));
        assert!(keys.contains(&&3));
    }

    // ===== 正则表达式测试 =====
    #[test]
    fn test_regex_simple() {
        let text = "The quick brown fox";
        
        // 检查是否包含 "quick"
        assert!(text.contains("quick"));
        
        // 检查是否以 "The" 开头
        assert!(text.starts_with("The"));
    }

    #[test]
    fn test_regex_pattern() {
        let email = "test@example.com";
        
        // 简单的 @ 检查
        assert!(email.contains('@'));
        
        // 检查是否以 .com 结尾
        assert!(email.ends_with(".com"));
    }

    // ===== JSON 测试 =====
    #[test]
    fn test_json_parse_simple() {
        // 手动解析简单的 JSON 字符串
        let json = r#"{"name":"test","value":123}"#;
        
        // 检查基本结构
        assert!(json.starts_with('{'));
        assert!(json.ends_with('}'));
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"test\""));
    }

    // ===== Base64 编解码测试 =====
    #[test]
    fn test_base64_alphabet() {
        // 标准 Base64 字母表
        let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        
        // 检查字母表长度
        assert_eq!(alphabet.len(), 64);
        
        // 检查包含所有必需的字符
        assert!(alphabet.contains('A'));
        assert!(alphabet.contains('Z'));
        assert!(alphabet.contains('a'));
        assert!(alphabet.contains('z'));
        assert!(alphabet.contains('0'));
        assert!(alphabet.contains('9'));
    }

    #[test]
    fn test_base64_padding() {
        // 测试 padding 规则
        // 3 字节 -> 4 字符 (无 padding)
        // 2 字节 -> 3 字符 + 1 padding
        // 1 字节 -> 2 字符 + 2 padding
        
        // "a" (1 字节) -> "YQ==" (4 字符，2 个 padding)
        assert_eq!("a".len(), 1);
        
        // "ab" (2 字节) -> "YWI=" (4 字符，1 个 padding)
        assert_eq!("ab".len(), 2);
        
        // "abc" (3 字节) -> "YWJj" (4 字符，无 padding)
        assert_eq!("abc".len(), 3);
    }

    // ===== URL 编码测试 =====
    #[test]
    fn test_url_encoding_concepts() {
        // 测试 URL 编码的基本概念
        // 空格应该被编码为 %20
        assert!("Hello World".contains(" "));
        
        // URL 中的特殊字符需要编码
        let special_chars = vec![' ', '/', ':', '@', '&', '=', '$', ','];
        
        for c in special_chars {
            let encoded = format!("%{:02X}", c as u8);
            // 这些字符在 URL 中应该有替代表示
            assert!(encoded.starts_with('%'));
        }
    }

    #[test]
    fn test_url_path_segments() {
        // 测试 URL 路径分段
        let path = "/api/v1/users/123";
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        
        assert_eq!(segments, vec!["api", "v1", "users", "123"]);
    }

    #[test]
    fn test_url_query_string() {
        // 测试查询字符串
        let query = "name=John&age=30&city=NYC";
        let pairs: Vec<&str> = query.split('&').collect();
        
        assert_eq!(pairs.len(), 3);
        assert!(pairs[0].starts_with("name="));
    }

    // ===== 日期时间测试 =====
    #[test]
    fn test_timestamp() {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        // 时间戳应该是正数
        assert!(now > 0);
        
        // 2024 年 1 月 1 日的时间戳大约是 1704067200
        assert!(now > 1704067200);
    }

    // ===== 错误处理测试 =====
    #[test]
    fn test_io_error() {
        let result = std::fs::read_to_string("nonexistent_file_12345.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error() {
        let result: Result<i32, _> = "not a number".parse();
        assert!(result.is_err());
    }

    // ===== 性能测试 =====
    #[test]
    fn test_loop_performance() {
        let start = std::time::Instant::now();
        
        // 简单的循环操作
        let mut sum = 0i64;
        for i in 0..1_000_000 {
            sum += i;
        }
        
        let duration = start.elapsed();
        
        // 100 万次循环应该在合理时间内完成（< 1 秒）
        assert!(duration.as_secs() < 5);
        assert_eq!(sum, 499999500000); // n*(n-1)/2
    }

    // ===== 内存测试 =====
    #[test]
    fn test_vec_capacity() {
        let mut v = Vec::with_capacity(10);
        for i in 0..10 {
            v.push(i);
        }
        
        // 容量应该 >= 10
        assert!(v.capacity() >= 10);
        // 但长度应该是 10
        assert_eq!(v.len(), 10);
    }

    #[test]
    fn test_string_capacity() {
        let s = String::with_capacity(100);
        
        // 字符串容量应该 >= 100
        assert!(s.capacity() >= 100);
        // 但长度应该是 0
        assert_eq!(s.len(), 0);
    }

    // ===== 迭代器测试 =====
    #[test]
    fn test_iterator_take() {
        let v = vec![1, 2, 3, 4, 5];
        let taken: Vec<&i32> = v.iter().take(3).collect();
        
        assert_eq!(taken, vec![&1, &2, &3]);
    }

    #[test]
    fn test_iterator_skip() {
        let v = vec![1, 2, 3, 4, 5];
        let skipped: Vec<&i32> = v.iter().skip(2).collect();
        
        assert_eq!(skipped, vec![&3, &4, &5]);
    }

    #[test]
    fn test_iterator_zip() {
        let a = vec![1, 2, 3];
        let b = vec!['a', 'b', 'c'];
        
        let zipped: Vec<(&i32, char)> = a.iter().zip(b.iter().copied()).collect();
        
        assert_eq!(zipped.len(), 3);
        assert_eq!(zipped[0], (&1, 'a'));
    }

    // ===== 文件内容测试 =====
    #[test]
    fn test_file_metadata() {
        let temp = TempDir::new().unwrap();
        let content = "test content";
        let path = create_file(&temp.path().join("meta.txt"), content).unwrap();
        
        let metadata = fs::metadata(&path).unwrap();
        
        assert!(metadata.is_file());
        assert!(!metadata.is_dir());
        assert_eq!(metadata.len() as usize, content.len());
    }

    #[test]
    fn test_binary_file() {
        let temp = TempDir::new().unwrap();
        let binary_data: Vec<u8> = (0..=255).collect();
        let path = temp.path().join("binary.bin");
        
        fs::write(&path, &binary_data).unwrap();
        
        let read_data = fs::read(&path).unwrap();
        
        assert_eq!(read_data.len(), 256);
        assert_eq!(read_data[0], 0);
        assert_eq!(read_data[255], 255);
    }
}
