# SideX（基于 void 改造）开发规则

本项目明确为：基于 void 源码改造的 Rust 编辑器项目。所有改动必须保证与 void 源码中已实现的功能与行为兼容，并以“兼容优先”为工程准则。

## 1. 上游基准与兼容范围

- 上游基准：void 源码（其上游为 VSCode / Code-OSS 分层架构）
- 兼容目标：
  - Workbench/Monaco 的行为与 API 期望保持一致
  - 关键能力链路（文件系统、终端、Git、搜索、菜单/窗口、扩展系统）不得回退
  - 依赖资源（内置扩展、extension-host 脚本、NLS 资源）必须可被正确打包与加载

## 2. 修改策略（优先级）

1. 优先在 SideX 适配层做改动（桥接/替换/补丁），避免改动 VSCode/void 上层业务逻辑
2. 若必须改动 workbench 逻辑，要求：
   - 明确与 void 的差异点与原因
   - 保留 void 的调用契约（接口、事件、数据形状、时序）
3. 严禁引入会破坏跨平台的假设（路径分隔符、大小写、系统依赖、权限）

## 3. 关键模块“不可破坏”清单

- 启动与 bridge：
  - 前端入口：`src/main.ts`
  - Rust commands 注册：`src-tauri/src/lib.rs`
  - `invoke()/emit()/listen()` 事件与命令名保持稳定
- 文件系统与工作区：
  - 文件树、文件操作、watcher、搜索能力保持可用
- 终端：
  - PTY spawn/write/resize/kill 行为保持稳定
  - shell 集成脚本可用并随构建/打包正确带入
- Git：
  - SCM 相关命令、状态刷新、diff/log 与 push/pull/fetch 链路保持稳定
- 扩展体系：
  - Node 扩展宿主 sidecar 可启动、可连接、可扫描扩展目录
  - 内置扩展资源可被打包并在运行时可发现
  - WASM 扩展加载与 provider 注册链路保持稳定

## 4. 变更前/后最小验证清单

每次修改代码后，至少完成以下验证（按改动范围取子集）：

- 前端
  - `npm run lint`
  - 启动验证：`npm run tauri dev`（能进入 workbench，核心视图正常）
- 扩展（若涉及 extensions/build/tauri resource）
  - `npm run setup` 或 `npm run setup:full` 生成内置扩展元数据
  - `npm run build` 后检查产物存在 `dist/extensions` 与 `dist/extensions-meta.json`
  - 运行日志中能看到扩展扫描数量与宿主启动信息
- Rust（若涉及 src-tauri/crates）
  - 在目标平台完成 `cargo check` / `cargo test`（系统依赖按平台补齐）

## 5. 交付约束

- 不输出或提交任何敏感信息（token、密钥、证书、内部地址）
- 提交信息使用 Conventional Commits（feat/fix/refactor/chore/docs/test）
- 任何“兼容 void 的破坏性变更”必须先通过兼容方案评审或提供迁移策略

## 6. 工作方式

- 需要对照 void 实现时：先定位 void 对应模块的行为与契约，再在 SideX 里实现等价能力或兼容层
- 遇到不确定是否影响兼容性：优先补充诊断日志/开关，但不要改变默认行为
