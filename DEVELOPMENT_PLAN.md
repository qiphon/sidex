# SideX 对齐 void 的详细开发计划（基于 TODO.md）

本文档基于 [TODO.md](file:///workspace/TODO.md) 将“SideX 相对 void 的差距”拆解为可执行的开发计划。目标是：在不破坏 void 已有行为与契约的前提下，逐步补齐缺失能力，并建立可持续的“兼容性回归”机制。

## 0. 前置原则（强约束）

- 兼容优先：void 源码与其默认行为是基准；任何变更不得造成能力回退或行为漂移。
- 适配层优先：优先在 SideX 的 bridge/adapter 层完成变更，尽量避免修改 workbench 上层逻辑。
- 资源可用性：扩展宿主、内置扩展、NLS、shell-integration 等资源必须在构建/打包产物中可发现、可加载。
- 安全与隐私：禁止引入日志/提交中泄露 token/密钥；认证与安全存储必须可审计。

## 1. 计划结构与里程碑

计划按优先级分为 5 个工作流，按“门禁条件（gating）”推进：

- A. 兼容性基线与回归机制（持续贯穿）
- B. P0：扩展与调试（生态核心）
- C. P0：平台能力缺口（Partial/Not started）
- D. P1：Workbench 能力对齐
- E. P1：远程与容器
- F. P2：体验完善与工程化

每个工作流均包含：
- 目标与非目标
- 交付物
- 子任务拆解（代码落点）
- 风险与回滚策略
- 验收标准与验证方式

## A. 兼容性基线与回归机制（gating：所有阶段必须满足）

### A1. 建立 void → SideX “功能矩阵”

**目标**
- 把 void 的关键功能、入口命令、设置项、默认行为整理为可审阅的对照表，作为每次变更的回归依据。

**交付物**
- `docs/parity/void-parity-matrix.md`（建议新增）：按模块列出“已对齐/部分对齐/未开始/不计划”。
- `docs/parity/void-smoke-checklist.md`（建议新增）：冒烟测试步骤（手工 + 可自动化）。

**任务拆解**
- 从 void 源码/发行版梳理：
  - 核心视图：Explorer/Search/SCM/Debug/Extensions/Terminal/Output/Problems
  - 关键命令：openFolder、search、git push/pull、run task、start debug、install extension 等
  - 关键设置：terminal profiles、git、extensions、security、sync、profiles
- 在 SideX 中确认对应路径与差异点，记录在矩阵中，并标注验证方式。

**验收标准**
- 功能矩阵覆盖 P0/P1 的所有条目，且每项都有“验证方法”和“代码落点”。

### A2. 兼容性门禁（每次改动最小回归）

**目标**
- 将关键链路的“最小回归”固化为标准流程，避免修一处坏一处。

**建议门禁**
- TS：`npm run lint`
- 运行：`npm run tauri dev` 能启动进入 workbench，核心视图可打开
- 扩展（涉及时）：
  - `npm run setup`/`setup:full` 生成元数据
  - `npm run build` 后产物存在 `dist/extensions`、`dist/extensions-meta.json`
  - 日志能看到扩展扫描数量与宿主启动信息
- Rust（涉及时）：目标平台 `cargo check`（系统依赖按平台补齐）

**验收标准**
- 每次合并到主开发分支前，至少通过对应子集门禁；失败必须阻断提交/发布。

## B. P0：扩展与调试（最影响 void 生态）

### B1. Node 扩展宿主：生命周期与 API 覆盖

**目标**
- 对齐 void 的扩展生命周期：发现 → 激活 → 运行 → 停用/卸载 → 崩溃恢复。
- 扩展 API 覆盖达到“void 关键内置扩展 + 常见第三方扩展”可运行的程度。

**交付物**
- 扩展宿主生命周期状态机与诊断面板（至少：已发现/已激活/失败原因/耗时）。
- 扩展崩溃自动重启策略（限频、保留日志、用户可见提示）。

**代码落点（参考）**
- Rust：`src-tauri/src/commands/ext_host.rs`（启动、会话管理、stdout 端口协议、stderr 日志）
- Rust：`src-tauri/src/commands/extension_platform.rs`（扫描路径、manifest、initData）
- Node：`src-tauri/extension-host/server.cjs`、`host.cjs`（协议、RPC、webview 资源处理）
- TS：`src/vs/workbench/contrib/extensions/browser/tauriExtensionHost.contribution.ts`（bootstrap、连接、hot-load）

**关键子任务**
- 端口握手协议加固：
  - 保证 stdout 首行仅包含端口 JSON（其余日志走 stderr）
  - 增加版本字段/兼容标记，便于未来演进
- 生命周期事件：
  - 记录 extension activate/deactivate、错误、耗时
  - 支持 unload/disable/enable 与 workbench UI 交互一致
- API 覆盖：
  - 以 void 常用扩展为基准建立“最小 API 列表”（commands、workspace、fs、window、terminal、scm、languages 等）
  - 缺失 API 统一走兼容层补齐（而不是让扩展静默失败）

**验收标准**
- 选择 5–10 个代表性扩展（含内置与第三方），在 SideX 中安装、启用、触发功能无崩溃，且错误可诊断。

### B2. 扩展宿主诊断与可观测性

**目标**
- 用户能在 UI 中看到“扩展为什么不可用”，开发者能快速定位协议/资源/权限问题。

**交付物**
- 扩展诊断 API 与 UI：
  - 扩展列表：状态、耗时、错误堆栈、日志入口
  - 慢扩展检测：阈值、告警、排序

**关键子任务**
- Rust 侧记录运行时状态（已存在诊断模块雏形时，补齐与前端联动）
- Node 宿主将错误与性能指标通过 RPC 上报

**验收标准**
- 扩展故障可以在 1–2 个入口页面中直接看到根因（缺资源/入口不存在/禁用/崩溃）。

### B3. Webview 扩展能力对齐

**目标**
- 对齐 void 的 webview 资源加载、CSP、安全策略与消息通道。

**关键子任务**
- 资源 URL → file 路径映射与隔离策略
- CSP/可信域与 void 一致（默认更严格时必须提供兼容策略）
- 消息通道、序列化策略、性能（大消息、二进制资源）

**验收标准**
- 选取 1–2 个 webview 扩展（例如 Markdown 预览/自定义视图）验证可用。

### B4. 调试器（DAP）全链路对齐

**目标**
- 对齐 void 的 debug 体验：launch/attach、多会话、断点、变量、调用栈、控制台。

**代码落点（参考）**
- Rust：`crates/sidex-dap`、`src-tauri/src/commands/debug.rs`
- TS：workbench debug 相关 contrib/service（按现状逐步补齐）

**关键子任务**
- 适配器启动与通信：
  - 进程生命周期、端口/stdio 通道、重连策略
- UI 协议：
  - 断点同步、停靠点、状态栏、调试控制台输出

**验收标准**
- 以 Node/JS、Python、Rust 任一语言为基准，能完成“设置断点→启动调试→单步→查看变量→停止”。

## C. P0：核心平台能力缺口（ARCHITECTURE：Partial/Not started）

### C1. powerMonitor（Not started）

**目标**
- 提供与 void 等价的系统事件：sleep/resume、锁屏、电源状态变化（按平台分级支持）。

**关键子任务**
- Rust：选择跨平台 crate 或按平台实现（macOS/Windows/Linux）
- TS：暴露统一接口给 workbench（事件名、payload 与 void 对齐）

**验收标准**
- 至少能触发 sleep/resume 事件并在日志中观测到，且不会导致终端/扩展异常状态。

### C2. contentTracing（Not started）

**目标**
- 对齐 void 的性能诊断能力：关键链路 trace、导出、采样策略（至少开发可用）。

**关键子任务**
- Rust：tracing 采集与导出（JSON/Chrome trace）
- TS：触发与导出入口（命令/菜单）

**验收标准**
- 能导出一次启动/扩展激活 trace 文件并能用 Chrome trace viewer 打开。

### C3. native-keymap（Not started）

**目标**
- 快捷键在不同键盘布局下行为与 void 一致（尤其是 macOS/Windows 的物理键位差异）。

**关键子任务**
- Rust：获取系统键盘布局与扫描码映射
- TS：与 keybinding service 协作，保证 UI 显示与触发一致

**验收标准**
- 选取一组常用快捷键（复制粘贴、搜索、命令面板、切换终端），在至少两种布局下不漂移。

### C4. safeStorage/encryption（Partial）

**目标**
- 对齐 void 的安全存储：token/密钥保存、访问控制、跨平台一致行为。

**关键子任务**
- Rust：keyring/secure enclave/windows credential manager 集成
- TS：统一 API；失败回退策略与用户提示

**验收标准**
- GitHub token 等敏感数据可安全写入/读取；卸载/重装行为符合预期。

### C5. logging（Partial）

**目标**
- 对齐 void 的日志：级别、文件落盘、崩溃/扩展日志聚合、导出。

**关键子任务**
- Rust：tracing-subscriber 输出到文件 + 轮转
- TS：workbench 日志视图/命令对齐

**验收标准**
- 出现扩展宿主崩溃时，用户能导出日志并定位 session 与错误原因。

## D. P1：Workbench 功能对齐

### D1. Settings Sync（当前默认关闭）

**目标**
- 对齐 void 的设置同步体验：UI、登录、同步项、冲突处理、存储位置。

**关键子任务**
- 明确同步范围（settings/keybindings/extensions/snippets/ui state）
- 本地存储与服务端协议（若沿用 VSCode/void 方案需兼容）

**验收标准**
- 完成一次“登录→开启同步→换机/新 profile 拉取”的闭环（可先做本地模拟）。

### D2. Profiles

**目标**
- 对齐 void 的 profile 管理：创建/切换/导出/导入，隔离设置与扩展。

**关键子任务**
- Rust/DB：profile 数据结构与存储
- TS：profile UI 与切换时的资源重载/窗口恢复

**验收标准**
- 两个 profile 之间切换后，扩展列表与设置按预期隔离。

### D3. Authentication

**目标**
- 对齐 void 的账户认证与 token 生命周期（GitHub 等），支持扩展使用。

**关键子任务**
- OAuth 流程与回调处理（Tauri scheme/protocol）
- token 存储（依赖 C4）

**验收标准**
- GitHub 认证可用于 git 操作或扩展请求（按 void 行为对齐）。

### D4. 更新机制对齐

**目标**
- 核对并对齐 void 的更新/回滚/签名校验策略；确保用户可控与可诊断。

**关键子任务**
- 对 sidex-update 的行为与 void 对照：更新检查频率、提示 UI、签名错误处理

**验收标准**
- 模拟更新元数据场景，行为与 void 预期一致（失败有提示，可回滚/重试）。

### D5. 多窗口/多工作区、搜索与索引回归

**目标**
- 在大仓库与多窗口场景下，确保行为与性能不低于 void 的基准。

**关键子任务**
- 多窗口：状态保存、恢复、工作区切换时序
- 搜索：ignore 规则、排序、索引更新与 watcher 事件一致性

**验收标准**
- 选取一个中大型仓库完成冒烟：打开/搜索/替换/切换分支/索引更新不异常。

## E. P1：远程与容器（如 void 支持场景）

### E1. Remote-SSH

**目标**
- 支持连接、端口转发、文件系统映射、远端扩展运行策略，行为对齐 void。

**关键子任务**
- Rust：ssh/tunnel/port forwarding（已有 sidex-remote 需核对能力缺口）
- TS：remote authority 与文件系统 provider 对齐

**验收标准**
- 能连接远端并打开文件夹，基本编辑与终端可用。

### E2. Dev Containers / WSL / Codespaces

**策略**
- 以“最小闭环”推进：先跑通一个官方示例，再扩展兼容面。

**验收标准**
- 任一场景可用（按 void 的路径与 UI 期望对齐）。

## F. P2：体验完善与工程化

### F1. 性能与资源基准

**目标**
- 建立与 void 对齐的性能指标与回归：启动耗时、空闲内存、搜索耗时、扩展激活耗时等。

**交付物**
- `docs/perf/benchmarks.md`（建议新增）：指标定义、采集方法、基线数据。

### F2. 可观测性与诊断面板

**目标**
- 对关键链路提供 metrics 与诊断入口，减少“黑盒故障”。

### F3. 回归测试与打包资源自检

**目标**
- 将“资源完整性检查”变成构建的一部分，避免发布包缺失扩展/宿主脚本等关键资源。

**建议自检项**
- `dist/extensions`、`dist/extensions-meta.json`
- Tauri resources：`extension-host/**`、`shell-integration/*`
- `public/builtin-extensions.js`（或等价元数据注入产物）

## 2. 执行顺序建议（不含时间，仅含依赖关系）

推荐推进顺序：

1. A（基线与门禁）先落地到“可持续”
2. B（扩展宿主 & 调试）优先推进到“可用且可诊断”
3. C（平台缺口）按依赖推进：logging/safeStorage → native-keymap → powerMonitor/contentTracing
4. D（Workbench 对齐）与 B/C 交错推进：Authentication/Profiles/Sync 等依赖 safeStorage
5. E（远程/容器）在扩展与基础平台稳定后推进
6. F（工程化）贯穿全程，用于防回退与提升交付质量

## 3. 计划维护规则

- TODO.md 作为“差距列表”，DEVELOPMENT_PLAN.md 作为“拆解与门禁”；两者需保持一致。
- 每完成一个里程碑，应回写：
  - TODO.md 勾选状态
  - parity matrix（建议新增）中对应条目标记与验证结果
  - 若涉及行为差异，补充说明与兼容策略
