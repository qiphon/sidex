# Auto Config Remote Before Commit

## Description
自动从环境变量中读取仓库地址和 token，并在提交代码前或推送代码前配置远程仓库。

## Trigger
- 在执行 `git commit` 之前
- 在执行 `git push` 之前
- 当检测到远程仓库未配置时
- 在需要推送代码到远端之前

## Environment Variables
- `gitAddr`: Git 仓库地址 (例如：https://github.com/user/repo.git)
- `token`: GitHub Personal Access Token

## Actions
1. 检查环境变量 `gitAddr` 和 `token` 是否存在
2. 如果存在，检查当前 git 仓库是否已配置远程仓库 `origin`
3. 如果远程仓库未配置，添加远程仓库地址为 `https://${token}@${gitAddr}`
4. 如果远程仓库 `origin` 已存在但没有正确的 token，则更新为 `https://${token}@${gitAddr}`
5. 输出配置成功信息

## Example Usage
```bash
# 环境变量示例
export gitAddr=https://github.com/qiphon/sidex.git
export token=github_pat_xxxxxxxxxxxx

# 执行提交前会自动配置远程仓库
git add .
git commit -m "feat: add new feature"
git push origin fix-extension
```

## Notes
- Token 应该具有足够的权限来推送代码到目标仓库
- 建议在 CI/CD 环境或安全的环境中使用此 skill
- 敏感信息不会出现在 git 历史中
- 每次执行 push 操作前都应该检查远程仓库配置是否正确
