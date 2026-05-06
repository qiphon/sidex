# void 兼容性冒烟清单（SideX）

本清单用于每次重要改动后快速验证 SideX 关键链路不回退。默认以 Windows 作为主要验证平台，其他平台可按需补充差异步骤。

## 0. 准备

- `npm install`
- 如涉及内置扩展/扩展宿主/打包资源：执行 `npm run setup`（必要时 `npm run setup:full`）

## 1. 启动与基础 UI

- [ ] 启动 `npm run tauri dev`，能进入 workbench
- [ ] 命令面板可打开（`workbench.action.showCommands`）
- [ ] Explorer / Search / SCM / Extensions / Terminal / Output 视图可切换
- [ ] 打开文件夹后窗口标题与工作区状态正确

## 2. 文件系统与编辑器

- [ ] 新建文件、编辑、保存、另存为正常
- [ ] 重命名/删除文件或文件夹正常
- [ ] 文件监听正常（外部改动能刷新）
- [ ] 搜索：文件名搜索与全文搜索可用，结果可打开定位

## 3. 终端

- [ ] 新建终端可用（可输入命令并回显）
- [ ] 分屏终端可用
- [ ] 终端 resize 后布局正确
- [ ] 终端关闭/重启无残留进程（至少不阻塞后续 spawn）

## 4. Git

- [ ] 打开一个 git 仓库：SCM 能显示变更
- [ ] stage/unstage 可用
- [ ] commit 可用
- [ ] diff 视图可打开
- [ ] push/pull/fetch 链路可用（如环境允许）

## 5. 扩展（Open VSX）

- [ ] 打开 Extensions 视图，能搜索并安装扩展
- [ ] 安装后无需重启或可按预期提示重启
- [ ] 扩展激活能触发（例如安装一个提供命令的扩展并执行其命令）
- [ ] 扩展宿主异常时有可见提示/日志可定位

## 6. 调试（如涉及 debug）

- [ ] 启动调试：能设置断点并命中
- [ ] 单步/继续/停止可用
- [ ] 变量/调用栈/控制台有输出

## 7. 构建与打包资源（如涉及 build/tauri resource）

- [ ] `npm run build` 成功
- [ ] `dist/extensions` 存在
- [ ] `dist/extensions-meta.json` 存在
- [ ] `src-tauri/extension-host/**` 资源在打包配置中可被递归包含
