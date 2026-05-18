# 自动推送代码到远端规则

当完成功能开发、bug 修复或其他代码修改后，**必须**将代码推送到远端仓库，但**不能推送 workflow 文件的修改**。

## 触发条件

满足以下任一条件时，执行自动推送：

1. **完成代码修改后**
   - 完成功能开发
   - 完成 bug 修复
   - 完成代码重构
   - 完成文档更新

2. **明确要求推送时**
   - 用户说"推送代码"、"push 代码"、"提交到远端"

## 执行步骤

### 1. 检查当前状态

```bash
git status
git branch --show-current
```

### 2. 添加并提交修改（排除 workflow 文件）

**重要：不要提交 `.github/workflows/` 目录下的任何文件修改！**

```bash
# 只添加非 workflow 文件的修改
git add .
git restore --staged .github/workflows/ 2>/dev/null || true
```

如果只有 workflow 文件被修改，不要推送，只提交即可。

### 3. 生成提交信息并提交

```bash
git commit -m "<commit message>"
```

提交信息应遵循以下格式：
- `feat: <描述>` - 新功能
- `fix: <描述>` - bug 修复
- `chore: <描述>` - 杂项/工具链
- `refactor: <描述>` - 代码重构
- `docs: <描述>` - 文档更新

### 4. 推送到远端

```bash
git push -u origin <current-branch>
```

如果分支在远端不存在，会自动创建并推送。

## 重要约束

- **不要推送 workflow 文件**：`.github/workflows/` 目录下的任何文件修改都不要推送到远端
- 如果 workflow 文件有修改，必须由用户手动在 GitHub 网页上修改，或提供带有 `workflow` scope 的 GitHub Token
- 推送前确保提交信息清晰描述了修改内容

## 自动触发时机

根据用户指令记忆规则（MEMORY.md），当用户在对话中教导"开发完成后自动推送代码到远端"时，应将本 skill 添加到记忆文件中，并在每次完成开发任务后主动执行推送操作，但排除 workflow 文件的修改。
