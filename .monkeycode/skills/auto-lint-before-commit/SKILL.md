# 自动 Lint 检查规则

每次提交代码前，**必须**使用项目 lint 工具检查代码，确保代码通过 lint 检查后再提交。

## 项目 Lint 配置

- **Lint 命令**: `npm run lint`
- **自动修复**: `npm run lint:fix`
- **格式化**: `npm run format`

## 执行流程

### 1. 开发完成后运行 Lint

```bash
npm run lint
```

### 2. 如有 lint 错误，使用自动修复

```bash
npm run lint:fix
```

### 3. 手动检查剩余问题

自动修复无法解决的问题需要手动修复。

### 4. 确认 lint 通过后再提交

```bash
git add .
git commit -m "<commit message>"
```

## 注意事项

- Lint 检查失败时**不要强制提交**
- 如果存在 lint 错误，先修复再提交
- 提交信息应清晰描述修改内容
- 遵循项目的代码规范

## 触发时机

当用户完成以下任务时自动触发：
- 功能开发
- Bug 修复
- 代码重构
- 任何涉及代码修改的任务
