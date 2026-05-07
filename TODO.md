# SideX 相对 void 的差距 TODO

本清单用于跟踪 SideX（Tauri + Rust）相对 void 源码在功能与行为上的差距，优先保证“兼容 void 已实现能力”，在此基础上补齐缺口。

> 说明：条目来源主要依据本仓库 [README.md](file:///workspace/README.md) 与 [ARCHITECTURE.md](file:///workspace/ARCHITECTURE.md) 中标注的 Porting Status（in progress / partial / not started），以及当前默认配置（例如 settings sync、telemetry、chat 等开关）。

## P0：扩展与调试（最影响 void 生态）

- [ ] Node 扩展宿主：对齐 void 的扩展生命周期（激活/停用/崩溃恢复/隔离）与 API 覆盖范围
- [ ] 扩展宿主：完善与 workbench 的协议与诊断能力（可视化扩展错误、慢扩展检测、崩溃日志）
- [ ] 扩展能力：对齐 void 的 Webview 扩展能力（资源加载、CSP、消息通道、性能与安全策略）
- [ ] 扩展管理：对齐 void 的扩展市场与安装体验（更多来源、更新策略、依赖处理、兼容性提示）
- [ ] 调试器：DAP 全链路对齐（断点、变量/调用栈、调试控制台、attach/launch、多会话）
- [ ] 调试适配器管理：对齐 void 的 adapter 发现、下载/配置与运行沙箱策略

## P0：核心平台能力差距（ARCHITECTURE 标注 Partial/Not started）

- [x] powerMonitor：系统睡眠/唤醒、电源状态监听等（ARCHITECTURE：Not started） - **已实现**
- [x] contentTracing：性能追踪/采样与导出（ARCHITECTURE：Not started） - **已实现**
- [x] native-keymap：键盘布局/物理键位映射与快捷键一致性（ARCHITECTURE：Not started） - **已实现**
- [x] safeStorage/encryption：安全存储与加密能力完整对齐（ARCHITECTURE：Partial） - **已实现**
- [x] logging：对齐 void 的日志能力（日志文件、级别、持久化、崩溃日志聚合）（ARCHITECTURE：Partial） - **已实现**

## P1：Workbench 功能对齐（按 void 常用能力补齐）

- [ ] Settings Sync：设置同步（目前默认关闭，需要对齐 void 的 UI/流程/存储）
- [ ] Profiles：用户配置/扩展/设置的 Profile 管理与切换（若要对齐 void 体验）
- [ ] Authentication：对齐 void 的账户认证与 token 生命周期（GitHub 等）
- [ ] 更新机制：与 void 的更新/回滚/签名校验策略对齐（SideX 已有 sidex-update，但需核对行为一致性）
- [ ] 多窗口/多工作区：跨窗口工作区切换、恢复与状态保存的兼容性回归
- [ ] 搜索与索引：大仓库性能、ignore 规则、符号搜索与结果排序的兼容性回归

## P1：远程与容器（如 void 支持的场景）

- [ ] Remote-SSH：连接、端口转发、文件系统映射、扩展运行位置与兼容性回归
- [ ] Dev Containers：容器启动、环境探测、端口映射、扩展与语言服务运行策略
- [ ] WSL / Codespaces：平台适配与登录/鉴权链路（如需对齐 void）

## P2：体验完善与工程化

- [ ] 性能与资源：启动耗时、渲染帧率、内存曲线与 void 对齐的基准与持续回归
- [ ] 可观测性：关键链路 metrics（启动、扩展激活、搜索、git、终端）与诊断面板
- [ ] 回归测试：引入面向“void 兼容性”的最小回归套件（冒烟测试 + 关键命令/事件时序）
- [ ] 打包资源校验：构建产物自检（extension-host、builtin extensions、NLS、shell-integration 等必需资源）

## 待补充（需要 void 对照确认）

- [ ] void 特有功能清单对齐：从 void 源码/发行版梳理差异并在此文件补齐（命令、设置项、默认行为、内置扩展）
