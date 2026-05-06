# SideX（基于 void 改造）项目概览

本仓库是一个基于 void 源码改造的 Rust 桌面编辑器项目：前端沿用 VSCode/void 的 TypeScript Workbench 与 Monaco，后端用 Tauri 2 + Rust 替代 Electron/Node 主进程与原生能力模块。项目目标是在保持 void 现有功能与行为兼容的前提下，持续演进 SideX 的性能、体积与平台集成能力。

## 一句话定位

VSCode/void Workbench（TS）+ Tauri（Rust）后端能力层 + 扩展体系（Node sidecar + WASM 扩展），构成一个不依赖 Electron 的桌面 IDE。

## 功能结构（从用户视角）

### 编辑器与工作台

- 编辑器内核：Monaco（语法高亮、基础智能提示、编辑器贡献项）
- IDE 工作台：Explorer / Search / SCM / Debug / Extensions / Output / Terminal 等视图与贡献项
- 主题与图标：VSCode 主题体系 + 内置主题资源
- 国际化：NLS loader + 构建期产物注入

### 终端

- 前端：xterm.js
- 后端：portable-pty，提供 spawn/write/resize/kill 等能力

### Git

- 后端：Rust git 能力（状态、diff、log、分支、push/pull/fetch、stash、reset 等）
- 前端：Workbench SCM 视图与命令集成

### 工作区与文件系统

- 文件树、文件操作、监听、搜索（文件名/全文）
- 存储：SQLite（用户数据、状态、历史等）

### 扩展体系

- Node 扩展：Tauri Rust 启动 sidecar（extension-host/server.cjs），前端通过 WebSocket/RPC 连接
- WASM 扩展：Rust 直接加载并提供能力，前端注册 providers
- 扩展来源：Open VSX 安装与管理

## 架构分层（代码视角）

### TypeScript 前端（VSCode/void workbench 移植）

- `src/main.ts`：前端启动入口（加载 NLS、初始化 Rust bridge services、创建 workbench）
- `src/vs/**`：VSCode/void 目录结构（base/platform/editor/workbench）
- `src/vs/platform/sidex/**`：SideX 的 Tauri/Rust bridge 与平台适配层

### Rust 后端（Tauri 2）

- `src-tauri/src/main.rs`：桌面端入口
- `src-tauri/src/lib.rs`：Tauri app 初始化与命令注册
- `src-tauri/src/commands/**`：后端能力模块（fs、terminal、git、search、settings、extensions、debug、tasks 等）

### Rust workspace crates（能力内聚）

- `crates/sidex-terminal`：终端/PTY
- `crates/sidex-git`：Git 能力
- `crates/sidex-workspace`：文件树/监听/搜索/工作区
- `crates/sidex-extensions`：扩展管理（安装、解析 manifest、注册表等）
- `crates/sidex-extension-api`：扩展 API（与前端/宿主的协议与消息处理）
- 其他：`sidex-db`、`sidex-settings`、`sidex-remote`、`sidex-dap`、`sidex-update` 等

## 构建与运行（关键约束）

- `npm run setup`：生成内置扩展元数据（依赖 `extensions/` 目录）
- `npm run setup:full`：填充内置扩展（拷贝/下载 VSCode 版本扩展）并生成元数据
- `npm run dev` / `npm run tauri dev`：开发模式
- `npm run build`：构建前端产物，并把内置扩展复制进 `dist/extensions`（供 Tauri Resource 使用）

## 修改代码时的“兼容优先”原则

- 把 void 源码视为功能与行为基准：改动必须不破坏 void 已实现能力与用户行为
- 遇到差异时优先补齐兼容层（bridge/adapter），而不是直接改动 workbench 上层逻辑
- 扩展/终端/Git/文件系统/菜单等属于核心链路，修改前需明确回归验证路径
