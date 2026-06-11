# TypeScript 语言服务技术实现规格

本文档详细描述 SideX TypeScript 语言服务各功能的实现方案，基于 `extensions-rust/typescript-language-extension/src/lib.rs` 的现有架构。

## 1. 架构概述

### 1.1 系统架构

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│   Monaco Editor │────▶│ WASM 扩展 (Rust) │────▶│   tsserver      │
│   (Frontend)    │◀────│ (语言服务)       │◀────│   (TypeScript)  │
└─────────────────┘     └──────────────────┘     └─────────────────┘
```

### 1.2 核心组件

| 组件 | 职责 | 位置 |
|------|------|------|
| `TypeScriptLanguageExtension` | 语言服务入口 | `lib.rs:7` |
| `tsserver_request()` | 发送请求到 tsserver | `lib.rs:398-415` |
| 解析函数 | 解析 tsserver 响应 | `lib.rs:417-816` |

### 1.3 现有实现模式

所有已实现功能遵循统一模式：

```rust
fn provide_xxx(ctx: DocumentContext, pos: Position) -> ReturnType {
    // 1. 检查语言类型
    if !is_ts_js(&ctx.language_id) {
        return empty_result();
    }
    // 2. 调用 tsserver
    tsserver_request("command", &ctx, Some(pos), extra)
        // 3. 解析响应
        .and_then(|r| parse_ts_xxx(&r))
        // 4. 返回结果或默认值
        .unwrap_or_default()
}
```

## 2. tsserver 通信协议

### 2.1 请求格式

```rust
fn tsserver_request(
    command: &str,
    ctx: &DocumentContext,
    pos: Option<Position>,
    extra: Option<&str>,
) -> Option<String> {
    let file = ctx.uri.strip_prefix("file://").unwrap_or(&ctx.uri);
    let pos_str = pos
        .map(|p| format!(r#","line":{},"offset":{}"#, p.line + 1, p.character + 1))
        .unwrap_or_default();
    let extra_str = extra.unwrap_or("");
    let payload = format!(
        r#"{{"command":"{command}","arguments":{{"file":"{}"{pos_str}{extra_str}}}}}"#,
        file.replace('"', "\\\"")
    );
    host::execute_command("__sidex.tsserver", &payload).ok()
}
```

### 2.2 常用 tsserver 命令

| 命令 | 用途 | 参数 |
|------|------|------|
| `definition` | 跳转定义 | file, line, offset |
| `typeDefinition` | 跳转类型定义 | file, line, offset |
| `implementation` | 跳转实现 | file, line, offset |
| `declaration` | 跳转声明 | file, line, offset |
| `references` | 查找引用 | file, line, offset |
| `format` | 格式化文档 | file, line, offset, endLine, endOffset |
| `getCodeFixes` | 代码修复 | file, startLine, startOffset, endLine, endOffset |
| `navtree` | 文档符号 | file |
| `navto` | 工作区符号 | file(?), search value |
| `getFoldingRanges` | 代码折叠 | file |
| `semanticDiagnosticsSync` | 语义诊断 | file |
| `documentHighlights` | 文档高亮 | file, line, offset |

### 2.3 响应解析模式

现有解析函数使用字符串提取模式，以 `parse_ts_locations` 为例：

```rust
fn parse_ts_locations(json: &str) -> Option<Vec<Location>> {
    let mut locs = Vec::new();
    let mut search = json;
    // 1. 查找 "file": 字段
    while let Some(file_pos) = search.find("\"file\":") {
        let after = &search[file_pos + 7..];
        // 2. 提取文件路径
        let file = extract_string_value(after)?;
        // 3. 提取行号和列号
        let start_line = extract_field_from_str(&search[file_pos..], "line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        // 4. 构建 Location
        locs.push(Location { ... });
        search = &search[file_pos + 7..];
    }
    if locs.is_empty() { None } else { Some(locs) }
}
```

## 3. P0 功能实现规格

### 3.1 Type Definition（类型定义跳转）

**功能描述**: 跳转到类型（interface、type、class）的定义位置

**实现方案**:

```rust
// 文件: extensions-rust/typescript-language-extension/src/lib.rs
// 位置: 第 199-201 行

fn provide_type_definition(ctx: DocumentContext, pos: Position) -> Vec<Location> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    tsserver_request("typeDefinition", &ctx, Some(pos), None)
        .and_then(|r| parse_ts_locations(&r))
        .unwrap_or_default()
}
```

**tsserver 请求**:

```json
{
  "command": "typeDefinition",
  "arguments": {
    "file": "/path/to/file.ts",
    "line": 10,
    "offset": 15
  }
}
```

**tsserver 响应**:

```json
{
  "body": [
    {
      "file": "/path/to/types.ts",
      "start": { "line": 5, "offset": 1 },
      "end": { "line": 5, "offset": 20 }
    }
  ],
  "success": true
}
```

**验收标准**:

- 对 `let x: MyType` 中的 `MyType` 按 Ctrl+点击可跳转到类型定义
- 支持 TypeScript、JavaScript、TSX、JSX
- 支持泛型类型参数跳转

### 3.2 Formatting（代码格式化）

**功能描述**: 格式化整个文档或选区

**实现方案**:

```rust
// 位置: 第 217 行

fn provide_formatting(
    ctx: DocumentContext,
    tab_size: u32,
    insert_spaces: bool,
) -> Vec<TextEdit> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    // 注意: tsserver format 命令的参数格式需要实际验证
    // 以下是推测的格式，可能需要调整
    let extra = format!(
        r#","options":{{"tabSize":{},"insertSpaces":{}}}"#,
        tab_size,
        if insert_spaces { "true" } else { "false" }
    );
    tsserver_request("format", &ctx, None, Some(&extra))
        .and_then(|r| parse_ts_formatting(&r))
        .unwrap_or_default()
}
```

**tsserver 请求（格式待验证）**:

```json
{
  "command": "format",
  "arguments": {
    "file": "/path/to/file.ts",
    "line": 1,
    "offset": 1,
    "endLine": 100,
    "endOffset": 1,
    "options": {
      "tabSize": 2,
      "insertSpaces": true
    }
  }
}
```

**tsserver 响应**:

```json
{
  "body": [
    {
      "start": { "line": 1, "offset": 1 },
      "end": { "line": 1, "offset": 1 },
      "newText": "  "
    }
  ],
  "success": true
}
```

**解析函数**:

```rust
fn parse_ts_formatting(json: &str) -> Option<Vec<TextEdit>> {
    let mut edits = Vec::new();
    let mut search = json;
    while let Some(start_pos) = search.find("\"start\":") {
        let start_line = extract_field_from_str(&search[start_pos..], "line")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let start_col = extract_field_from_str(&search[start_pos..], "offset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let end_line = extract_field_from_str(&search[start_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let end_col = extract_field_from_str(&search[start_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let new_text = extract_field_from_str(&search[start_pos..], "newText")
            .unwrap_or_default();
        
        edits.push(TextEdit {
            range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: end_line, character: end_col },
            },
            new_text,
        });
        search = &search[start_pos + 7..];
    }
    if edits.is_empty() { None } else { Some(edits) }
}
```

**验收标准**:

- Shift+Alt+F 触发文档格式化
- 选中文本后触发选区格式化
- 支持配置 tabSize 和 insertSpaces
- 保持光标位置相对不变

### 3.3 Implementation（实现跳转）

**功能描述**: 跳转到接口或抽象方法的具体实现

**实现方案**:

```rust
// 位置: 第 202-204 行

