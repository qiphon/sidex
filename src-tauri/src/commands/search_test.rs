#[cfg(test)]
mod search_tests {
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    // 辅助函数：创建测试文件
    fn create_test_file(dir: &PathBuf, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).expect("Failed to create test file");
        path
    }

    // 3.5.1 文件搜索测试
    #[test]
    fn test_search_files_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create test files
        create_test_file(&temp_dir.path().to_path_buf(), "main.ts", "console.log('hello');");
        create_test_file(&temp_dir.path().to_path_buf(), "utils.ts", "export function helper() {}");
        create_test_file(&temp_dir.path().to_path_buf(), "README.md", "# Project");
        
        // Search for TypeScript files
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                if path.extension().map(|ext| ext == "ts").unwrap_or(false) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|p| p.file_name().unwrap() == "main.ts"));
        assert!(results.iter().any(|p| p.file_name().unwrap() == "utils.ts"));
    }

    #[test]
    fn test_search_files_fuzzy() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "App.tsx", "function App() {}");
        create_test_file(&temp_dir.path().to_path_buf(), "application.py", "def app(): pass");
        create_test_file(&temp_dir.path().to_path_buf(), "main.rs", "fn main() {}");
        
        // Fuzzy search for "app"
        let query = "app";
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let file_name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                if file_name.contains(&query.to_lowercase()) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert_eq!(results.len(), 2); // App.tsx and application.py
    }

    #[test]
    fn test_search_files_case_sensitive() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "MyClass.ts", "class MyClass {}");
        create_test_file(&temp_dir.path().to_path_buf(), "myclass.ts", "class myclass {}");
        
        // Case-sensitive search
        let query = "MyClass";
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let file_name = path.file_name().unwrap().to_string_lossy();
                if file_name.contains(query) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert_eq!(results.len(), 1);
        assert!(results[0].file_name().unwrap() == "MyClass.ts");
    }

    #[test]
    fn test_search_files_case_insensitive() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "MyClass.ts", "class MyClass {}");
        create_test_file(&temp_dir.path().to_path_buf(), "myclass.ts", "class myclass {}");
        
        // Case-insensitive search
        let query = "myclass";
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let file_name = path.file_name().unwrap().to_string_lossy().to_lowercase();
                if file_name.contains(&query.to_lowercase()) {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_files_with_wildcard() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "config.dev.json", "{}");
        create_test_file(&temp_dir.path().to_path_buf(), "config.prod.json", "{}");
        create_test_file(&temp_dir.path().to_path_buf(), "app.config.js", "{}");
        
        // Pattern: config*.json
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let file_name = path.file_name().unwrap().to_string_lossy();
                if file_name.starts_with("config") && file_name.ends_with(".json") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_files_no_results() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "main.ts", "console.log('hello');");
        
        // Search for non-existent pattern
        let results: Vec<PathBuf> = fs::read_dir(temp_dir.path())
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                let file_name = path.file_name().unwrap().to_string_lossy();
                if file_name.contains("nonexistent") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        
        assert!(results.is_empty());
    }

    // 3.5.2 文本搜索测试
    #[test]
    fn test_search_text_basic() {
        let content = "Hello world!\nThis is a test file.\nSearch for 'test'.";
        let query = "test";
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.to_lowercase().contains(&query.to_lowercase()))
            .collect();
        
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_search_text_regex() {
        use regex::Regex;
        
        let content = "Line 1: 123\nLine 2: 456\nLine 3: 789";
        let pattern = r"\d+"; // Match numbers
        
        let re = Regex::new(pattern).expect("Invalid regex");
        let matches: Vec<&str> = content.lines()
            .filter(|line| re.is_match(line))
            .collect();
        
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_search_text_multiline() {
        use regex::Regex;
        
        let content = "Start\ntarget\nEnd";
        let pattern = r"Start.*End";
        
        let re = Regex::new(pattern).expect("Invalid regex");
        let result = re.is_match(content);
        
        assert!(!result); // Without DOTALL flag, . doesn't match newlines
        
        let re_multiline = Regex::new(r"(?s)Start.*End").expect("Invalid regex");
        let result_multiline = re_multiline.is_match(content);
        
        assert!(result_multiline);
    }

    #[test]
    fn test_search_text_whole_word() {
        let content = "testing test tester testy";
        let query = "test";
        
        // Whole word search using word boundaries
        let pattern = format!(r"\b{}\b", query);
        let re = regex::Regex::new(&pattern).expect("Invalid regex");
        
        let matches: Vec<&str> = content.split_whitespace()
            .filter(|word| re.is_match(word))
            .collect();
        
        assert_eq!(matches, vec!["test"]);
    }

    #[test]
    fn test_search_text_case_sensitive() {
        let content = "Hello World\nhello world\nHELLO WORLD";
        let query = "Hello";
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.contains(query))
            .collect();
        
        assert_eq!(matches.len(), 1); // Only the first line matches
    }

    #[test]
    fn test_search_text_with_context() {
        let content = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        let query = "Line 3";
        
        let lines: Vec<&str> = content.lines().collect();
        let mut context_lines: Vec<&str> = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if line.contains(query) {
                // Add previous line if exists
                if i > 0 {
                    context_lines.push(lines[i - 1]);
                }
                // Add matching line
                context_lines.push(line);
                // Add next line if exists
                if i < lines.len() - 1 {
                    context_lines.push(lines[i + 1]);
                }
            }
        }
        
        assert_eq!(context_lines, vec!["Line 2", "Line 3", "Line 4"]);
    }

    // 3.5.3 工作区搜索测试
    #[test]
    fn test_search_workspace_basic() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create nested structure
        fs::create_dir(temp_dir.path().join("src")).expect("Failed to create src");
        create_test_file(&temp_dir.path().to_path_buf(), "src/main.ts", "function test() {}");
        create_test_file(&temp_dir.path().to_path_buf(), "README.md", "Project test");
        
        let query = "test";
        let mut results: Vec<String> = Vec::new();
        
        // Walk directory recursively
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains(query) {
                        results.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }
        
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_workspace_exclude() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create directories
        fs::create_dir(temp_dir.path().join("src")).expect("Failed to create src");
        fs::create_dir(temp_dir.path().join("node_modules")).expect("Failed to create node_modules");
        
        create_test_file(&temp_dir.path().to_path_buf(), "src/main.ts", "import lodash");
        create_test_file(&temp_dir.path().to_path_buf(), "node_modules/lodash.js", "export default {}");
        
        let query = "lodash";
        let exclude_dirs = ["node_modules"];
        let mut results: Vec<String> = Vec::new();
        
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry.unwrap();
            
            // Skip excluded directories
            let mut should_skip = false;
            for exclude in &exclude_dirs {
                if entry.path().components().any(|c| c.as_os_str() == *exclude) {
                    should_skip = true;
                    break;
                }
            }
            if should_skip {
                continue;
            }
            
            if entry.path().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains(query) {
                        results.push(entry.path().to_string_lossy().to_string());
                    }
                }
            }
        }
        
        assert_eq!(results.len(), 1); // Should only find main.ts, not lodash.js
    }

    #[test]
    fn test_search_workspace_include() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        create_test_file(&temp_dir.path().to_path_buf(), "main.ts", "typescript code");
        create_test_file(&temp_dir.path().to_path_buf(), "utils.js", "javascript code");
        create_test_file(&temp_dir.path().to_path_buf(), "README.md", "markdown text");
        
        let query = "code";
        let include_patterns = ["*.ts"];
        let mut results: Vec<String> = Vec::new();
        
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry.unwrap();
            
            if entry.path().is_file() {
                // Check if file matches include patterns
                let mut should_include = false;
                for pattern in &include_patterns {
                    if let Some(ext) = entry.path().extension() {
                        if pattern == &format!("*.{}", ext.to_string_lossy()) {
                            should_include = true;
                            break;
                        }
                    }
                }
                
                if should_include {
                    if let Ok(content) = fs::read_to_string(entry.path()) {
                        if content.contains(query) {
                            results.push(entry.path().to_string_lossy().to_string());
                        }
                    }
                }
            }
        }
        
        assert_eq!(results.len(), 1); // Should only find main.ts
    }

    #[test]
    fn test_search_workspace_replace_preview() {
        let content = "Hello world\nThis is a test\nWorld is big";
        let search_query = "World";
        let replace_query = "Universe";
        
        let lines: Vec<String> = content.lines()
            .map(|line| {
                if line.contains(search_query) {
                    line.replace(search_query, replace_query)
                } else {
                    line.to_string()
                }
            })
            .collect();
        
        let result = lines.join("\n");
        assert!(result.contains("Universe"));
        assert!(!result.contains("World"));
    }

    #[test]
    fn test_search_workspace_replace_apply() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let path = create_test_file(&temp_dir.path().to_path_buf(), "test.txt", "Hello world\nWorld is beautiful");
        
        let content = fs::read_to_string(&path).unwrap();
        let search_query = "World";
        let replace_query = "Universe";
        
        let replaced = content.replace(search_query, replace_query);
        fs::write(&path, replaced).expect("Failed to write replaced content");
        
        let final_content = fs::read_to_string(&path).unwrap();
        assert!(final_content.contains("Universe"));
        assert!(!final_content.contains("World"));
    }

    // 3.5.4 搜索选项测试
    #[test]
    fn test_search_options_max_results() {
        let content = "test\ntest\ntest\ntest\ntest"; // 5 matches
        let query = "test";
        let max_results = 3;
        
        let matches: Vec<&str> = content.lines()
            .filter(|line| line.contains(query))
            .take(max_results)
            .collect();
        
        assert_eq!(matches.len(), max_results);
    }

    #[test]
    fn test_search_options_binary_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create binary file (PNG header)
        let binary_content = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        let path = temp_dir.path().join("image.png");
        fs::write(&path, &binary_content).expect("Failed to write binary file");
        
        // Create text file
        create_test_file(&temp_dir.path().to_path_buf(), "text.txt", "This is text");
        
        let query = "text";
        let mut text_results = 0;
        let mut binary_results = 0;
        
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                match fs::read(entry.path()) {
                    Ok(bytes) => {
                        // Simple binary detection: check for null bytes
                        if bytes.contains(&0x00) {
                            binary_results += 1;
                        } else {
                            if let Ok(content) = String::from_utf8(bytes) {
                                if content.contains(query) {
                                    text_results += 1;
                                }
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        
        assert_eq!(text_results, 1);
        assert_eq!(binary_results, 1);
    }

    // 3.5.5 性能测试
    #[test]
    fn test_search_performance_large_files() {
        let large_content = "test line\n".repeat(1_000_000); // 1 million lines
        let query = "test";
        
        let start = std::time::Instant::now();
        let matches: Vec<&str> = large_content.lines()
            .filter(|line| line.contains(query))
            .collect();
        let duration = start.elapsed();
        
        assert_eq!(matches.len(), 1_000_000);
        assert!(duration.as_secs() < 5); // Should complete in under 5 seconds
    }

    #[test]
    fn test_search_performance_many_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        
        // Create 1000 files
        for i in 0..1000 {
            create_test_file(&temp_dir.path().to_path_buf(), &format!("file_{}.txt", i), "content with test");
        }
        
        let query = "test";
        let start = std::time::Instant::now();
        
        let mut results = 0;
        for entry in walkdir::WalkDir::new(temp_dir.path()) {
            let entry = entry.unwrap();
            if entry.path().is_file() {
                if let Ok(content) = fs::read_to_string(entry.path()) {
                    if content.contains(query) {
                        results += 1;
                    }
                }
            }
        }
        
        let duration = start.elapsed();
        
        assert_eq!(results, 1000);
        assert!(duration.as_secs() < 10); // Should complete in under 10 seconds
    }
}
