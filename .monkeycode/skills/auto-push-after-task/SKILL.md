# 任务完成后自动推送代码规则

当一个任务完成并产出代码改动后，必须自动将本地分支推送到远端对应分支，保证远端始终包含最新改动。

## 触发条件

满足以下条件时触发：

1. 任务完成（功能开发 / Bug 修复 / 重构 / 文档更新 / 配置更新等）
2. 当前仓库存在已提交但未推送到远端的提交（本地分支 ahead 远端）

## 执行步骤

### 1. 确认当前分支与状态

```bash
git branch --show-current
git status --porcelain=v1 -b
```

若 `git status -b` 显示当前分支 `ahead` 远端，继续执行推送；否则跳过。

### 2. 推送到远端

```bash
git push
```

如果当前分支尚未设置 upstream，则执行：

```bash
git push -u origin <current-branch>
```

## 注意事项

- 推送前不得输出或记录任何敏感信息（token、密钥、证书等）
- 若远端拒绝推送（例如需要先 pull/rebase），应先与用户确认再变基或合并
- 默认推送到 `origin`，如用户指定其他 remote，则以用户指定为准
