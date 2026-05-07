# 自动提交并推送代码规则

每次完成代码修改任务后，必须自动执行 **提交 + 推送** 的完整流程，确保远端仓库始终包含最新改动。

## 触发条件

满足以下任一条件时触发：

1. 完成功能开发 / Bug 修复 / 代码重构 / 配置更新
2. 用户明确表达"提交代码"、"推送代码"、"提交并推送"

## 执行步骤

### 1. 检查是否有变更

```bash
git status --porcelain=v1 -b
```

如果没有文件变更，跳过后续步骤。

### 2. 暂存所有变更

```bash
git add -A
```

### 3. 生成提交信息并提交

提交信息使用 Conventional Commits 风格：
- `feat:` 新功能
- `fix:` Bug 修复
- `refactor:` 代码重构
- `chore:` 构建/工具链/配置
- `docs:` 文档更新
- `test:` 测试

```bash
git commit -m "<type>: <summary>"
```

### 4. 推送到远端

```bash
git push https://<GITHUB_PAT>@github.com/qiphon/sidex.git HEAD:refs/heads/<current-branch>
```

如果当前分支尚未设置 upstream，使用完整 refspec 推送：

```bash
git push https://<GITHUB_PAT>@github.com/qiphon/sidex.git HEAD:refs/heads/<current-branch>
```

### 5. 确认推送结果

```bash
git status --porcelain=v1 -b
```

确认本地分支与远端同步（无 ahead 提交）。

## 注意事项

- **安全**：推送命令中的 PAT token 不得出现在提交信息、日志输出或任何非推送命令中
- **敏感文件**：不要提交包含密钥、token、证书的文件；如发现应先移除并更新 .gitignore
- **冲突处理**：若远端拒绝推送（需要 pull/rebase），先与用户确认再操作
- **空提交**：没有变更时不提交
- **构建错误**：如果存在明显的构建/类型错误，应先修复再提交
