# SideX 完整任务清单

> 基于代码库探索结果和 void 对齐计划生成
> 更新时间：2026-05-07

## 任务状态图例
- ✅ 已完成
- 🟡 部分完成（主路径可用，存在缺口）
- 🔴 未开始
- ⚪ 不计划

---

## P0：扩展与调试（生态核心）

### B1. Node 扩展宿主生命周期
**状态**: 🟡 部分完成（API 覆盖审计完成，剩余缺口已记录）
**代码落点**: `src-tauri/src/commands/ext_host.rs`, `src-tauri/extension-host/host.cjs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 端口握手协议 | ✅ | stdout 首行 JSON 协议已实现 |
| 扩展发现与扫描 | ✅ | 多路径扫描，Node + WASM 双支持 |
| 扩展激活/停用 | ✅ | 生命周期事件已记录 |
| 崩溃自动重启 | 🟡 | 崩溃计数存在，限频策略需完善 |
| 环境变量注入 | ✅ | SIDEX_EXTENSIONS_DIR 等已注入 |
| API 覆盖度验证 | ✅ | 已完成完整审计，报告见 `EXTENSION_API_COVERAGE.md` |

### B1 后续改进项
| 改进项 | 优先级 | 说明 |
|--------|--------|------|
| 文件事件桥接 | P1 | `onDidCreateFiles`/`onDidDeleteFiles`/`onDidRenameFiles` 当前为 noop |
| 终端事件桥接 | P1 | `onDidOpenTerminal`/`onDidCloseTerminal` 当前为 noop |
| 工作区文件夹事件 | P2 | `onDidChangeWorkspaceFolders` 当前为 noop |
| Notebook 事件 | P2 | 所有 notebook 相关事件为 noop |

### B2. 扩展诊断与可观测性
**状态**: 🟡 部分完成
**代码落点**: `src-tauri/src/commands/extension_diagnostics.rs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 扩展状态记录 | ✅ | Discovered/Loading/Activated/Failed 等 |
| 激活耗时统计 | ✅ | 已记录 |
| 慢扩展检测 | ✅ | >2s 激活检测 |
| Extension Bisect | ✅ | 二分查找问题扩展 |
| UI 联动展示 | 🔴 | 诊断面板需与前端集成 |
| 错误堆栈可视化 | 🔴 | 需在前端展示根因 |

### B3. Webview 扩展能力
**状态**: 🟡 部分完成（CSP 安全策略已对齐 void 默认策略）
**代码落点**: `src-tauri/extension-host/host.cjs`, `crates/sidex-extensions/src/webview_host.rs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 资源 URL 映射 | 🟡 | host.cjs 有处理逻辑 |
| CSP 安全策略 | ✅ | 已对齐 void 默认策略，保留扩展 CSP，注入默认 CSP |
| 消息通道 | 🟡 | 基础通信可用 |
| 性能优化（大消息） | 🔴 | 二进制资源传输待优化 |
| Webview 面板管理 | ✅ | sidex-extensions 已实现 |

### B4. 调试器 DAP 全链路
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-dap/`, `src-tauri/src/commands/debug.rs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| DAP 协议类型 | ✅ | protocol.rs 覆盖全部标准类型 |
| DebugClient | ✅ | initialize/launch/attach/步进等 |
| 断点管理 | ✅ | setBreakpoints + 持久化 |
| 变量/调用栈 | ✅ | scopes/variables/threads |
| 调试控制台 | ✅ | evaluate + REPL |
| 启动配置解析 | ✅ | launch.json JSONC 解析 |
| 内存调试 | 🔴 | debugMemory.ts 占位 |
| 数据断点 | 🔴 | 前端支持有限 |
| 反汇编视图 | 🟡 | disassemblyView.ts 存在 |
| 多会话管理 | ✅ | session.rs 状态机 |

### B5. 调试适配器管理 🔴 **新增**
**状态**: ✅ 基本完成（自动发现 + 市场下载 + UI 集成）
**代码落点**: `crates/sidex-dap/src/adapter.rs`, `src-tauri/src/commands/debug.rs`, `crates/sidex-extensions/src/contribution_handler.rs`, `src-tauri/src/commands/extensions.rs`, `src/vs/workbench/contrib/debug/tauri/`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| Adapter 自动发现 | ✅ | 扫描扩展 debugger 贡献，动态注册/注销 |
| Adapter 下载 | ✅ | 从市场搜索/下载安装 debug adapter 扩展 |
| Adapter 配置 UI | ✅ | 市场搜索 + Quick Pick 安装界面 + launch.json IntelliSense |
| 沙箱运行策略 | 🔴 | 隔离调试适配器进程 |

---

## P0：核心平台能力

### C1-C5. 平台能力缺口
**状态**: ✅ 全部完成（见前文实现摘要）

---

## P1：Workbench 功能对齐

### D1. Settings Sync
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-settings/src/sync.rs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 6 种同步资源 | ✅ | Settings/Keybindings/Extensions/Snippets/GlobalState/Profiles |
| 三向 merge 冲突解决 | ✅ | AcceptLocal/AcceptRemote/Merge |
| 账户管理 | 🔴 | GitHub/Microsoft 认证未集成 |
| 云存储后端 | 🔴 | SyncDataProvider 接口存在，无实现 |
| 自动同步开关 | ✅ | Rust 层已实现 |
| 导出/导入 | ✅ | JSON 格式 |
| 前端 Sync UI | 🟡 | 需更多集成 |

### D2. Profiles
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-profiles/src/lib.rs`, `crates/sidex-settings/src/profiles.rs`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| profiles.json 持久化 | ✅ | 与 VS Code 格式对齐 |
| workspace 映射 | ✅ | profile-associations.json |
| Profile 创建/切换 | 🔴 | 完整生命周期未实现 |
| Profile 删除 | 🔴 | 未实现 |
| 扩展隔离联动 | 🔴 | 未实现 |
| 导出/导入 | 🔴 | 未实现 |
| 前端 Profile UI | 🔴 | 切换 UI 未集成 |

