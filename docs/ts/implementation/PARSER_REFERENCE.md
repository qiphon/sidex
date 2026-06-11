# TypeScript 解析函数参考实现

本文档提供各功能对应的 tsserver 响应解析函数的详细实现代码。

## 重要提示

**验证状态说明**:
- ✅ 已验证: 响应格式已通过实际测试验证
- ⚠️ 待验证: 响应格式基于推测或文档，需要实际验证
- 🔄 简化实现: 功能使用简化方案，可能无法完全匹配 tsserver 行为

**需要验证的 tsserver 命令响应**:
- `typeDefinition` - ⚠️ 待验证
- `format` - ⚠️ 待验证
- `getFoldingRanges` - ⚠️ 待验证
- `navto` - ⚠️ 待验证
- `documentHighlights` - ⚠️ 待验证
- `declaration` - ⚠️ 待验证

**建议**: 在实现前创建测试工具实际调用 tsserver 并捕获响应，更新文档中的格式说明。

---

## 1. 格式化解析函数

### 1.1 parse_ts_formatting

```rust
/// 解析 tsserver format 命令响应
/// 
/// ⚠️ 注意: tsserver format 命令的响应格式需要实际验证
/// 以下格式为推测，可能与实际情况不符
/// 
/// tsserver 响应格式（推测）:
/// {
///   "body": [
///     {
///       "start": { "line": 1, "offset": 1 },
///       "end": { "line": 1, "offset": 1 },
///       "newText": "  "
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<TextEdit>
fn parse_ts_formatting(json: &str) -> Option<Vec<TextEdit>> {
    let mut edits = Vec::new();
    let mut search = json;
    
    // 查找 "start": 字段（每个 TextEdit 的起始标记）
    while let Some(start_pos) = search.find("\"start\":") {
        // 提取起始行和列
        let start_line = extract_field_from_str(&search[start_pos..], "line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let start_col = extract_field_from_str(&search[start_pos..], "offset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        
        // 提取结束行和列
        let end_line = extract_field_from_str(&search[start_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|l| l.saturating_sub(1))
            .unwrap_or(start_line);
        let end_col = extract_field_from_str(&search[start_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1))
            .unwrap_or(start_col);
        
        // 提取新文本内容
        let new_text = extract_field_from_str(&search[start_pos..], "newText")
            .unwrap_or_default();
        
        edits.push(TextEdit {
            range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: end_line, character: end_col },
            },
            new_text,
        });
        
        // 移动搜索位置，避免无限循环
        search = &search[start_pos + 7..];
    }
    
    if edits.is_empty() {
        None
    } else {
        Some(edits)
    }
}
```

### 1.2 parse_ts_range_formatting

```rust
/// 解析 tsserver range format 命令响应
/// 与 parse_ts_formatting 相同，但需要处理 range 格式
fn parse_ts_range_formatting(json: &str) -> Option<Vec<TextEdit>> {
    parse_ts_formatting(json) // 复用相同逻辑
}
```

## 2. 类型定义解析函数

### 2.1 parse_ts_type_definition

```rust
/// 解析 tsserver typeDefinition 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "file": "/path/to/types.ts",
///       "start": { "line": 5, "offset": 1 },
///       "end": { "line": 5, "offset": 20 }
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<Location>
fn parse_ts_type_definition(json: &str) -> Option<Vec<Location>> {
    // 复用 parse_ts_locations 的逻辑
    parse_ts_locations(json)
}
```

## 3. 实现跳转解析函数

### 3.1 parse_ts_implementations

```rust
/// 解析 tsserver implementation 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "file": "/path/to/implementation.ts",
///       "start": { "line": 10, "offset": 4 },
///       "end": { "line": 10, "offset": 15 }
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<Location>
fn parse_ts_implementations(json: &str) -> Option<Vec<Location>> {
    // 复用 parse_ts_locations 的逻辑
    parse_ts_locations(json)
}
```

## 4. 代码折叠解析函数

### 4.1 parse_ts_folding_ranges

