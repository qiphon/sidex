# TypeScript 语言服务开发计划

基于 [docs/ts/README.md](./ts/README.md) 的功能分析，本文档制定 SideX TypeScript 语言服务的开发计划。

## 现状概述

| 类别 | 数量 |
|------|------|
| 已实现功能 | 10 |
| 未实现功能 | 10 |

## 开发目标

在 SideX 中实现与 void 等价的 TypeScript 语言服务功能。

## 实施步骤

### 阶段一：核心功能补齐（P0）

目标是实现用户最常用的功能，确保日常开发体验。

#### 1.1 Type Definition（类型定义跳转）

**功能描述**: 跳转到类型的定义位置（如 `interface`、`type`、`class` 的定义）

**实现方案**:
1. 在 `typescript-language-extension/src/lib.rs` 中实现 `provide_type_definition` 函数
2. 调用 tsserver 的 `typeDefinition` 请求
3. 解析返回的 Location 数组

**参考 void**: void 使用内置 TypeScript 扩展，通过 LSP 提供此功能

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:199-201`

**验收标准**:
- 对 `let x: MyType` 中的 `MyType` 按 Ctrl+点击可跳转到类型定义
- 支持 TypeScript、JavaScript、TSX、JSX

#### 1.2 Formatting（代码格式化）

**功能描述**: 格式化整个文档或选区

**实现方案**:
1. 实现 `provide_formatting` 函数
2. 调用 tsserver 的 `format` 请求
3. 返回 TextEdit 数组

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:217-219`

**验收标准**:
- Shift+Alt+F 触发文档格式化
- 选中文本后触发选区格式化

#### 1.3 Implementation（实现跳转）

**功能描述**: 跳转到接口或抽象方法的具体实现

**实现方案**:
1. 实现 `provide_implementation` 函数
2. 调用 tsserver 的 `implementation` 请求
3. 解析返回的 Location 数组

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:202-204`

**验收标准**:
- 对接口方法 Ctrl+点击可跳转到实现

### 阶段二：增强功能（P1）

提升开发效率的辅助功能。

#### 2.1 Folding Ranges（代码折叠）

**功能描述**: 支持代码区域折叠

**实现方案**:
1. 实现 `provide_folding_ranges` 函数
2. 调用 tsserver 的 `getFoldingRanges` 请求
3. 返回 FoldingRange 数组

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:223-225`

#### 2.2 Semantic Tokens（语义高亮）

**功能描述**: 变量、函数、类等语法元素的语义高亮

**实现方案**:
1. 实现 `provide_semantic_tokens` 函数
2. 调用 tsserver 的 `semanticDiagnosticsSync` 获取语义信息
3. 返回 tokens 数组

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:232-234`

#### 2.3 Workspace Symbols（工作区符号搜索）

**功能描述**: 通过 `#` 快速搜索工作区中的符号

**实现方案**:
1. 实现 `provide_workspace_symbols` 函数
2. 调用 tsserver 的 `navto` 请求
3. 返回 DocumentSymbol 数组

**代码落点**: `extensions-rust/typescript-language-extension/src/lib.rs:238-240`

### 阶段三：辅助功能（P2）

#### 3.1 Declaration（声明跳转）

**功能描述**: 跳转到变量的声明位置

#### 3.2 Document Highlights（文档高亮）

**功能描述**: 高亮文档中所有引用同一变量的位置

#### 3.3 Code Lenses（代码透镜）

**功能描述**: 显示引用计数、运行测试等代码信息

#### 3.4 Range Formatting（选区格式化）

**功能描述**: 格式化选中的代码区域

## 技术实现要点

### tsserver 请求模式

参考现有实现模式（如 `provide_definition`）:

```rust
fn provide_definition(ctx: DocumentContext, pos: Position) -> Vec<Location> {
    tsserver_request("definition", &ctx, Some(pos), None)
        .and_then(|r| parse_ts_locations(&r))
        .unwrap_or_default()
}
```

需要添加的 tsserver 命令:
- `typeDefinition` - 类型定义
- `format` - 格式化
- `implementation` - 实现
- `getFoldingRanges` - 折叠范围
- `semanticDiagnosticsSync` - 语义诊断
- `navto` - 工作区搜索
- `declaration` - 声明

### 解析函数

需要在 `typescript-language-extension/src/lib.rs` 中添加对应的解析函数:

```rust
fn parse_ts_type_definition(response: &str) -> Vec<Location> { ... }
fn parse_ts_formatting(response: &str) -> Vec<TextEdit> { ... }
fn parse_ts_implementations(response: &str) -> Vec<Location> { ... }
// etc.
```

## 依赖项

1. **tsserver**: 需要工作区安装 TypeScript
2. **sidex-extension-sdk**: 需要已构建
3. **WASM 构建环境**: Rust + wasm32-wasip2 target

## 验证方法

每个功能实现后需验证:
1. 创建 TypeScript 测试项目
2. 触发对应功能（如 Ctrl+点击、快捷键等）
3. 确认行为与 void/VSCode 一致

## 风险与挑战

1. **tsserver 版本兼容性**: 不同版本返回格式可能有差异
2. **性能**: 某些请求（如 semantic tokens）可能较慢
3. **WASM 调试**: 跨平台调试 WASM 扩展较困难

## 里程碑

| 阶段 | 目标 | 预计时间 |
|------|------|----------|
| P0 | Type Definition + Formatting + Implementation | 2 周 |
| P1 | Folding + Semantic Tokens + Workspace Symbols | 2 周 |
| P2 | 剩余功能 | 1 周 |
