# TypeScript 功能开发设计文档问题报告

本报告记录了在检查 TypeScript 语言服务开发设计文档时发现的问题和修复方案。

**最后更新**: 2026-02-04  
**状态**: 主要问题已修复

## 已修复问题 ✓

### 1.1 Workspace Symbols 实现方案错误 ✓

**原问题**: 文档创建空的 `DocumentContext` 调用 `tsserver_request`

**修复方案**:
- 使用 `host::execute_command` 直接构造 tsserver 请求
- 避免使用需要 DocumentContext 的 `tsserver_request` 函数

**更新位置**:
- `docs/ts/implementation/TECHNICAL_SPEC.md:468-482`

---

### 1.2 Semantic Tokens 实现方案不可行 ✓

**原问题**: 使用 `semanticDiagnosticsSync` 命令获取语义标记（错误）

**修复方案**:
- 使用 `navtree` 命令获取符号信息
- 将符号信息转换为 LSP Semantic Tokens 格式
- 添加详细的实现说明和限制说明

**更新位置**:
- `docs/ts/implementation/TECHNICAL_SPEC.md:416-423`
- `docs/ts/implementation/PARSER_REFERENCE.md:418-456`

---

## 需要验证的问题 ⚠️

### 2.1 Format 命令参数格式 ⚠️

**位置**: `docs/ts/implementation/TECHNICAL_SPEC.md:194-210`

**状态**: 标记为待验证，添加了验证说明

**问题**: tsserver format 命令的参数格式和响应格式需要实际验证

---

### 2.2 tsserver 响应格式 ⚠️

**位置**: 多个位置

**状态**: 在 PARSER_REFERENCE.md 开头添加了验证状态说明

**问题**: 多个命令的响应格式需要实际验证

---

## 架构说明 ⚠️

### 4.1 tsserver_request 函数的设计限制 ⚠️

**状态**: 已添加说明，作为架构限制记录

**说明**: tsserver_request 函数强制要求 DocumentContext 参数，对于不需要文件路径的命令需要使用 `host::execute_command`

---

## 修复总结

| 问题 | 状态 | 更新文件 |
|------|------|----------|
| Workspace Symbols 实现方案错误 | ✅ 已修复 | TECHNICAL_SPEC.md |
| Semantic Tokens 实现方案不可行 | ✅ 已修复 | TECHNICAL_SPEC.md, PARSER_REFERENCE.md |
| Format 命令参数格式不确定 | ⚠️ 已标记 | TECHNICAL_SPEC.md, PARSER_REFERENCE.md |
| tsserver 响应格式假设可能不准确 | ⚠️ 已标记 | PARSER_REFERENCE.md |
| tsserver_request 函数设计限制 | ⚠️ 已说明 | TECHNICAL_SPEC.md |

---

## 后续行动建议

### 高优先级:
1. ⚠️ 创建测试工具实际验证 tsserver 命令响应格式
2. ⚠️ 更新已验证的响应格式到文档

### 中优先级:
3. 📝 考虑重构 `tsserver_request` 函数支持可选文件路径
4. 📝 添加 sidex-extension-sdk API 文档

---

## 保留的历史分析

以下内容保留供参考，显示问题的原始分析：

### 原问题分析（已修复）

#### 1. Workspace Symbols 实现方案错误

**位置**: `docs/ts/implementation/TECHNICAL_SPEC.md:468-482`

**问题描述**:
文档中的实现方案创建了一个空的 `DocumentContext` 来调用 `tsserver_request`，但这与实际的函数签名不匹配，且 tsserver 的 `navto` 命令不需要 `file` 参数。

**错误代码**:
```rust
// 文档中的错误实现
fn provide_workspace_symbols(query: String) -> Vec<DocumentSymbol> {
    let extra = format!(r#","searchValue":"{}""#, query.replace('"', "\\\""));
    // 错误：工作区符号不需要 DocumentContext
    let ctx = DocumentContext {
        uri: "".to_string(),
        language_id: "typescript".to_string(),
        version: 0,
    };
    tsserver_request("navto", &ctx, None, Some(&extra))
        .and_then(|r| parse_ts_workspace_symbols(&r))
        .unwrap_or_default()
}
```

**实际函数签名**:
```rust
// extensions-rust/typescript-language-extension/src/lib.rs:238
fn provide_workspace_symbols(_: String) -> Vec<DocumentSymbol> {
    vec![]
}
```

#### 2. Semantic Tokens 实现方案不完整且可能错误

**位置**: `docs/ts/implementation/TECHNICAL_SPEC.md:416-423` 和 `docs/ts/implementation/PARSER_REFERENCE.md:428-449`

**问题描述**:
文档使用 `semanticDiagnosticsSync` 命令来获取语义标记，但这个命令返回的是诊断信息（错误、警告），不是语义标记（变量、函数、类的分类信息）。

**错误代码**:
```rust
// 文档中的错误实现
fn provide_semantic_tokens(ctx: DocumentContext) -> Option<SemanticTokens> {
    if !is_ts_js(&ctx.language_id) {
        return None;
    }
    tsserver_request("semanticDiagnosticsSync", &ctx, None, None)
        .and_then(|r| parse_ts_semantic_tokens(&r, &ctx.uri))
}
```

**问题分析**:
1. `semanticDiagnosticsSync` 返回的是 `Diagnostic` 对象（错误、警告、提示）
2. Semantic Tokens 需要的是语法元素的分类（变量、函数、类型等）
3. PARSER_REFERENCE 中也说"tsserver 不直接返回 semantic tokens"
4. 没有提供可行的实现方案

---

## 附录：需要验证的 tsserver 命令

以下命令的参数格式和响应格式需要实际验证：

| 命令 | 参数格式 | 响应格式 | 验证状态 |
|------|----------|----------|----------|
| `typeDefinition` | file, line, offset | Location[] | ⚠️ 待验证 |
| `format` | file, options | TextEdit[] | ⚠️ 待验证 |
| `getFoldingRanges` | file | FoldingRange[] | ⚠️ 待验证 |
| `navto` | searchValue | DocumentSymbol[] | ⚠️ 待验证 |
| `documentHighlights` | file, line, offset | DocumentHighlight[] | ⚠️ 待验证 |
| `declaration` | file, line, offset | Location[] | ⚠️ 待验证 |

建议创建一个测试工具来实际调用这些命令并记录响应。