# void → SideX 功能对齐矩阵（Parity Matrix）

本文档用于把 void 作为功能与行为基准，将 SideX 的对齐状态可视化，并为每个条目提供“验证方法”和“代码落点”。该矩阵是开发与回归的核心依据：任何改动不得让已对齐项回退。

## 状态定义

- ✅ 已对齐：行为与 void 一致（或差异可忽略且有说明）
- 🟡 部分对齐：主路径可用，但存在缺口/降级/平台差异
- 🔴 未开始：缺失或仅有占位实现
- ⚪ 不计划：明确不做（需记录原因与替代方案）

## P0：扩展与调试

| 能力 | 状态 | SideX 现状/差异 | 代码落点（参考） | 验证方法 |
|---|---|---|---|---|
| Node 扩展宿主生命周期（发现/激活/停用/崩溃恢复） | 🟡 | 可启动 sidecar，但生命周期与恢复策略仍需补齐 | `src-tauri/src/commands/ext_host.rs`、`src-tauri/src/commands/extension_platform.rs`、`src/vs/workbench/contrib/extensions/browser/tauriExtensionHost.contribution.ts` | 安装/启用扩展并触发激活；模拟宿主崩溃后观察是否可恢复与提示 |
| 扩展诊断（错误可视化/慢扩展检测/日志聚合） | 🟡 | 有诊断状态结构，但需完善与 UI 联动 | `src-tauri/src/commands/extension_diagnostics.rs`、扩展 UI 相关 contrib | 制造一个失败扩展；在 UI/日志中能定位原因 |
| Webview 扩展能力（资源加载/CSP/消息通道） | 🟡 | host.cjs 有资源处理逻辑，需对齐 void 细节与安全策略 | `src-tauri/extension-host/host.cjs` | 运行 webview 扩展（如 Markdown/自定义 view）并验证资源/通信 |
| 扩展管理（市场/更新/依赖/兼容提示） | 🟡 | 支持 Open VSX 安装，但体验与策略待完善 | `src-tauri/src/commands/extensions.rs`、`crates/sidex-extensions/*` | 从 Open VSX 安装/卸载/更新扩展并验证 |
| 调试器（DAP 全链路） | 🟡 | 基础适配器/命令存在，但完整体验需补齐 | `crates/sidex-dap/*`、`src-tauri/src/commands/debug.rs` | Node/Python/Rust 任一项目完成断点→单步→查看变量→停止 |
| 调试适配器管理（发现/下载/配置/沙箱） | 🔴 | 需要对齐 void 的 adapter 管理逻辑 | debug 相关模块 | 通过 UI 配置并启动指定 adapter |

## P0：核心平台能力（ARCHITECTURE 标注 Partial/Not started）

| 能力 | 状态 | SideX 现状/差异 | 代码落点（参考） | 验证方法 |
|---|---|---|---|---|
| powerMonitor（睡眠/唤醒/电源事件） | ✅ | 已实现跨平台电源状态监听（Windows/macOS/Linux），支持 AC 状态变化事件 | `src-tauri/src/commands/power.rs`、`src/vs/platform/powerMonitor/common/powerMonitorService.ts` | 触发 sleep/resume 后终端/扩展状态不异常；插拔电源线时 UI 能收到事件 |
| contentTracing（性能追踪/导出） | ✅ | 已实现 Chrome Trace Event Format 导出，支持会话管理和事件记录 | `src-tauri/src/commands/content_tracing.rs`、`src/vs/platform/contentTracing/common/contentTracingService.ts` | 导出 trace 并可在 Chrome trace viewer 打开 |
| native-keymap（键盘布局/物理键位） | ✅ | 已实现跨平台键盘布局检测和物理键位映射 | `crates/sidex-keymap/src/layout.rs`、`src-tauri/src/commands/keymap.rs` | 不同布局下常用快捷键一致 |
| safeStorage/encryption（安全存储） | ✅ | 已实现 AES-GCM 加密层，密钥通过 OS keyring 保护 | `src/vs/workbench/services/encryption/tauri/encryptionService.ts`、`crates/sidex-auth/*` | 写入/读取 token，重启后仍可用 |
| logging（日志文件/级别/导出/崩溃聚合） | ✅ | 已桥接 TS 和 Rust 日志系统，支持多 logger 管理和文件轮转 | `src/vs/platform/log/common/tauriLogIpc.ts`、`src-tauri/src/commands/logging.rs` | 导出日志并可定位扩展宿主 session |

## P1：Workbench 功能对齐

| 能力 | 状态 | SideX 现状/差异 | 代码落点（参考） | 验证方法 |
|---|---|---|---|---|
| Settings Sync（设置同步） | 🔴 | 当前默认关闭，需要对齐 UI/流程/存储 | workbench settings sync 相关服务 | 开启同步并在新环境拉取成功 |
| Profiles（配置/扩展隔离） | 🟡 | 需要梳理现状并对齐 void 体验 | settings/profiles 相关模块 | 两个 profile 切换后扩展与设置隔离 |
| Authentication（GitHub 等） | 🟡 | 需要补齐 token 生命周期与扩展可用性 | authentication service + safeStorage | GitHub 登录后可用于 git/扩展请求 |
| 更新机制（策略/回滚/签名校验） | 🟡 | 有 sidex-update，需核对 void 行为一致性 | `crates/sidex-update/*` | 模拟更新元数据并验证流程 |
| 多窗口/多工作区（切换/恢复/状态） | 🟡 | 基础可用，需系统化回归 | window/workspace 相关服务 | 多窗口打开/切换/恢复无异常 |
| 搜索与索引（性能/ignore/排序） | 🟡 | 基础可用，需大仓库回归与对齐 | `crates/sidex-workspace/*`、search service | 大仓库搜索/替换/索引更新正确 |

## P1：远程与容器

| 能力 | 状态 | SideX 现状/差异 | 代码落点（参考） | 验证方法 |
|---|---|---|---|---|
| Remote-SSH（连接/转发/FS 映射/扩展策略） | 🟡 | 需要梳理 sidex-remote 的覆盖面 | `crates/sidex-remote/*` | 连接远端并打开文件夹，编辑与终端可用 |
| Dev Containers（启动/探测/端口/扩展） | 🔴 | 未开始 | 远程/容器相关模块 | 跑通官方示例容器工程 |
| WSL / Codespaces | 🔴 | 未开始或需确认 | 远程相关模块 | 跑通任一场景最小闭环 |

## P2：体验与工程化

| 能力 | 状态 | SideX 现状/差异 | 代码落点（参考） | 验证方法 |
|---|---|---|---|---|
| 性能与资源基准（启动/内存/搜索/扩展激活） | 🔴 | 缺少统一基准与持续回归 | 待新增 perf docs/scripts | 形成基线数据并可重复测量 |
| 可观测性（metrics + 诊断面板） | 🟡 | 部分日志，缺少指标与 UI | diagnostics + logging | 能定位关键链路耗时与错误 |
| 回归测试（void 兼容性） | 🔴 | 缺少自动化冒烟套件 | 待新增 tests/scripts | 关键冒烟流程可自动运行 |
| 打包资源校验（构建产物自检） | 🟡 | 已补齐部分打包路径，需自检脚本固化 | build/postbuild/scripts | 构建后自检通过或给出明确缺失项 |

## 维护方式

- 每次合并前：更新本矩阵中对应条目的状态与差异说明。
- 对“🟡 部分对齐”条目：必须补充缺口列表与明确的下一步任务链接。