```rust
/// 解析 tsserver getFoldingRanges 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "startLine": 5,
///       "startOffset": 0,
///       "endLine": 20,
///       "endOffset": 1,
///       "kind": "region"
///     },
///     {
///       "startLine": 10,
///       "startOffset": 4,
///       "endLine": 15,
///       "endOffset": 5,
///       "kind": "function"
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<FoldingRange>
fn parse_ts_folding_ranges(json: &str) -> Option<Vec<FoldingRange>> {
    let mut ranges = Vec::new();
    let mut search = json;
    
    while let Some(start_pos) = search.find("\"startLine\":") {
        // 提取起始行
        let start_line = extract_field_from_str(&search[start_pos..], "startLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        
        // 提取结束行
        let end_line = extract_field_from_str(&search[start_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        
        // 提取起始列（可选）
        let start_col = extract_field_from_str(&search[start_pos..], "startOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1));
        
        // 提取结束列（可选）
        let end_col = extract_field_from_str(&search[start_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1));
        
        // 提取折叠类型
        let kind_str = extract_field_from_str(&search[start_pos..], "kind")
            .unwrap_or_default();
        
        // LSP FoldingRangeKind:
        // 0 = not foldable
        // 1 = comment
        // 2 = imports
        // 3 = region
        // 4 = function (扩展)
        // 5 = class (扩展)
        let kind = match kind_str.as_str() {
            "comment" => Some(1),
            "import" | "imports" => Some(2),
            "region" => Some(3),
            "function" => Some(4),
            "class" => Some(5),
            _ => None,
        };
        
        ranges.push(FoldingRange {
            start_line,
            end_line,
            start_column: start_col,
            end_column: end_col,
            kind,
            // 其他可选字段使用默认值
            collapsed_text: None,
        });
        
        search = &search[start_pos + 11..];
    }
    
    if ranges.is_empty() {
        None
    } else {
        Some(ranges)
    }
}
```

## 5. 文档高亮解析函数

### 5.1 parse_ts_document_highlights

```rust
/// 解析 tsserver documentHighlights 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "file": "/path/to/file.ts",
///       "refs": [
///         {
///           "start": { "line": 5, "offset": 4 },
///           "end": { "line": 5, "offset": 10 },
///           "type": "reference"
///         }
///       ]
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<DocumentHighlight>
fn parse_ts_document_highlights(json: &str) -> Option<Vec<DocumentHighlight>> {
    let mut highlights = Vec::new();
    let mut search = json;
    
    while let Some(file_pos) = search.find("\"file\":") {
        // 提取文件路径
        let _file = extract_string_value(&search[file_pos + 7..])?;
        
        // 查找 "refs": 数组
        let refs_start = search[file_pos..].find("\"refs\":")?;
        let arr_start = search[file_pos + refs_start..].find('[')? + file_pos + refs_start;
        
        let mut refs_search = &search[arr_start..];
        
        while let Some(ref_pos) = refs_search.find("\"start\":") {
            // 提取起始位置
            let start_line = extract_field_from_str(refs_search, "line")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .saturating_sub(1);
            let start_col = extract_field_from_str(refs_search, "offset")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(1)
                .saturating_sub(1);
            
            // 提取结束位置
            let end_line = extract_field_from_str(refs_search, "endLine")
                .and_then(|s| s.parse::<u32>().ok())
                .map(|l| l.saturating_sub(1))
                .unwrap_or(start_line);
            let end_col = extract_field_from_str(refs_search, "endOffset")
                .and_then(|s| s.parse::<u32>().ok())
                .map(|c| c.saturating_sub(1))
                .unwrap_or(start_col);
            
            // 提取高亮类型
            let type_str = extract_field_from_str(refs_search, "type")
                .unwrap_or_default();
            
            // DocumentHighlightKind:
            // 0 = Text (默认)
            // 1 = Read
            // 2 = Write
            let kind = match type_str.as_str() {
                "definition" => Some(1),
                "reference" | "implicit" => Some(0),
                "written" => Some(2),
                _ => Some(0),
            };
            
            highlights.push(DocumentHighlight {
                range: Range {
                    start: Position { line: start_line, character: start_col },
                    end: Position { line: end_line, character: end_col },
                },
                kind,
            });
            
            refs_search = &refs_search[ref_pos + 7..];
        }
        
        search = &search[file_pos + 7..];
    }
    
    if highlights.is_empty() {
        None
    } else {
        Some(highlights)
    }
}
```