fn provide_implementation(ctx: DocumentContext, pos: Position) -> Vec<Location> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    tsserver_request("implementation", &ctx, Some(pos), None)
        .and_then(|r| parse_ts_locations(&r))
        .unwrap_or_default()
}
```

**tsserver 请求**:

```json
{
  "command": "implementation",
  "arguments": {
    "file": "/path/to/file.ts",
    "line": 10,
    "offset": 15
  }
}
```

**验收标准**:

- 对接口方法 Ctrl+点击可跳转到实现
- 支持一个接口多个实现的情况
- 返回所有实现的 Location 列表

## 4. P1 功能实现规格

### 4.1 Folding Ranges（代码折叠）

**功能描述**: 支持代码区域折叠（函数、类、条件块等）

**实现方案**:

```rust
// 位置: 第 223-225 行

fn provide_folding_ranges(ctx: DocumentContext) -> Vec<FoldingRange> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    tsserver_request("getFoldingRanges", &ctx, None, None)
        .and_then(|r| parse_ts_folding_ranges(&r))
        .unwrap_or_default()
}
```

**tsserver 请求**:

```json
{
  "command": "getFoldingRanges",
  "arguments": {
    "file": "/path/to/file.ts"
  }
}
```

**tsserver 响应**:

```json
{
  "body": [
    {
      "startLine": 5,
      "startOffset": 0,
      "endLine": 20,
      "endOffset": 1,
      "kind": "region"
    },
    {
      "startLine": 10,
      "startOffset": 4,
      "endLine": 15,
      "endOffset": 5,
      "kind": "function"
    }
  ],
  "success": true
}
```

**解析函数**:

```rust
fn parse_ts_folding_ranges(json: &str) -> Option<Vec<FoldingRange>> {
    let mut ranges = Vec::new();
    let mut search = json;
    while let Some(start_pos) = search.find("\"startLine\":") {
        let start_line = extract_field_from_str(&search[start_pos..], "startLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let end_line = extract_field_from_str(&search[start_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let kind_str = extract_field_from_str(&search[start_pos..], "kind")
            .unwrap_or_default();
        
        let kind = match kind_str.as_str() {
            "region" => 1,  // comment
            "function" => 2, // function
            "class" => 3,   // class
            "import" => 4,  // import
            "comment" => 5,
            _ => 0,         // not foldable
        };
        
        ranges.push(FoldingRange {
            start_line,
            end_line,
            kind: Some(kind),
            ..Default::default()
        });
        search = &search[start_pos + 11..];
    }
    if ranges.is_empty() { None } else { Some(ranges) }
}
```

### 4.2 Semantic Tokens（语义高亮）

**功能描述**: 变量、函数、类等语法元素的语义高亮

**实现方案**:

```rust
// 位置: 第 232 行

fn provide_semantic_tokens(ctx: DocumentContext) -> Option<SemanticTokens> {
    if !is_ts_js(&ctx.language_id) {
        return None;
    }
    // 注意: tsserver 不直接提供 semantic tokens
    // 使用 navtree 命令获取符号信息，然后转换为 semantic tokens
    tsserver_request("navtree", &ctx, None, None)
        .and_then(|r| parse_semantic_tokens_from_navtree(&r))
}
```

**重要说明**:
- tsserver 的 `semanticDiagnosticsSync` 命令返回的是诊断信息（错误、警告、提示），不是语义标记
- 语义标记需要区分变量、函数、类、接口等语法元素的类型
- 上述方案使用 `navtree` 命令获取符号树，然后转换为 LSP 的 Semantic Tokens 格式
- 这是一个简化实现，可能无法提供与 VS Code 完全一致的语义高亮效果
- 如需完整的语义标记支持，可能需要扩展 tsserver 协议或使用其他方法

**get_semantic_tokens_legend 配置**:

```rust
fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
    Some(SemanticTokensLegend {
        token_types: vec![
            "comment".to_string(),
            "keyword".to_string(),
            "string".to_string(),
            "number".to_string(),
            "regexp".to_string(),
            "operator".to_string(),
            "namespace".to_string(),
            "type".to_string(),
            "class".to_string(),
            "interface".to_string(),
            "enum".to_string(),
            "function".to_string(),
            "variable".to_string(),
            "parameter".to_string(),
            "property".to_string(),
            "label".to_string(),
        ],
        token_modifiers: vec![
            "declaration".to_string(),
            "definition".to_string(),
            "readonly".to_string(),
            "static".to_string(),
            "deprecated".to_string(),
            "abstract".to_string(),
            "async".to_string(),
            "modification".to_string(),
        ],
    })
}
```

```rust
fn get_semantic_tokens_legend() -> Option<SemanticTokensLegend> {
    Some(SemanticTokensLegend {
        token_types: vec![
            "comment".to_string(),
            "keyword".to_string(),
            "string".to_string(),
            "number".to_string(),
            "regexp".to_string(),
            "operator".to_string(),
            "namespace".to_string(),
            "type".to_string(),
            "class".to_string(),
            "interface".to_string(),
            "enum".to_string(),
            "function".to_string(),
            "variable".to_string(),
            "parameter".to_string(),
            "property".to_string(),
            "label".to_string(),
        ],
        token_modifiers: vec![
            "declaration".to_string(),
            "definition".to_string(),
            "readonly".to_string(),
            "static".to_string(),
            "deprecated".to_string(),
            "abstract".to_string(),
            "async".to_string(),
            "modification".to_string(),
        ],
    })
}
```

### 4.3 Workspace Symbols（工作区符号搜索）

**功能描述**: 通过 `#` 快速搜索工作区中的符号

