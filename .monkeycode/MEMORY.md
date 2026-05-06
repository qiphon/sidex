# 用户指令记忆

本文件记录了用户的指令、偏好和教导，用于在未来的交互中提供参考。

## 格式

### 用户指令条目
用户指令条目应遵循以下格式：

[用户指令摘要]
- Date: [YYYY-MM-DD]
- Context: [提及的场景或时间]
- Instructions:
  - [用户教导或指示的内容，逐行描述]

### 项目知识条目
Agent 在任务执行过程中发现的条目应遵循以下格式：

[项目知识摘要]
- Date: [YYYY-MM-DD]
- Context: Agent 在执行 [具体任务描述] 时发现
- Category: [代码结构|代码模式|代码生成|构建方法|测试方法|依赖关系|环境配置]
- Instructions:
  - [具体的知识点，逐行描述]

## 去重策略
- 添加新条目前，检查是否存在相似或相同的指令
- 若发现重复，跳过新条目或与已有条目合并
- 合并时，更新上下文或日期信息
- 这有助于避免冗余条目，保持记忆文件整洁

## 条目

[任务完成后自动推送到远端]
- Date: 2026-05-06
- Context: 用户要求每次任务完成自动推送代码到远端
- Instructions:
  - 当任务完成且本地分支存在未推送提交（ahead 远端）时，自动执行 `git push` 推送到远端分支。
  - 若未设置 upstream，则使用 `git push -u origin <current-branch>` 建立追踪关系。
  - 推送过程中不得输出或记录任何敏感信息（token、密钥、证书等）。
  - 对应 workspace skill：`.ai-skills/auto-push-after-task/SKILL.md`。

[兼容 void 源码优先]
- Date: 2026-05-06
- Context: 用户要求项目明确基于 void 改造，并保证每次修改兼容 void 已实现功能
- Instructions:
  - 将 void 源码作为功能与行为基准；对 workbench/Monaco/核心链路的改动必须保持兼容与不回退。
  - 优先在 SideX 的适配层（Tauri/Rust bridge 与替换实现）完成变更，避免直接修改上层 workbench 行为。
  - 修改扩展/终端/Git/文件系统/构建打包时，必须执行对应的最小回归验证清单，确保链路可用。

[开发完成后自动提交代码]
- Date: 2026-05-06
- Context: 用户要求每次修改完代码自动提交
- Instructions:
  - 当完成任何涉及代码修改的任务且工作区存在未提交变更时，自动执行 `git add -A` 并创建一次提交。
  - 提交信息使用 Conventional Commits 风格，且不得包含任何敏感信息。
  - 对应 workspace skill：`.ai-skills/auto-commit-after-change/SKILL.md`。

[SideX Tauri 启动入口与项目打开流程]
- Date: 2026-04-29
- Context: Agent 在执行修复拖拽文件夹进入窗口无法添加项目问题时发现
- Category: 代码结构
- Instructions:
  - 桌面端前端启动入口位于 `src/main.ts`，Tauri 相关全局初始化在 `boot()` 创建 workbench 后执行。
  - 打开项目文件夹通过 URL 查询参数 `folder` 传递，`navigateToFolder()` 会写入该参数并刷新页面。
  - Tauri 窗口配置位于 `src-tauri/tauri.conf.json` 和 `src-tauri/tauri.sidex-ui.conf.json`，原生文件拖放能力由 `app.windows[].dragDropEnabled` 控制。