### D3. Authentication
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-auth/`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| SecretStorage | ✅ | OS keyring + SQLite fallback |
| get/set/delete/keys | ✅ | 基础操作 |
| OAuth 流程 | 🔴 | 浏览器回调处理未实现 |
| AuthenticationProviderRegistry | 🔴 | 定义存在，未完整实现 |
| GitHub 登录 | 🔴 | 未实现 |
| Microsoft 登录 | 🔴 | 未实现 |
| Token 生命周期管理 | 🔴 | 刷新/过期处理未实现 |

### D4. 更新机制对齐
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-update/`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 状态机 | ✅ | Idle->Checking->Available->Downloading->Ready->Updating |
| Manifest 获取 | ✅ | 版本比较 |
| SHA-256 校验 | ✅ | 完整性校验 |
| Ed25519 签名验证 | ✅ | Minisign 支持 |
| 下载进度 | ✅ | 回调支持 |
| 平台适配 | ✅ | Windows/macOS/Linux |
| 与 void 行为对照 | 🔴 | 需核对更新频率/提示 UI |
| 回滚策略 | 🔴 | 失败回滚未实现 |

### D5. 多窗口/搜索回归
**状态**: 🟡 部分完成

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 多窗口 API | ✅ | create_window/close_window/set_window_title |
| 窗口状态持久化 | ✅ | sidex-db::window_state |
| 多根工作区 | ✅ | .code-workspace 解析 |
| 全文搜索 | ✅ | regex/大小写/全词匹配 |
| 模糊文件查找 | ✅ | FuzzyFileMatch |
| 文件监视器 | ✅ | notify crate |
| 索引器 | ✅ | InvertedIndex |
| 大仓库性能回归 | 🔴 | 需建立基准测试 |
| ignore 规则对齐 | 🔴 | 需与 void 对照 |
| 符号搜索 | 🔴 | 未实现 |

---

## P1：远程与容器

### E1. Remote-SSH
**状态**: 🟡 部分完成
**代码落点**: `crates/sidex-remote/`

| 子任务 | 状态 | 说明 |
|--------|------|------|
| SSH 连接 | ✅ | 密码/密钥/agent 认证 |
| ProxyJump | ✅ | 多跳连接 |
| 端口转发 | ✅ | 双向转发 |
| 文件读写 | ✅ | 远程文件系统 |
| PTY | ✅ | 远程终端 |
| SSH config 解析 | ✅ | known-hosts 检查 |
| WSL 支持 | ✅ | 发行版列表与连接 |
| Container 支持 | ✅ | devcontainer.json 解析 |
| Codespaces 支持 | ✅ | 列表与连接 |
| Tunnel 传输 | 🔴 | 未完全接入 RemoteManager |
| 远程文件 provider | 🔴 | 前端集成待完善 |
| 远端扩展策略 | 🔴 | 本地 vs 远端运行策略 |
| SideX Server 部署 | 🔴 | 自动安装/启动流程未完成 |