**实现方案**:

```rust
// 位置: 第 238 行

fn provide_workspace_symbols(query: String) -> Vec<DocumentSymbol> {
    // 工作区符号搜索不需要 DocumentContext，直接构造 tsserver 请求
    let payload = format!(
        r#"{{"command":"navto","arguments":{{"searchValue":"{}"}}}}"#,
        query.replace('"', "\\\"")
    );
    host::execute_command("__sidex.tsserver", &payload)
        .and_then(|r| parse_ts_workspace_symbols(&r))
        .unwrap_or_default()
}
```

**注意**: 
- 由于 `tsserver_request` 函数强制要求 `DocumentContext` 参数来提取文件路径，而 Workspace Symbols 不需要限定在单个文件，因此直接调用 `host::execute_command` 构造请求
- `navto` 命令只接受 `searchValue` 参数，不需要 `file` 参数

**tsserver 请求**:

```json
{
  "command": "navto",
  "arguments": {
    "searchValue": "MyClass"
  }
}
```

**tsserver 响应**:

```json
{
  "body": [
    {
      "name": "MyClass",
      "kind": "class",
      "file": "/path/to/types.ts",
      "start": { "line": 10, "offset": 6 },
      "end": { "line": 10, "offset": 14 }
    }
  ],
  "success": true
}
```

