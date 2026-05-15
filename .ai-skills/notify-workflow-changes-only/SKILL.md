# 不直接修改 workflow 文件规则

当需要修改 GitHub Workflow 文件（位于 `.github/workflows/` 目录下）时，**不要直接修改这些文件**，而是告知用户如何修改，由用户手动进行更改。

## 什么是 Workflow 文件

Workflow 文件是位于 `.github/workflows/` 目录下的 YAML 文件，这些文件控制 GitHub Actions 的行为。

## 触发条件

1. 需要修改 `.github/workflows/` 目录下的任何文件时，不管是新增、修改还是删除
2. 检查文件路径是否匹配以下模式：
   - `.github/workflows/*.yml`
   - `.github/workflows/*.yaml`

## 执行步骤

### 1. 先分析需要做什么修改

阅读用户要求，确定需要对 Workflow 文件进行的修改内容。

### 2. 不要直接修改文件

不要使用 Edit、Write、git add 或其他任何会修改这些文件的工具。

### 3. 向用户说明需要做的修改

以清晰、结构化的方式向用户告知需要做的修改：

- 明确指出是哪个文件
- 说明具体需要做什么修改（添加、修改、删除内容）
- 给出代码片段或完整的文件内容建议

例如：

```
您需要修改以下文件：

**文件：.github/workflows/build.yml**

在 Install Linux system dependencies 步骤中，添加以下包：
- libglib2.0-dev
- pkg-config
```

### 4. 可以进行其他非 workflow 文件的修改

其他类型的文件可以正常修改，但 workflow 文件必须由用户处理。

## 注意事项

- 这个规则适用于所有位于 `.github/workflows/` 目录下的文件
- 如果用户明确表示“我来修改 workflow 文件”或类似表述，此规则仍然适用
- 如果用户有带 `workflow` scope 的 GitHub PAT 并要求直接修改，请先确认用户的意图