## 6. 工作区符号解析函数

### 6.1 parse_ts_workspace_symbols

```rust
/// 解析 tsserver navto 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "name": "MyClass",
///       "kind": "class",
///       "file": "/path/to/types.ts",
///       "start": { "line": 10, "offset": 6 },
///       "end": { "line": 10, "offset": 14 },
///       "containerName": "MyNamespace"
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<DocumentSymbol>
fn parse_ts_workspace_symbols(json: &str) -> Option<Vec<DocumentSymbol>> {
    let mut symbols = Vec::new();
    let mut search = json;
    
    while let Some(name_pos) = search.find("\"name\":") {
        // 提取符号名称
        let name = extract_string_value(&search[name_pos + 6..])?;
        
        // 提取文件路径
        let file = extract_field_from_str(&search[name_pos..], "file")
            .unwrap_or_default();
        
        // 提取位置信息
        let start_line = extract_field_from_str(&search[name_pos..], "line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let start_col = extract_field_from_str(&search[name_pos..], "startOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1))
            .unwrap_or(0);
        let end_col = extract_field_from_str(&search[name_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1))
            .unwrap_or(start_col);
        
        // 提取符号类型
        let kind_str = extract_field_from_str(&search[name_pos..], "kind")
            .unwrap_or_default();
        
        // 提取容器名称（命名空间/类）
        let detail = extract_field_from_str(&search[name_pos..], "containerName");
        
        symbols.push(DocumentSymbol {
            name,
            detail,
            kind: ts_kind_to_symbol_kind(&kind_str),
            range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: start_line, character: end_col },
            },
            selection_range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: start_line, character: end_col },
            },
            children: vec![],
            tags: vec![],
            deprecated: None,
            uri: if file.is_empty() { None } else { Some(format!("file://{file}")) },
        });
        
        search = &search[name_pos + 6..];
    }
    
    if symbols.is_empty() {
        None
    } else {
        Some(symbols)
    }
}
```

## 7. 语义标记解析函数

### 7.1 parse_semantic_tokens_from_navtree