## 5. P2 功能实现规格

### 5.1 Declaration（声明跳转）

**功能描述**: 跳转到变量的声明位置

**实现方案**:

```rust
// 位置: 第 205-207 行

fn provide_declaration(ctx: DocumentContext, pos: Position) -> Vec<Location> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    tsserver_request("declaration", &ctx, Some(pos), None)
        .and_then(|r| parse_ts_locations(&r))
        .unwrap_or_default()
}
```

### 5.2 Document Highlights（文档高亮）

**功能描述**: 高亮文档中所有引用同一变量的位置

**实现方案**:

```rust
// 位置: 第 208-210 行

fn provide_document_highlights(ctx: DocumentContext, pos: Position) -> Vec<DocumentHighlight> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    tsserver_request("documentHighlights", &ctx, Some(pos), None)
        .and_then(|r| parse_ts_document_highlights(&r))
        .unwrap_or_default()
}
```

**解析函数**:

```rust
fn parse_ts_document_highlights(json: &str) -> Option<Vec<DocumentHighlight>> {
    let mut highlights = Vec::new();
    let mut search = json;
    while let Some(file_pos) = search.find("\"file\":") {
        let file = extract_string_value(&search[file_pos + 7..])?;
        let start_line = extract_field_from_str(&search[file_pos..], "startLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let start_col = extract_field_from_str(&search[file_pos..], "startOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let end_line = extract_field_from_str(&search[file_pos..], "endLine")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        let end_col = extract_field_from_str(&search[file_pos..], "endOffset")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(1)
            .saturating_sub(1);
        
        highlights.push(DocumentHighlight {
            range: Range {
                start: Position { line: start_line, character: start_col },
                end: Position { line: end_line, character: end_col },
            },
            kind: Some(1), // Text
        });
        search = &search[file_pos + 7..];
    }
    if highlights.is_empty() { None } else { Some(highlights) }
}
```

