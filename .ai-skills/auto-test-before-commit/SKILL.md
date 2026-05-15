# 自动测试检查规则

每次提交代码前，**必须**运行项目测试用例，确保所有测试通过后再提交。

## 项目测试配置

### 前置要求

#### Linux 系统依赖（仅 Linux 环境需要）

```bash
# Ubuntu/Debian
sudo apt-get update
sudo apt-get install -y \
  libwebkit2gtk-4.1-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  libssl-dev \
  libglib2.0-dev

# Fedora
sudo dnf install \
  gtk3-devel \
  webkit2gtk4.1-devel \
  libappindicator-gtk3-devel \
  librsvg2-devel \
  openssl-devel \
  glib2-devel
```

### TypeScript 测试

```bash
# 安装依赖
npm install

# 运行所有测试
npm test

# 或使用 mocha（如果项目配置了）
npx mocha "src/**/*.test.ts"

# 运行特定测试文件
npx mocha src/vs/workbench/contrib/scm/browser/git.contribution.test.ts
```

### Rust 测试

```bash
# 安装 Rust 依赖
cd src-tauri && cargo fetch

# 运行库测试（仅编译和运行 #[cfg(test)] 测试）
cd src-tauri && cargo test --lib

# 运行所有测试（包括集成测试）
cd src-tauri && cargo test

# 运行特定测试
cd src-tauri && cargo test urlencoding
```

## 执行流程

### 1. 提交前检查测试

在 `git add` 和 `git commit` 之前，**必须**运行测试：

```bash
# TypeScript 测试
npm test

# Rust 测试
cd src-tauri && cargo test --lib
```

### 2. 处理测试失败

#### 如果测试失败：

1. **不要强制提交** - 测试失败说明代码有问题
2. **查看测试输出** - 了解失败原因
3. **修复代码或测试** - 根据失败原因修复
4. **重新运行测试** - 确保修复后测试通过
5. **再次尝试提交**

#### 常见测试失败处理：

- **编译错误**：修复语法或类型错误
- **断言失败**：检查预期值是否正确，可能需要修复测试用例
- **依赖缺失**：安装缺失的依赖 `npm install` 或 `cargo fetch`
- **环境问题**：配置正确的测试环境

### 3. 测试通过后提交

```bash
git add .
git commit -m "<commit message>"
```

## 测试用例说明

### TypeScript 测试位置

- `src/vs/workbench/contrib/scm/browser/git.contribution.test.ts` - Git Graph URI 解析测试

### Rust 测试位置

- `src-tauri/src/lib.rs` 中的 `#[cfg(test)] mod tests` - urlencoding 测试

### 测试覆盖的功能

1. **Git Graph URI 解析**
   - Base64 编解码
   - UTF-8 中文支持
   - URL 编码处理
   - 多种格式支持

2. **urlencoding 库**
   - URL 解码
   - URL 编码
   - Unicode 处理
   - 特殊字符处理

## 注意事项

- **测试失败时禁止提交** - 必须先修复测试
- **保持测试通过** - 新代码必须通过现有测试
- **添加新测试** - 新功能应添加对应的测试用例
- **测试幂等性** - 测试可以重复运行，结果应一致

## 触发时机

当用户完成以下任务时自动触发：

- 功能开发
- Bug 修复
- 代码重构
- 添加测试用例
- 修改现有测试
- 任何涉及代码修改的任务

## 紧急情况处理

如果测试环境不可用（如缺少依赖、配置问题），可以：

1. 先提交代码（使用 `--no-verify`）
2. 在提交信息中注明测试环境问题
3. 后续修复测试环境后补充测试

**但这是最后的手段**，正常情况下应该确保测试通过后再提交。