```rust
/// 从 navtree 响应解析语义标记
/// 
/// 注意: tsserver 不直接返回 semantic tokens
/// 使用 navtree 命令获取符号信息，然后转换为 semantic tokens
/// 
/// tsserver navtree 响应格式:
/// {
///   "body": {
///     "childItems": [
///       {
///         "name": "function",
///         "kind": "function",
///         "start": { "line": 10, "offset": 0 },
///         "end": { "line": 20, "offset": 1 },
///         "childItems": [...]
///       }
///     ]
///   },
///   "success": true
/// }
/// 
/// 返回: Option<SemanticTokens>
fn parse_semantic_tokens_from_navtree(json: &str) -> Option<SemanticTokens> {
    let mut data: Vec<u32> = Vec::new();
    let mut last_line = 0u32;
    let mut last_start = 0u32;
    
    // 递归解析符号树
    fn extract_tokens(
        json: &str,
        data: &mut Vec<u32>,
        last_line: &mut u32,
        last_start: &mut u32,
    ) {
        let mut search = json;
        
        // 查找 childItems 数组
        if let Some(child_pos) = search.find("\"childItems\":") {
            let arr_start = search[child_pos..].find('[').map(|p| p + child_pos);
            
            if let Some(arr_start) = arr_start {
                // 遍历每个符号
                let mut item_search = &search[arr_start..];
                let depth = 1;
                
                while let Some(name_pos) = item_search.find("\"name\":") {
                    // 提取符号名称和类型
                    let name = extract_string_value(&item_search[name_pos + 6..]);
                    
                    if let Some(_) = name {
                        // 提取位置信息
                        let start_line = extract_field_from_str(&item_search[name_pos..], "line")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0);
                        
                        let start_col = extract_field_from_str(&item_search[name_pos..], "offset")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(0);
                        
                        let end_line = extract_field_from_str(&item_search[name_pos..], "endLine")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(start_line);
                        
                        let end_col = extract_field_from_str(&item_search[name_pos..], "endOffset")
                            .and_then(|s| s.parse::<u32>().ok())
                            .unwrap_or(start_col);
                        
                        let kind_str = extract_field_from_str(&item_search[name_pos..], "kind")
                            .unwrap_or_default();
                        
                        // 将符号类型转换为 token 类型
                        let token_type = ts_kind_to_token_type(&kind_str);
                        
                        // 计算 delta 值
                        let delta_line = start_line - *last_line;
                        let delta_start = if delta_line == 0 {
                            start_col - *last_start
                        } else {
                            start_col
                        };
                        
                        let length = if start_line == end_line {
                            end_col - start_col
                        } else {
                            // 多行符号，这里简化处理
                            0
                        };
                        
                        // 添加 token 数据: deltaLine, deltaStart, length, tokenType, tokenModifiers
                        data.push(delta_line);
                        data.push(delta_start);
                        data.push(length);
                        data.push(token_type);
                        data.push(0); // tokenModifiers: none
                        
                        *last_line = start_line;
                        *last_start = start_col;
                    }
                    
                    item_search = &item_search[name_pos + 6..];
                }
            }
        }
    }
    
    extract_tokens(json, &mut data, &mut last_line, &mut last_start);
    
    if data.is_empty() {
        None
    } else {
        Some(SemanticTokens {
            result_id: None,
            data,
        })
    }
}

/// 将 tsserver 的 kind 映射到 LSP token 类型
fn ts_kind_to_token_type(kind: &str) -> u32 {
    match kind {
        "class" => 5,        // class
        "enum" => 13,        // enum
        "interface" => 7,    // interface
        "namespace" => 3,    // namespace
        "type alias" => 22,  // type
        "type" => 22,
        "function" | "method" => 12,  // function
        "var" | "let" | "const" => 0, // variable
        "property" => 8,     // property
        "parameter" => 1,    // parameter
        "constructor" => 9,  // constructor
        _ => 0,              // 默认为 variable
    }
}
```

## 8. 代码透镜解析函数

### 8.1 parse_ts_code_lenses

```rust
/// 解析引用信息为代码透镜
/// 
/// 实现思路:
/// 1. 调用 references 获取所有引用位置
/// 2. 按文件/函数分组
/// 3. 生成显示引用数量的 CodeLens
/// 
/// 返回: Vec<CodeLens>
fn parse_ts_code_lenses(json: &str) -> Option<Vec<CodeLens>> {
    let locations = parse_ts_locations(json)?;
    
    // 按文件分组统计引用数量
    use std::collections::HashMap;
    let mut file_refs: HashMap<String, Vec<Range>> = HashMap::new();
    
    for loc in locations {
        let uri = loc.uri.clone();
        let range = loc.range;
        file_refs.entry(uri).or_insert_with(Vec::new).push(range);
    }
    
    let mut lenses = Vec::new();
    
    // 为每个唯一位置生成 CodeLens
    let mut seen_ranges: HashMap<String, Vec<Range>> = HashMap::new();
    
    for loc in locations {
        let key = format!("{}:{}:{}", loc.uri, loc.range.start.line, loc.range.start.character);
        
        // 去重
        let seen = seen_ranges.entry(loc.uri.clone()).or_insert_with(Vec::new);
        let is_dup = seen.iter().any(|r| 
            r.start.line == loc.range.start.line && 
            r.start.character == loc.range.start.character
        );
        
        if !is_dup {
            seen.push(loc.range.clone());
            
            let count = file_refs.get(&loc.uri)
                .map(|v| v.len())
                .unwrap_or(1);
            
            lenses.push(CodeLens {
                range: loc.range,
                command: None,
                data: Some(format!("{} references", count)),
            });
        }
    }
    
    if lenses.is_empty() {
        None
    } else {
        Some(lenses)
    }
}
```