### 5.3 Code Lenses（代码透镜）

**功能描述**: 显示引用计数、运行测试等代码信息

**实现方案**:

```rust
// 位置: 第 214-216 行

fn provide_code_lenses(ctx: DocumentContext) -> Vec<CodeLens> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    // 需要实现 references 提供引用计数
    tsserver_request("references", &ctx, None, None)
        .and_then(|r| parse_ts_code_lenses(&r))
        .unwrap_or_default()
}
```

### 5.4 Range Formatting（选区格式化）

**功能描述**: 格式化选中的代码区域

**实现方案**:

```rust
// 位置: 第 220-222 行

fn provide_range_formatting(
    ctx: DocumentContext,
    range: Range,
    tab_size: u32,
    insert_spaces: bool,
) -> Vec<TextEdit> {
    if !is_ts_js(&ctx.language_id) {
        return vec![];
    }
    let extra = format!(
        r#","startLine":{},"startOffset":{},"endLine":{},"endOffset":{},"options":{{"tabSize":{},"insertSpaces":{}}}"#,
        range.start.line + 1,
        range.start.character + 1,
        range.end.line + 1,
        range.end.character + 1,
        tab_size,
        if insert_spaces { "true" } else { "false" }
    );
    tsserver_request("format", &ctx, None, Some(&extra))
        .and_then(|r| parse_ts_formatting(&r))
        .unwrap_or_default()
}
```

## 6. 通用解析函数参考

### 6.1 辅助函数

现有代码中已定义的辅助函数：

```rust
// 从 JSON 字符串提取字段值
fn extract_field_from_str(json: &str, field: &str) -> Option<String>

// 提取字符串值（处理转义）
fn extract_string_value(s: &str) -> Option<String>

// 从字符数组提取字段
fn extract_field(chars: &[char], obj_json: &str, field: &str) -> Option<String>

// 提取 JSON 对象
fn extract_json_object(chars: &[char], start: usize) -> (String, usize)
```

### 6.2 类型映射

```rust
fn ts_kind_to_completion_kind(kind: &str) -> u32 {
    match kind {
        "function" | "local function" => 2,  // Function
        "method" => 1,                       // Method
        "constructor" => 3,                  // Constructor
        "field" | "property" => 9,           // Field
        "variable" | "local var" => 5,       // Variable
        "class" => 6,                        // Class
        "interface" => 7,                     // Interface
        "module" | "namespace" => 8,         // Module
        "keyword" => 13,                      // Keyword
        "type" | "alias" => 24,              // TypeParameter
        "enum" => 12,                        // Enum
        "enum member" => 19,                  // EnumMember
        "const" => 20,                       // Constant
        "parameter" => 5,                    // Variable
        _ => 0,                              // Text
    }
}
```

## 7. 测试验证策略

### 7.1 单元测试

每个解析函数需要对应的单元测试：

```rust
#[test]
fn test_parse_ts_type_definition() {
    let json = r#"{"body":[{"file":"/test.ts","start":{"line":5,"offset":1},"end":{"line":5,"offset":20}}],"success":true}"#;
    let result = parse_ts_type_definition(json);
    assert!(result.is_some());
    let locs = result.unwrap();
    assert_eq!(locs.len(), 1);
    assert_eq!(locs[0].uri, "file:///test.ts");
    assert_eq!(locs[0].range.start.line, 4);
}
```