### E2. Dev Containers / WSL / Codespaces
**状态**: 🔴 部分未开始

| 子任务 | 状态 | 说明 |
|--------|------|------|
| Dev Container 启动 | 🟡 | container.rs 存在，端到端待验证 |
| 环境探测 | 🔴 | 自动探测开发环境 |
| 端口映射 | ✅ | port_forwarding.rs |
| 扩展运行策略 | 🔴 | 未实现 |
| WSL 最小闭环 | 🟡 | 基础可用 |
| Codespaces 最小闭环 | 🟡 | 基础可用 |

---

## P2：体验完善与工程化

### F1. 性能与资源基准 🔴 **需新建**
**状态**: 🔴 未开始

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 启动耗时测量 | 🔴 | 建立基线 |
| 空闲内存测量 | 🔴 | 建立基线 |
| 搜索耗时测量 | 🔴 | 大仓库基准 |
| 扩展激活耗时 | 🔴 | 建立基线 |
| 持续回归脚本 | 🔴 | 自动化测量 |
| perf/benchmarks.md | 🔴 | 文档新建 |

### F2. 可观测性与诊断面板
**状态**: 🟡 部分完成

| 子任务 | 状态 | 说明 |
|--------|------|------|
| 日志聚合 | ✅ | logging 已桥接 |
| 关键链路 metrics | 🔴 | 启动/扩展激活/搜索/git/终端 |
| 诊断面板 UI | 🔴 | 需新建 |
| 性能 trace 集成 | 🟡 | contentTracing 已实现 |

### F3. 回归测试 🔴 **需新建**
**状态**: 🔴 未开始

| 子任务 | 状态 | 说明 |
|--------|------|------|
| TS 单元测试框架 | 🔴 | vitest/jest 配置 |
| 集成测试 | 🔴 | 关键链路 E2E |
| void 兼容性冒烟 | 🔴 | 最小回归套件 |
| CI 集成 | 🟡 | test.yml 存在，需扩展 |

### F4. 打包资源校验 🔴 **需新建**
**状态**: 🔴 未开始

| 子任务 | 状态 | 说明 |
|--------|------|------|
| dist/extensions 检查 | 🟡 | verify-build-resources.js 存在 |
| extension-host 检查 | 🟡 | postbuild.js 部分覆盖 |
| NLS 资源检查 | 🔴 | 未覆盖 |
| shell-integration 检查 | 🔴 | 未覆盖 |
| 自检脚本固化 | 🔴 | npm run verify:resources |

---

## 待补充（需要 void 对照确认）

### G1. void 特有功能对齐 🔴
**状态**: 🔴 未开始

| 子任务 | 状态 | 说明 |
|--------|------|------|
| void 命令清单梳理 | 🔴 | 从 void 源码提取 |
| void 设置项对照 | 🔴 | 默认行为差异 |
| void 内置扩展清单 | 🔴 | 差异分析 |
| void 特有 UI 行为 | 🔴 | 交互差异记录 |

---

## 任务优先级建议

### 第一阶段（P0 剩余）
1. B5. 调试适配器管理（🔴）
2. B1. 扩展宿主 API 覆盖度验证（🔴）
3. B3. Webview CSP 安全策略（🔴）

### 第二阶段（P1 Workbench）
4. D3. Authentication OAuth 流程（🔴）
5. D1. Settings Sync 云存储后端（🔴）
6. D2. Profiles 完整生命周期（🔴）

### 第三阶段（P1 远程）
7. E1. Remote-SSH 端到端集成（🔴）
8. E2. Dev Containers 最小闭环（🔴）

### 第四阶段（P2 工程化）
9. F3. 回归测试框架（🔴）
10. F1. 性能基准（🔴）
11. F4. 打包资源校验（🔴）

### 持续贯穿
12. G1. void 特有功能对齐（🔴）

---

## 任务统计

| 状态 | 数量 | 占比 |
|------|------|------|
| ✅ 已完成 | 6 | 16% |
| 🟡 部分完成 | 9 | 24% |
| 🔴 未开始 | 23 | 61% |
| **总计** | **38** | **100%** |