## 9. 声明解析函数

### 9.1 parse_ts_declaration

```rust
/// 解析 tsserver declaration 命令响应
/// 
/// tsserver 响应格式:
/// {
///   "body": [
///     {
///       "file": "/path/to/file.ts",
///       "start": { "line": 5, "offset": 1 },
///       "end": { "line": 5, "offset": 20 }
///     }
///   ],
///   "success": true
/// }
/// 
/// 返回: Vec<Location>
fn parse_ts_declaration(json: &str) -> Option<Vec<Location>> {
    // 复用 parse_ts_locations 的逻辑
    parse_ts_locations(json)
}
```

## 10. 辅助函数扩展

### 10.1 parse_ts_locations 的完整实现

```rust
/// 解析包含 file/line/offset 格式的 Location 数组
/// 这是最常用的解析函数，被多个功能复用
fn parse_ts_locations(json: &str) -> Option<Vec<Location>> {
    let mut locs = Vec::new();
    let mut search = json;
    
    while let Some(file_pos) = search.find("\"file\":") {
        // 提取文件路径
        let after = &search[file_pos + 7..];
        let file = extract_string_value(after)?;
        
        // 提取起始位置
        let start_line = extract_field_from_str(&search[file_pos..], "line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let start_col = extract_field_from_str(&search[file_pos..], "offset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        
        // 提取结束位置（可选）
        let end_line = extract_field_from_str(&search[file_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|l| l.saturating_sub(1))
            .unwrap_or(start_line);
        let end_col = extract_field_from_str(&search[file_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .map(|c| c.saturating_sub(1))
            .unwrap_or(start_col);
        
        locs.push(Location {
            uri: format!("file://{file}"),
            range: Range {
                start: Position {
                    line: start_line,
                    character: start_col,
                },
                end: Position {
                    line: end_line,
                    character: end_col,
                },
            },
        });
        
        search = &search[file_pos + 7..];
    }
    
    if locs.is_empty() {
        None
    } else {
        Some(locs)
    }
}
```

## 11. 错误处理

### 11.1 健壮性建议

所有解析函数应遵循以下错误处理原则：

1. **使用 `and_then` 链式处理**: 避免嵌套的 match
2. **提供默认值**: 使用 `unwrap_or_default()` 或 `unwrap_or()`
3. **日志记录**: 解析失败时记录诊断信息
4. **边界检查**: 确保索引不越界

```rust
fn safe_extract_field(json: &str, field: &str) -> Option<String> {
    // 确保 field 存在
    if !json.contains(&format!("\"{}\"", field)) {
        return None;
    }
    extract_field_from_str(json, field)
}
```

## 12. 测试用例

### 12.1 parse_ts_formatting 测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_single_edit() {
        let json = r#"{"body":[{"start":{"line":1,"offset":1},"end":{"line":1,"offset":1},"newText":"  "}],"success":true}"#;
        let result = parse_ts_formatting(json);
        assert!(result.is_some());
        let edits = result.unwrap();
        assert_eq!(edits.len(), 1);
        assert_eq!(edits[0].new_text, "  ");
    }
    
    #[test]
    fn test_format_multiple_edits() {
        let json = r#"{
            "body": [
                {"start":{"line":1,"offset":1},"end":{"line":1,"offset":1},"newText":"  "},
                {"start":{"line":2,"offset":5},"end":{"line":2,"offset":5},"newText":"const "}
            ],
            "success": true
        }"#;
        let result = parse_ts_formatting(json);
        assert!(result.is_some());
        let edits = result.unwrap();
        assert_eq!(edits.len(), 2);
    }
    
    #[test]
    fn test_format_empty_response() {
        let json = r#"{"body":[],"success":true}"#;
        let result = parse_ts_formatting(json);
        assert!(result.is_none());
    }
}
```
