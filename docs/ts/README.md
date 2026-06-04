# TypeScript 功能现状分析

> 本文档对比 SideX 与 void 源码的 TypeScript 语言服务实现差异
> void 源码: https://github.com/voideditor/void

## void 源码实现方式

void 使用 VS Code 原生的 TypeScript 扩展，位于 `extensions/typescript` 目录，通过以下方式提供语言服务：

- **tsserver 进程**: void 直接使用 VS Code 内置的 TypeScript 扩展，启动 tsserver 进程提供语言服务
- **Language Server Protocol**: 通过 LSP 与 Monaco Editor 通信
- **扩展机制**: 使用 VS Code 标准的扩展系统加载语言服务

## SideX 实现方式

SideX 使用自定义的 WASM 扩展实现：

- **WASM 扩展**: `typescript-language-extension.wasm`
- **sidex-extension-sdk**: 提供 Rust 到 WASM 的绑定
- **tsserver 客户端**: Rust 后端启动和管理 tsserver 进程

## 当前实现状态

### ✅ SideX 已实现的功能

| 功能 | SideX 状态 | void 状态 |
|------|-----------|----------|
| Completion | ✅ 已实现 | ✅ |
| Hover | ✅ 已实现 | ✅ |
| Definition | ✅ 已实现 | ✅ |
| References | ✅ 已实现 | ✅ |
| Document Symbols | ✅ 已实现 | ✅ |
| Signature Help | ✅ 已实现 | ✅ |
| Code Actions | ✅ 已实现 | ✅ |
| Rename | ✅ 已实现 | ✅ |
| Inlay Hints | ✅ 已实现 | ✅ |
| Diagnostics | ✅ 已实现 | ✅ |

### ❌ SideX 未实现的功能

| 功能 | SideX 状态 | void 状态 | 优先级 |
|------|-----------|----------|--------|
| Type Definition | ❌ 返回空 | ✅ | 高 |
| Implementation | ❌ 返回空 | ✅ | 高 |
| Declaration | ❌ 返回空 | ✅ | 中 |
| Formatting | ❌ 返回空 | ✅ | 高 |
| Range Formatting | ❌ 返回空 | ✅ | 中 |
| Folding Ranges | ❌ 返回空 | ✅ | 中 |
| Document Highlights | ❌ 返回空 | ✅ | 低 |
| Code Lenses | ❌ 返回空 | ✅ | 低 |
| Semantic Tokens | ❌ 返回空 | ✅ | 中 |
| Workspace Symbols | ❌ 返回空 | ✅ | 中 |

## 架构差异

| 方面 | void | SideX |
|------|------|-------|
| 语言服务 | 内置 TypeScript 扩展 | WASM 扩展 |
| tsserver | 由扩展管理 | Rust 后端启动 |
| 进程模型 | 扩展宿主进程 | Sidecar 进程 |
| 扩展系统 | VS Code 扩展 API | sidex-extension-sdk |

## 实现优先级

### 高优先级
1. **Type Definition** - 跳转到类型定义
2. **Formatting** - 代码格式化
3. **Implementation** - 跳转到实现

### 中优先级
4. **Folding Ranges** - 代码折叠
5. **Semantic Tokens** - 语义高亮
6. **Workspace Symbols** - 工作区符号搜索

### 低优先级
7. Declaration
8. Document Highlights
9. Code Lenses
10. Range Formatting

## 相关文件

- SideX 扩展实现: `extensions-rust/typescript-language-extension/src/lib.rs`
- SideX 扩展配置: `extensions-rust/typescript-language-extension/sidex.toml`
- SideX CI 构建: `.github/workflows/release.yml`
- void 源码: https://github.com/voideditor/void

## 构建说明

WASM 扩展构建需要:
1. Rust 工具链 + wasm32-wasip2 target
2. sidex-extension-sdk (需先构建)
3. 执行: `cargo build --release --target wasm32-wasip2 -p typescript-language-extension`

## 已知问题

1. **CORS 问题**: marketplace.siden.ai 语法文件加载时遇到 CORS 错误（仅影响语法高亮）
2. **tsserver 依赖**: 需要在工作区安装 TypeScript 包
3. **WASM 扩展**: 需要在 CI 中构建