### 7.2 集成测试

创建 TypeScript 测试项目验证功能：

```
tests/
├── ts-project/
│   ├── src/
│   │   ├── index.ts       # 主入口
│   │   ├── types.ts       # 类型定义
│   │   ├── classes.ts    # 类和接口
│   │   └── functions.ts   # 函数定义
│   ├── tsconfig.json
│   └── package.json
```

### 7.3 验证清单

| 功能 | 验证方法 | 预期结果 |
|------|----------|----------|
| Type Definition | Ctrl+点击类型引用 | 跳转到类型定义 |
| Formatting | Shift+Alt+F | 文档正确格式化 |
| Implementation | Ctrl+点击接口方法 | 跳转到实现 |
| Folding | 点击折叠标记 | 代码区域折叠 |
| Document Highlights | 选择变量 | 所有引用高亮 |
| Workspace Symbols | #搜索 | 返回匹配符号列表 |

## 8. 文件索引

| 文件路径 | 描述 |
|----------|------|
| `extensions-rust/typescript-language-extension/src/lib.rs` | 主扩展实现 |
| `extensions-rust/typescript-language-extension/sidex.toml` | 扩展配置 |
| `docs/ts/README.md` | 功能现状分析 |
| `docs/ts/DEVELOPMENT_PLAN.md` | 开发计划 |
| `docs/ts/implementation/TECHNICAL_SPEC.md` | 本技术规格文档 |

## 9. 依赖项

1. **tsserver**: 需要工作区安装 TypeScript
2. **sidex-extension-sdk**: 需要已构建
3. **WASM 构建环境**: Rust + wasm32-wasip2 target

## 10. 已知问题与验证需求

### 10.1 tsserver 命令参数格式验证

以下 tsserver 命令的参数格式需要实际验证：

| 命令 | 参数格式 | 验证状态 |
|------|----------|----------|
| `typeDefinition` | file, line, offset | ⚠️ 待验证 |
| `format` | file, options | ⚠️ 待验证 |
| `getFoldingRanges` | file | ⚠️ 待验证 |
| `navto` | searchValue | ⚠️ 待验证 |
| `documentHighlights` | file, line, offset | ⚠️ 待验证 |
| `declaration` | file, line, offset | ⚠️ 待验证 |

**验证方法**:
1. 创建测试工具直接调用 tsserver
2. 捕获实际响应格式
3. 更新文档中的格式说明

### 10.2 架构限制

**tsserver_request 函数限制**:
- 强制要求 `DocumentContext` 参数
- 对于不需要文件路径的命令（如 `navto`），需要直接调用 `host::execute_command`

**解决方案**:
- 对于 Workspace Symbols 等功能，使用 `host::execute_command` 直接构造请求
- 未来考虑重构 `tsserver_request` 支持可选的文件路径参数

### 10.3 功能实现复杂度

| 功能 | 实现复杂度 | 注意事项 |
|------|-----------|----------|
| Type Definition | 低 | 复用现有解析函数 |
| Formatting | 中 | 参数格式需验证 |
| Implementation | 低 | 复用现有解析函数 |
| Folding Ranges | 中 | 响应格式需验证 |
| Semantic Tokens | 高 | tsserver 不直接支持，需要转换 |
| Workspace Symbols | 中 | 需要绕过 tsserver_request |
| Document Highlights | 中 | 响应格式需验证 |
| Code Lenses | 高 | 需要组合多个命令 |

## 11. 风险与挑战

1. **tsserver 版本兼容性**: 不同版本返回格式可能有差异
2. **性能**: 某些请求（如 semantic tokens）可能较慢
3. **WASM 调试**: 跨平台调试 WASM 扩展较困难
4. **异步处理**: tsserver 响应可能需要异步处理
